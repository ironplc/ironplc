//! Transform that folds constant-expression `VAR` initializers into plain
//! literal initializers.
//!
//! The IEC 61131-3 standard's `constant()` grammar production only permits
//! a bare literal in a `VAR` initializer position (e.g. `x : LREAL := 4.25;`).
//! Real CODESYS/TwinCAT code commonly uses a constant *expression* instead
//! (e.g. `scaled : LREAL := SCALE*4.0;`). The parser accepts this broader
//! form unconditionally, producing `InitialValueAssignmentKind::SimpleExpr`
//! — a placeholder that this pass always normalizes away before any other
//! semantic pass runs, so that no downstream pass ever sees `SimpleExpr`.
//! Two independent questions decide what the normalized `Simple` looks
//! like:
//!
//! - **Is the expression allowed here?** With
//!   `--allow-constant-initializer-expressions` disabled, every `SimpleExpr`
//!   is diagnosed (P4037) whatever it contains. With it enabled, only an
//!   expression that fails to reduce is diagnosed (P4038, or P4039/P4040
//!   when the fold itself has no defined result).
//! - **Does it reduce?** Substituting references to known
//!   `CONSTANT`-qualified declarations and then folding arithmetic either
//!   yields a constant or does not. When it does, the resulting `Simple`
//!   carries that value — *including* when the flag is off and P4037 has
//!   already failed the build. Only an expression that does not reduce
//!   normalizes to an uninitialized `Simple`.
//!
//! Folding an initializer the flag has already rejected looks pointless,
//! since the build fails either way. It is what stops a `VAR CONSTANT` from
//! *also* being reported as uninitialized (P4008) when it plainly carries an
//! initializer; that cascade belongs to the unfoldable case alone, in either
//! flag state.
//!
//! One asymmetry follows from the order the disabled-flag arm works in: it
//! diagnoses first, then folds, and keeps only the fold's outcome, not its
//! error. So `bad : INT := 10/ZERO` reports P4037 alone there, where the
//! enabled arm reports P4039.
//!
//! An initializer that is exactly one constant's name (`x : UDINT := C;`)
//! is the smallest constant expression. Substituting the value leaves a
//! literal, and the literal checks that follow would accept it for any type
//! of the same family (`C : UDINT` initializing an `INT`), although the same
//! `C` assigned in a statement is rejected. So the enabled arm first checks
//! the constant's declared type against the variable's, with the relation an
//! assignment uses, and reports P4022 on a mismatch; the initializer is then
//! normalized to an uninitialized `Simple`, as for an expression that does
//! not reduce. A longer expression has no declared type to check and is
//! accepted on its value alone, as before.
//!
//! The same extension serves two more positions. A `TYPE` default
//! (`TYPE MyInt : UDINT := C;`) is a `SimpleExpr` like a variable's and
//! folds the same way. A string initializer (`s : STRING[20] := NAME;`) is
//! a `StringExpr`, which keeps the length a `SimpleExpr` cannot, and is
//! normalized to `String` with the literal its expression folds to; a value
//! that folds to anything but a string is reported as P4022. A `CONSTANT`
//! string is a named constant like any other.
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

use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::constant_folding::{fold_error_to_diagnostic, try_fold_binary, try_fold_unary};
use crate::scoped_table::{ScopedTable, Value};
use crate::type_compat::are_types_compatible;
use crate::type_environment::TypeEnvironment;

/// A `CONSTANT` declaration whose value is a literal: the literal, and the
/// type the constant was declared with.
#[derive(Clone, Debug)]
struct NamedConstant {
    type_name: TypeName,
    value: ConstantKind,
}

impl Value for NamedConstant {}

pub fn apply(
    lib: Library,
    types: &TypeEnvironment,
    options: &CompilerOptions,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut folder = InitializerFolder {
        constants: collect_constants(&lib),
        types,
        options,
        diagnostics: Vec::new(),
    };

    // Diagnostics ride along with the normalized library rather than failing
    // the transform: every `SimpleExpr` is normalized away even when it is
    // diagnosed, so later passes must run over this result. Reverting to the
    // pre-transform library on a per-declaration diagnostic would leak
    // `SimpleExpr` nodes downstream (P9998 in
    // rule_var_decl_const_initialized).
    match folder.fold_library(lib) {
        Ok(result) => Ok((result, folder.diagnostics)),
        Err(e) => {
            let mut diagnostics = folder.diagnostics;
            diagnostics.push(e);
            Err(diagnostics)
        }
    }
}

/// Scan the library for top-level (`VAR_GLOBAL`) constant declarations with
/// literal values.
///
/// Deliberately narrowed to true globals only -- `CONFIGURATION`/`RESOURCE`
/// global vars are scoped to their own configuration or resource (not
/// visible everywhere the way a top-level `VAR_GLOBAL` is), which this pass
/// does not yet model. Handling those "half global" vars correctly is
/// left for a follow-up rather than treating them as unconditionally
/// global here.
fn collect_constants(lib: &Library) -> ScopedTable<'static, Id, NamedConstant> {
    let mut constants = ScopedTable::new();

    for element in &lib.elements {
        if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
            register_constants(&mut constants, decls);
        }
    }

    constants
}

/// Registers each `CONSTANT`-qualified, literal-valued declaration in
/// `decls` into the current (innermost) scope of `constants`. A name
/// already present *in that same scope* keeps its first value: the repeat
/// is the symbol environment's to report (P4014), so it is not diagnosed a
/// second time from this table. Shadowing an outer scope's constant (e.g.
/// a function-local constant with the same name as a global) is unaffected,
/// since that lives in a different scope entirely.
fn register_constants<'a>(
    constants: &mut ScopedTable<Id, NamedConstant>,
    decls: impl IntoIterator<Item = &'a VarDecl>,
) {
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

        // A string constant is its literal, declared with the string type's
        // keyword: the length takes no part in whether a constant may
        // initialize a variable.
        let constant = match &decl.initializer {
            InitialValueAssignmentKind::Simple(simple) => {
                simple.initial_value.clone().map(|value| NamedConstant {
                    type_name: simple.type_name.clone(),
                    value,
                })
            }
            InitialValueAssignmentKind::String(string) => {
                string.initial_value.clone().map(|literal| NamedConstant {
                    type_name: TypeName::from(string.width.keyword()),
                    value: ConstantKind::CharacterString(literal),
                })
            }
            _ => None,
        };
        if let Some(constant) = constant {
            constants.try_add(&name, constant);
        }
    }
}

/// Recursively substitutes known constant references and folds arithmetic
/// within an initializer's expression tree. Reuses the same binary/unary
/// folding rules as `xform_fold_constant_expressions`.
///
/// Cannot recurse into a cycle: `constants` only ever holds already-literal
/// values (see `register_constants`), never an expression referencing
/// another name, so a substituted value is always terminal -- there is
/// nothing left to look up again.
///
/// A substituted literal takes the span of the reference it replaces, so
/// that a diagnostic on the value (a range check, say) points at the
/// initializer that uses the constant rather than at the declaration that
/// defines it, and two initializers using the same constant are reported
/// separately.
///
/// Returns `Err` if a sub-expression is a genuine constant expression
/// (both operands known) whose operation has no defined result (division
/// by zero, overflow) -- distinct from simply not folding, which leaves
/// the node as an unfolded `BinaryOp`/`UnaryOp` for `normalize` to report
/// as "not a constant expression".
fn substitute_and_fold(
    expr: Expr,
    constants: &mut ScopedTable<Id, NamedConstant>,
) -> Result<Expr, Diagnostic> {
    let span = expr.span();
    let kind = match expr.kind {
        ExprKind::BinaryOp(binary) => {
            let left = substitute_and_fold(binary.left, constants)?;
            let right = substitute_and_fold(binary.right, constants)?;
            let binary = BinaryExpr {
                op: binary.op,
                left,
                right,
            };
            try_fold_binary(&binary)
                .map_err(|e| fold_error_to_diagnostic(e, span))?
                .unwrap_or(ExprKind::BinaryOp(Box::new(binary)))
        }
        ExprKind::UnaryOp(unary) => {
            let term = substitute_and_fold(unary.term, constants)?;
            let unary = UnaryExpr { op: unary.op, term };
            try_fold_unary(&unary).unwrap_or(ExprKind::UnaryOp(Box::new(unary)))
        }
        ExprKind::Expression(inner) => {
            ExprKind::Expression(Box::new(substitute_and_fold(*inner, constants)?))
        }
        ExprKind::Deref(inner) => {
            ExprKind::Deref(Box::new(substitute_and_fold(*inner, constants)?))
        }
        ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
            match constants.find(&named.name) {
                Some(constant) => ExprKind::Const(constant.value.clone().with_span(span)),
                None => ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))),
            }
        }
        // Usually already resolved to `Variable` by
        // xform_resolve_late_bound_expr_kind (which runs before this pass
        // in the normal pipeline), but handled here too so this pass does
        // not depend on that ordering.
        ExprKind::LateBound(late_bound) => match constants.find(&late_bound.value) {
            Some(constant) => ExprKind::Const(constant.value.clone().with_span(span)),
            None => ExprKind::LateBound(late_bound),
        },
        other => other,
    };

    Ok(Expr {
        kind,
        expr_type: expr.expr_type,
        span: expr.span,
    })
}

struct InitializerFolder<'a> {
    constants: ScopedTable<'static, Id, NamedConstant>,
    types: &'a TypeEnvironment,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl InitializerFolder<'_> {
    /// The declared type of the constant that `e` names, when `e` is exactly
    /// one name (`C`, not `C + 1`) and that name is a known constant.
    fn named_constant_type(&self, e: &Expr) -> Option<&TypeName> {
        let name = match &e.kind {
            ExprKind::LateBound(late_bound) => &late_bound.value,
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
                &named.name
            }
            _ => return None,
        };
        self.constants
            .find(name)
            .map(|constant| &constant.type_name)
    }

    /// Whether a constant declared with `constant_type` may initialize a
    /// variable declared with `variable_type`: the relation an assignment
    /// uses, asked of the elementary types the names resolve to, so an
    /// alias on either side compares as the type it names. A type that does
    /// not resolve to an elementary type is not checked here: an undeclared
    /// type is reported by the resolver, and a constant of an enumerated or
    /// a subrange type is the business of the passes that check those.
    fn constant_type_fits(&self, variable_type: &TypeName, constant_type: &TypeName) -> bool {
        match (
            self.types.resolve_elementary_type_name(variable_type),
            self.types.resolve_elementary_type_name(constant_type),
        ) {
            (Some(expected), Some(actual)) => {
                are_types_compatible(&expected, &actual, self.options)
            }
            _ => true,
        }
    }

    /// The literal that `initial_value`, written against `type_name`,
    /// folds to, or `None` with the reason diagnosed: P4037 when the dialect
    /// does not allow a constant expression here, P4022 when the expression
    /// is a constant whose declared type the position does not accept,
    /// P4038 when it does not reduce, or the fold's own error.
    fn fold_value(&mut self, type_name: &TypeName, initial_value: Expr) -> Option<ConstantKind> {
        if !self.options.allow_constant_initializer_expressions {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::ConstantInitializerExpressionNotAllowed,
                    Label::span(initial_value.span(), "Constant expression initializer"),
                )
                .with_context("type", &type_name.to_string()),
            );
            // Still fold when possible: the P4037 above already fails the
            // build, but keeping a foldable value prevents a misleading
            // cascade on `VAR CONSTANT` declarations, which would otherwise
            // be diagnosed as *uninitialized* when they plainly carry an
            // initializer.
            return match substitute_and_fold(initial_value, &mut self.constants) {
                Ok(folded) => match folded.kind {
                    ExprKind::Const(c) => Some(c),
                    _ => None,
                },
                Err(_) => None,
            };
        }

        // A bare constant name is checked by declared type before its value
        // replaces it (see the module documentation).
        let mismatch = self
            .named_constant_type(&initial_value)
            .filter(|constant_type| !self.constant_type_fits(type_name, constant_type))
            .cloned();
        if let Some(constant_type) = mismatch {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::InitializerTypeMismatch,
                    Label::span(
                        initial_value.span(),
                        format!("Constant of type {constant_type}"),
                    ),
                )
                .with_context("type", &type_name.to_string()),
            );
            return None;
        }

        match substitute_and_fold(initial_value, &mut self.constants) {
            Ok(folded) => match folded.kind {
                ExprKind::Const(c) => Some(c),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::InitializerNotConstantExpression,
                            Label::span(folded.span(), "Initializer expression"),
                        )
                        .with_context("type", &type_name.to_string()),
                    );
                    None
                }
            },
            Err(diag) => {
                self.diagnostics.push(diag);
                None
            }
        }
    }

    /// Normalizes a `SimpleExprInitializer` back to `Simple`, folding it if
    /// possible and emitting a diagnostic otherwise. Always returns
    /// `Simple` so that no other pass ever observes `SimpleExpr`.
    fn normalize(&mut self, se: SimpleExprInitializer) -> InitialValueAssignmentKind {
        let initial_value = self.fold_value(&se.type_name, se.initial_value);
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name: se.type_name,
            initial_value,
        })
    }

    /// Normalizes a `StringExprInitializer` to `String`, as `normalize` does
    /// a `SimpleExpr`. The value has to fold to a string literal: a bare
    /// constant of another type is reported by `fold_value`, and an
    /// expression that folds to a value of another kind is reported here.
    fn normalize_string(&mut self, se: StringExprInitializer) -> InitialValueAssignmentKind {
        let type_name = se.type_name();
        let initial_value = match self.fold_value(&type_name, se.initial_value) {
            Some(ConstantKind::CharacterString(literal)) => Some(literal),
            Some(other) => {
                // Without the flag, P4037 is already reported and the fold
                // only avoids a cascade, so nothing more is said about the
                // value.
                if self.options.allow_constant_initializer_expressions {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::InitializerTypeMismatch,
                            Label::span(other.span(), "Value is not a string"),
                        )
                        .with_context("type", &type_name.to_string()),
                    );
                }
                None
            }
            None => None,
        };
        InitialValueAssignmentKind::String(StringInitializer {
            length: se.length,
            width: se.width,
            initial_value,
            keyword_span: se.keyword_span,
        })
    }
}

impl Fold<Diagnostic> for InitializerFolder<'_> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            InitialValueAssignmentKind::SimpleExpr(se) => Ok(self.normalize(se)),
            InitialValueAssignmentKind::StringExpr(se) => Ok(self.normalize_string(se)),
            other => InitialValueAssignmentKind::recurse_fold(other, self),
        }
    }

    /// Opens the scope of a declaration and registers the constants it
    /// declares, so that a `VAR CONSTANT` is visible to initializers
    /// within the declaration and to nothing outside it.
    ///
    /// The match is exhaustive because every kind registers the same
    /// thing: should a new kind of scope not want its constants
    /// registered, that has to be said here rather than inferred from an
    /// absent arm.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.constants.enter();

        match node {
            ScopeNode::Function(node) => register_constants(&mut self.constants, &node.variables),
            ScopeNode::FunctionBlock(node) => {
                register_constants(&mut self.constants, &node.variables)
            }
            ScopeNode::Program(node) => register_constants(&mut self.constants, &node.variables),
            ScopeNode::Method(node) => {
                register_constants(&mut self.constants, node.all_variables())
            }
            ScopeNode::MethodPrototype(node) => {
                register_constants(&mut self.constants, &node.variables)
            }
            // An interface declares no variables of its own.
            ScopeNode::Interface(_) => {}
        }

        Ok(())
    }

    fn exit_scope(&mut self) {
        self.constants.exit();
    }
}

#[cfg(test)]
mod named_constant_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_environment::TypeEnvironmentBuilder;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use ironplc_parser::parse_program;
    use ironplc_test::cast;

    fn opts() -> CompilerOptions {
        CompilerOptions::from_dialect(Dialect::Rusty)
    }

    /// The elementary types alone: these tests declare no user types.
    fn elementary_types() -> TypeEnvironment {
        TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap()
    }

    fn parse(src: &str, options: &CompilerOptions) -> Library {
        parse_program(src, &FileId::default(), options).unwrap()
    }

    pub(super) fn find_var_decl<'a>(lib: &'a Library, var_name: &str) -> &'a VarDecl {
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

    /// Applies the transform expecting no diagnostics; returns the library.
    fn apply_clean(lib: Library, options: &CompilerOptions) -> Library {
        let (lib, diagnostics) = apply(lib, &elementary_types(), options).unwrap();
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        lib
    }

    /// Applies the transform expecting diagnostics; returns them.
    fn apply_expect_diagnostics(lib: Library, options: &CompilerOptions) -> Vec<Diagnostic> {
        let (_, diagnostics) = apply(lib, &elementary_types(), options).unwrap();
        assert!(!diagnostics.is_empty(), "expected diagnostics");
        diagnostics
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
        let lib = apply_clean(lib, &opts());
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
        let lib = apply_clean(lib, &opts());
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
        let lib = apply_clean(lib, &opts());
        let var = find_var_decl(&lib, "asec2r");
        assert!((real_value(var) - (4.25 / (180.0 * 3600.0))).abs() < f64::EPSILON);
    }

    /// A repeated global constant keeps its first value here; the repeat is
    /// the symbol environment's to report, so this pass stays clean.
    #[test]
    fn apply_when_duplicate_global_constant_then_first_value_kept() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                SCALE : LREAL := 2.0;
            END_VAR
            VAR_GLOBAL CONSTANT
                SCALE : LREAL := 3.0;
            END_VAR
            PROGRAM main
            VAR
                d2r : LREAL := SCALE*180.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let lib = apply_clean(lib, &opts());
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - 360.0).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_constant_reference_different_case_then_resolves() {
        // Constant lookup is case-insensitive, matching Id's own
        // case-insensitive Hash/Eq (per the IEC 61131-3 spec) -- reusing
        // Id as the table key gets this for free, no manual
        // to_uppercase() needed.
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                scale : LREAL := 2.0;
            END_VAR
            PROGRAM main
            VAR
                d2r : LREAL := SCALE*180.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let lib = apply_clean(lib, &opts());
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - (2.0 * 180.0)).abs() < f64::EPSILON);
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
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
    }

    #[test]
    fn apply_when_flag_disabled_then_error_even_if_foldable() {
        let lib = parse(
            "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM",
            &opts(),
        );
        let (lib, diagnostics) =
            apply(lib, &elementary_types(), &CompilerOptions::default()).unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d.code == Problem::ConstantInitializerExpressionNotAllowed.code()));
        // The initializer is still folded best-effort so that a `VAR
        // CONSTANT` declaration is not additionally (and misleadingly)
        // diagnosed as uninitialized by a downstream rule.
        let var = find_var_decl(&lib, "d2r");
        assert!((real_value(var) - (4.25 / 180.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_when_bare_literal_initializer_then_unchanged() {
        let lib = parse(
            "PROGRAM main VAR x : LREAL := 4.25; END_VAR END_PROGRAM",
            &opts(),
        );
        let lib = apply_clean(lib, &opts());
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
        let lib = apply_clean(lib, &opts());
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
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
    }

    #[test]
    fn apply_when_mutual_reference_cycle_then_error_not_hang() {
        // A references B and B references A. `constants` only ever admits
        // already-literal values (collected before any substitution runs),
        // so neither A nor B is ever registered as a known constant here --
        // both fail to fold, and a downstream reference to either also
        // fails. This is a regression test proving that shape terminates
        // with diagnostics rather than recursing indefinitely.
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                A : LREAL := B+0.0;
                B : LREAL := A+0.0;
            END_VAR
            PROGRAM main
            VAR
                x : LREAL := A+0.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert_eq!(diagnostics.len(), 3);
    }

    #[test]
    fn apply_when_self_reference_cycle_then_error_not_hang() {
        // A constant whose initializer references itself (`A := A + 0.0`).
        // This is the degenerate one-node cycle. `A` is never registered as
        // a known constant (only bare-literal `Simple` initializers are), so
        // the self-reference does not resolve, the expression does not fold,
        // and a P4037 diagnostic is emitted. The test completing at all is
        // the proof of termination -- there is no bounded recursion here
        // because substitution can only ever replace a name with a literal,
        // never with another name-bearing expression.
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                A : LREAL := A+0.0;
            END_VAR
            PROGRAM main
            VAR
                x : LREAL := A+0.0;
            END_VAR
            END_PROGRAM
        ",
            &opts(),
        );
        let errors = apply_expect_diagnostics(lib, &opts());
        // One diagnostic for A's own initializer, one for x referencing A.
        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
    }

    #[test]
    fn apply_when_three_node_cycle_then_error_not_hang() {
        // A longer cycle A -> B -> C -> A. Same reasoning as the mutual
        // (two-node) case: none of A, B, C is a bare literal, so none is
        // registered, every initializer fails to fold, and each emits a
        // diagnostic. Demonstrates termination is independent of cycle
        // length -- the pass never chases references transitively.
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                A : LREAL := B+0.0;
                B : LREAL := C+0.0;
                C : LREAL := A+0.0;
            END_VAR
        ",
            &opts(),
        );
        let errors = apply_expect_diagnostics(lib, &opts());
        assert_eq!(errors.len(), 3);
        assert!(errors
            .iter()
            .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
    }

    #[test]
    fn apply_when_integer_arithmetic_then_folds_to_integer_literal() {
        let lib = parse(
            "PROGRAM main VAR x : DINT := 2+3; END_VAR END_PROGRAM",
            &opts(),
        );
        let lib = apply_clean(lib, &opts());
        let var = find_var_decl(&lib, "x");
        let simple = cast!(&var.initializer, InitialValueAssignmentKind::Simple);
        let lit = cast!(
            simple.initial_value.as_ref().unwrap(),
            ConstantKind::IntegerLiteral
        );
        assert_eq!(lit.value.value.value, 5);
    }

    #[test]
    fn apply_when_initializer_divides_by_zero_then_division_by_zero_error_not_misleading_p4038() {
        // 1.0/0.0 is a genuine constant expression (both operands known);
        // it must be reported as "division by zero", not misdiagnosed as
        // "not a constant expression" (P4038 / InitializerNotConstantExpression).
        let lib = parse(
            "PROGRAM main VAR x : LREAL := 1.0/0.0; END_VAR END_PROGRAM",
            &opts(),
        );
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionDivisionByZero.code()));
    }

    #[test]
    fn apply_when_initializer_int_overflows_then_overflow_error_not_misleading_p4038() {
        let lib = parse(
            "PROGRAM main VAR x : LINT := 170141183460469231731687303715884105727 * 2; END_VAR END_PROGRAM",
            &opts(),
        );
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionOverflow.code()));
    }

    #[test]
    fn apply_when_initializer_real_overflows_then_overflow_error_not_misleading_p4038() {
        let lib = parse(
            "PROGRAM main VAR x : LREAL := 1.0E300 * 1.0E300; END_VAR END_PROGRAM",
            &opts(),
        );
        let diagnostics = apply_expect_diagnostics(lib, &opts());
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionOverflow.code()));
    }

    /// A method's `VAR CONSTANT` belongs to that method. Before the
    /// method scope existed every method's constants were registered in
    /// the enclosing function block's scope, so a sibling could fold
    /// against them. See
    /// https://github.com/ironplc/ironplc/issues/1439.
    #[test]
    fn apply_when_method_local_constant_not_visible_in_sibling_method_then_error() {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..opts()
        };
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR
                x : LREAL;
            END_VAR
            METHOD m1
            VAR CONSTANT
                LOCAL_SCALE : LREAL := 2.0;
            END_VAR
                x := 1.0;
            END_METHOD
            METHOD m2
            VAR
                d2r : LREAL := LOCAL_SCALE*180.0;
            END_VAR
                x := 2.0;
            END_METHOD
            END_FUNCTION_BLOCK
        ",
            &options,
        );
        let diagnostics = apply_expect_diagnostics(lib, &options);
        assert!(diagnostics
            .iter()
            .all(|d| d.code == Problem::InitializerNotConstantExpression.code()));
    }
}
