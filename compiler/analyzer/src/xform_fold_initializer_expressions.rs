//! Transform that folds constant-expression `VAR` initializers into plain
//! literal initializers.
//!
//! The IEC 61131-3 standard's `constant()` grammar production only permits
//! a bare literal in a `VAR` initializer position (e.g. `x : LREAL := 4.25;`).
//! Real CODESYS/TwinCAT code commonly uses a constant *expression* instead
//! (e.g. `scaled : LREAL := SCALE*4.0;`). The parser accepts this broader
//! form unconditionally, producing `InitialValueAssignmentKind::SimpleExpr`
//! — a placeholder that this pass always normalizes away before any other
//! semantic pass runs:
//!
//! - If the expression fully reduces to a constant (substituting references
//!   to known `CONSTANT`-qualified declarations, then folding arithmetic),
//!   it is rewritten to the ordinary `InitialValueAssignmentKind::Simple`
//!   shape.
//! - Otherwise (the expression references a non-constant, or
//!   `--allow-constant-initializer-expressions` is disabled), a diagnostic
//!   is emitted and the initializer is normalized to an uninitialized
//!   `Simple` so downstream passes never see `SimpleExpr`.
//!
//! ## Before
//!
//! ```ignore
//! VAR
//!     scaled : LREAL := SCALE*4.0;
//! END_VAR
//! ```
//!
//! ## After
//!
//! ```ignore
//! VAR
//!     scaled : LREAL := 10.0;
//! END_VAR
//! ```

use std::collections::HashMap;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::xform_fold_constant_expressions::{try_fold_binary, try_fold_unary};

pub fn apply(lib: Library, options: &CompilerOptions) -> Result<Library, Vec<Diagnostic>> {
    let constants = collect_constants(&lib);

    let mut folder = InitializerFolder {
        constants,
        options,
        diagnostics: vec![],
    };

    let result = folder.fold_library(lib).map_err(|e| vec![e]);

    if !folder.diagnostics.is_empty() {
        return Err(folder.diagnostics);
    }

    result
}

/// Scan the library for global constant declarations with literal values.
fn collect_constants(lib: &Library) -> HashMap<String, ConstantKind> {
    let mut constants = HashMap::new();

    for element in &lib.elements {
        match element {
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                collect_from_var_decls(decls, &mut constants);
            }
            LibraryElementKind::ConfigurationDeclaration(config) => {
                collect_from_var_decls(&config.global_var, &mut constants);
                for resource in &config.resource_decl {
                    collect_from_var_decls(&resource.global_vars, &mut constants);
                }
            }
            _ => {}
        }
    }

    constants
}

fn collect_from_var_decls(decls: &[VarDecl], constants: &mut HashMap<String, ConstantKind>) {
    for decl in decls {
        if decl.qualifier != DeclarationQualifier::Constant {
            continue;
        }

        let name = match &decl.identifier {
            VariableIdentifier::Symbol(id) => id.clone(),
            VariableIdentifier::Direct(d) => match &d.name {
                Some(name) => name.clone(),
                None => continue,
            },
        };

        if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
            if let Some(value) = &simple.initial_value {
                constants.insert(name.to_string().to_uppercase(), value.clone());
            }
        }
    }
}

/// Recursively substitutes known constant references and folds arithmetic
/// within an initializer's expression tree. Reuses the same binary/unary
/// folding rules as `xform_fold_constant_expressions`.
fn substitute_and_fold(expr: Expr, constants: &HashMap<String, ConstantKind>) -> Expr {
    let kind = match expr.kind {
        ExprKind::BinaryOp(binary) => {
            let left = substitute_and_fold(binary.left, constants);
            let right = substitute_and_fold(binary.right, constants);
            let binary = BinaryExpr {
                op: binary.op,
                left,
                right,
            };
            try_fold_binary(&binary).unwrap_or(ExprKind::BinaryOp(Box::new(binary)))
        }
        ExprKind::UnaryOp(unary) => {
            let term = substitute_and_fold(unary.term, constants);
            let unary = UnaryExpr { op: unary.op, term };
            try_fold_unary(&unary).unwrap_or(ExprKind::UnaryOp(Box::new(unary)))
        }
        ExprKind::Expression(inner) => {
            ExprKind::Expression(Box::new(substitute_and_fold(*inner, constants)))
        }
        ExprKind::Deref(inner) => ExprKind::Deref(Box::new(substitute_and_fold(*inner, constants))),
        ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
            match constants.get(&named.name.to_string().to_uppercase()) {
                Some(value) => ExprKind::Const(value.clone()),
                None => ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))),
            }
        }
        // Usually already resolved to `Variable` by
        // xform_resolve_late_bound_expr_kind (which runs before this pass
        // in the normal pipeline), but handled here too so this pass does
        // not depend on that ordering.
        ExprKind::LateBound(late_bound) => {
            match constants.get(&late_bound.value.to_string().to_uppercase()) {
                Some(value) => ExprKind::Const(value.clone()),
                None => ExprKind::LateBound(late_bound),
            }
        }
        other => other,
    };

    Expr {
        kind,
        resolved_type: expr.resolved_type,
    }
}

struct InitializerFolder<'a> {
    constants: HashMap<String, ConstantKind>,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl InitializerFolder<'_> {
    /// Normalizes a `SimpleExprInitializer` back to `Simple`, folding it if
    /// possible and emitting a diagnostic otherwise. Always returns
    /// `Simple` so that no other pass ever observes `SimpleExpr`.
    fn normalize(&mut self, se: SimpleExprInitializer) -> InitialValueAssignmentKind {
        if !self.options.allow_constant_initializer_expressions {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::ConstantInitializerExpressionNotAllowed,
                    Label::span(se.initial_value.span(), "Constant expression initializer"),
                )
                .with_context("type", &se.type_name.to_string()),
            );
            return InitialValueAssignmentKind::Simple(SimpleInitializer {
                type_name: se.type_name,
                initial_value: None,
            });
        }

        let folded = substitute_and_fold(se.initial_value, &self.constants);
        match folded.kind {
            ExprKind::Const(c) => InitialValueAssignmentKind::Simple(SimpleInitializer {
                type_name: se.type_name,
                initial_value: Some(c),
            }),
            _ => {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::InitializerNotConstantExpression,
                        Label::span(folded.span(), "Initializer expression"),
                    )
                    .with_context("type", &se.type_name.to_string()),
                );
                InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: se.type_name,
                    initial_value: None,
                })
            }
        }
    }
}

impl Fold<Diagnostic> for InitializerFolder<'_> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            InitialValueAssignmentKind::SimpleExpr(se) => Ok(self.normalize(se)),
            other => InitialValueAssignmentKind::recurse_fold(other, self),
        }
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }

    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use ironplc_parser::parse_program;
    use ironplc_test::cast;

    fn opts() -> CompilerOptions {
        CompilerOptions::from_dialect(Dialect::Rusty)
    }

    fn parse(src: &str, options: &CompilerOptions) -> Library {
        parse_program(src, &FileId::default(), options).unwrap()
    }

    fn find_var_decl<'a>(lib: &'a Library, var_name: &str) -> &'a VarDecl {
        for element in &lib.elements {
            let vars = match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => &fb.variables,
                LibraryElementKind::FunctionDeclaration(f) => &f.variables,
                LibraryElementKind::ProgramDeclaration(p) => &p.variables,
                LibraryElementKind::GlobalVarDeclarations(decls) => decls,
                _ => continue,
            };
            for var in vars {
                if var.identifier.to_string().eq_ignore_ascii_case(var_name) {
                    return var;
                }
            }
        }
        panic!("Variable '{}' not found", var_name);
    }

    fn real_value(var: &VarDecl) -> f64 {
        let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
        let lit = cast!(
            simple.initial_value.as_ref().unwrap(),
            ConstantKind::RealLiteral
        );
        lit.value
    }

    #[test]
    fn apply_when_arithmetic_initializer_then_folds_to_literal() {
        let lib = parse(
            "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_named_constant_initializer_then_substitutes_and_folds() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                PI : LREAL := 4.25;
            END_VAR
            PROGRAM main
            VAR
                d2r : LREAL := PI/180.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_nested_arithmetic_then_folds_completely() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                PI : LREAL := 4.25;
            END_VAR
            PROGRAM main
            VAR
                asec2r : LREAL := PI/(180.0*3600.0);
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "asec2r");
        assert!((real_value(var) - (4.25 / (180.0 * 3600.0))).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_reference_to_non_constant_then_error() {
        let lib = parse(
            "
            PROGRAM main
            VAR
                scale : LREAL := 2.0;
                d2r : LREAL := scale/180.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let result = apply(lib, &opts());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_flag_disabled_then_error_even_if_foldable() {
        let lib = parse(
            "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM",
            &opts(),
        );
        let result = apply(lib, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_bare_literal_initializer_then_unchanged() {
        let lib = parse(
            "PROGRAM main VAR x : LREAL := 4.25; END_VAR END_PROGRAM",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "x");
        assert!((real_value(var) - 4.25).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_function_local_constant_then_resolves() {
        let lib = parse(
            "
            FUNCTION my_func : LREAL
            VAR CONSTANT
                SCALE : LREAL := 2.0;
            END_VAR
            VAR
                d2r : LREAL := SCALE*180.0;
            END_VAR
            my_func := d2r;
            END_FUNCTION
        ",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - (2.0 * 180.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_fb_local_constant_not_visible_in_other_fb_then_error() {
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR CONSTANT
                LOCAL_SCALE : LREAL := 2.0;
            END_VAR
            END_FUNCTION_BLOCK
            FUNCTION_BLOCK fb2
            VAR
                d2r : LREAL := LOCAL_SCALE*180.0;
            END_VAR
            END_FUNCTION_BLOCK
        ",
            &opts(),
        );
        let result = apply(lib, &opts());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_integer_arithmetic_then_folds_to_integer_literal() {
        let lib = parse(
            "PROGRAM main VAR x : DINT := 2+3; END_VAR END_PROGRAM",
            &opts(),
        );
        let lib = apply(lib, &opts()).unwrap();
        let var = find_var_decl(&lib, "x");
        let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
        let lit = cast!(
            simple.initial_value.as_ref().unwrap(),
            ConstantKind::IntegerLiteral
        );
        assert_eq!(lit.value.value.value, 5);
    }
}
