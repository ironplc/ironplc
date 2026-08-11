//! Transform that rewrites the TwinCAT/CODESYS `ADR(x)` address-of operator
//! into the reference address-of expression the backend already understands.
//!
//! `ADR(x)` parses as an ordinary function call (like `SIZEOF` — no keyword
//! token). Its return type depends on its argument (`POINTER TO typeof(x)`),
//! which a stdlib function signature cannot express, so the call is rewritten
//! early instead: `Function("ADR", [variable])` becomes
//! `ExprKind::Ref(variable)`, and all existing reference type inference,
//! assignment checking (P2032), semantic rules, and codegen apply unchanged.
//! The reference semantic rules (`rule_ref_to`) validate `ExprKind::Ref`
//! nodes unconditionally, so the rewritten node is checked without requiring
//! `allow_ref_to` — as in the `twincat` dialect.
//!
//! An `ADR` call whose operand is not a variable expression (a literal, a
//! call result, or a wrong number of arguments) is diagnosed with P2028 and
//! lowered to `NULL` so the recognized-but-invalid call does not cascade into
//! a misleading "undeclared function" diagnostic. Operands that are variables
//! but not addressable (array elements, struct fields, ephemeral variables)
//! are rewritten and left to `rule_ref_to` to reject (P2028/P2029/P2030),
//! reusing the `REF()` operand rules.
//!
//! Runs after late-bound expression resolution (so bare identifiers are
//! already `ExprKind::Variable`) and after the implicit-dereference transform
//! (so a `REFERENCE TO` operand is not silently mis-addressed), but before
//! symbol/function resolution (so a recognized `ADR` is not reported as an
//! undeclared function). With `allow_adr` off the transform is a no-op and
//! `ADR(x)` falls through to the normal undeclared-function path (P4017).
//!
//! See `specs/design/adr-and-pointer-to.md`.

use ironplc_dsl::common::Library;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

/// The address-of operator, recognized only when `allow_adr` is set.
const ADR: &str = "ADR";

pub fn apply(
    lib: Library,
    options: &CompilerOptions,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    if !options.allow_adr {
        return Ok((lib, Vec::new()));
    }
    let mut resolver = ResolveAdr {
        diagnostics: Vec::new(),
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e])?;
    Ok((result, resolver.diagnostics))
}

struct ResolveAdr {
    diagnostics: Vec<Diagnostic>,
}

/// If `func` is an `ADR(...)` call with exactly one positional input
/// argument, returns that argument expression.
fn adr_operand(func: &Function) -> Option<Expr> {
    if func.param_assignment.len() != 1 {
        return None;
    }
    match &func.param_assignment[0] {
        ParamAssignmentKind::PositionalInput(pos) => Some(pos.expr.clone()),
        _ => None,
    }
}

impl Fold<Diagnostic> for ResolveAdr {
    fn fold_expr_kind(&mut self, node: ExprKind) -> Result<ExprKind, Diagnostic> {
        match node {
            ExprKind::Function(func) if func.name.to_string().eq_ignore_ascii_case(ADR) => {
                // Fold the arguments first so a nested `ADR` is rewritten
                // before the outer operand is inspected.
                let func = func.recurse_fold(self)?;
                let span = func.name.span();
                match adr_operand(&func) {
                    Some(operand) => match operand.kind {
                        // Any variable expression becomes an address-of node;
                        // `rule_ref_to` rejects non-addressable variables
                        // (array elements, struct fields, ephemerals) with
                        // the same codes `REF()` uses.
                        ExprKind::Variable(var) => Ok(ExprKind::Ref(Box::new(var))),
                        _ => {
                            self.diagnostics.push(Diagnostic::problem(
                                Problem::RefOperandNotVariable,
                                Label::span(operand.span(), "ADR() operand must be a variable"),
                            ));
                            Ok(ExprKind::Null(operand.span()))
                        }
                    },
                    None => {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::RefOperandNotVariable,
                            Label::span(span.clone(), "ADR() requires exactly one input argument"),
                        ));
                        Ok(ExprKind::Null(span))
                    }
                }
            }
            other => other.recurse_fold(self),
        }
    }
}
