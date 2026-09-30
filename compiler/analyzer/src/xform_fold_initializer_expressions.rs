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

impl Value for ConstantKind {}

pub fn apply(
    lib: Library,
    options: &CompilerOptions,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut folder = InitializerFolder {
        constants: collect_constants(&lib),
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
fn collect_constants(lib: &Library) -> ScopedTable<'static, Id, ConstantKind> {
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
fn register_constants(constants: &mut ScopedTable<Id, ConstantKind>, decls: &[VarDecl]) {
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
                constants.try_add(&name, value.clone());
            }
        }
    }
}

/// Recursively substitutes known constant references and folds arithmetic
/// within an initializer's expression tree. Reuses the same binary/unary
/// folding rules as `xform_fold_constant_expressions`.
///
/// Cannot recurse into a cycle: `constants` only ever holds already-literal
/// `ConstantKind` values (see `register_constants`), never an expression
/// referencing another name, so a substituted value is always terminal --
/// there is nothing left to look up again.
///
/// Returns `Err` if a sub-expression is a genuine constant expression
/// (both operands known) whose operation has no defined result (division
/// by zero, overflow) -- distinct from simply not folding, which leaves
/// the node as an unfolded `BinaryOp`/`UnaryOp` for `normalize` to report
/// as "not a constant expression".
fn substitute_and_fold(
    expr: Expr,
    constants: &mut ScopedTable<Id, ConstantKind>,
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
                Some(value) => ExprKind::Const(value.clone()),
                None => ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))),
            }
        }
        // Usually already resolved to `Variable` by
        // xform_resolve_late_bound_expr_kind (which runs before this pass
        // in the normal pipeline), but handled here too so this pass does
        // not depend on that ordering.
        ExprKind::LateBound(late_bound) => match constants.find(&late_bound.value) {
            Some(value) => ExprKind::Const(value.clone()),
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
    constants: ScopedTable<'static, Id, ConstantKind>,
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
            // Still fold when possible: the P4037 above already fails the
            // build, but keeping a foldable value prevents a misleading
            // cascade on `VAR CONSTANT` declarations, which would otherwise
            // be diagnosed as *uninitialized* when they plainly carry an
            // initializer.
            let type_name = se.type_name;
            let initial_value = match substitute_and_fold(se.initial_value, &mut self.constants) {
                Ok(folded) => match folded.kind {
                    ExprKind::Const(c) => Some(c),
                    _ => None,
                },
                Err(_) => None,
            };
            return InitialValueAssignmentKind::Simple(SimpleInitializer {
                type_name,
                initial_value,
            });
        }

        let type_name = se.type_name;
        match substitute_and_fold(se.initial_value, &mut self.constants) {
            Ok(folded) => match folded.kind {
                ExprKind::Const(c) => InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name,
                    initial_value: Some(c),
                }),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::InitializerNotConstantExpression,
                            Label::span(folded.span(), "Initializer expression"),
                        )
                        .with_context("type", &type_name.to_string()),
                    );
                    InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name,
                        initial_value: None,
                    })
                }
            },
            Err(diag) => {
                self.diagnostics.push(diag);
                InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name,
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

        let variables = match node {
            ScopeNode::Function(node) => &node.variables,
            ScopeNode::FunctionBlock(node) => &node.variables,
            ScopeNode::Program(node) => &node.variables,
            ScopeNode::Method(node) => &node.variables,
        };
        register_constants(&mut self.constants, variables);

        Ok(())
    }

    fn exit_scope(&mut self) {
        self.constants.exit();
    }
}

#[cfg(test)]
mod tests;
