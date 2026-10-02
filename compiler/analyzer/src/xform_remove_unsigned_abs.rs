//! Transform that removes `ABS` applied to an unsigned integer.
//!
//! IEC 61131-3 declares `ABS` on `ANY_NUM`, and the absolute value of an
//! unsigned value is the value itself. Removing the call here, before code
//! generation, means no back end has to know that: a back end that lowered
//! it with its signed absolute value would read a large unsigned value as
//! negative.
//!
//! This runs after `xform_resolve_expr_types`, which gives the argument its
//! type.
//!
//! ## Before
//!
//! ```ignore
//! y := ABS(x);   (* x : UDINT *)
//! ```
//!
//! ## After
//!
//! ```ignore
//! y := x;
//! ```
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;

use crate::intermediate_type::IntermediateType;
use crate::type_environment::TypeEnvironment;

pub fn apply(lib: Library, type_environment: &TypeEnvironment) -> Result<Library, Vec<Diagnostic>> {
    let mut remover = UnsignedAbsRemover { type_environment };
    remover.fold_library(lib).map_err(|e| vec![e])
}

struct UnsignedAbsRemover<'a> {
    type_environment: &'a TypeEnvironment,
}

impl UnsignedAbsRemover<'_> {
    /// The argument of `node` when it is `ABS` of an unsigned integer.
    fn unsigned_abs_argument(&self, node: &Expr) -> Option<Expr> {
        let ExprKind::Function(function) = &node.kind else {
            return None;
        };
        if !function.name.original().eq_ignore_ascii_case("ABS") {
            return None;
        }
        let [ParamAssignmentKind::PositionalInput(input)] = function.param_assignment.as_slice()
        else {
            return None;
        };
        match self.type_environment.representation_of_expr(&input.expr)? {
            IntermediateType::UInt { .. } => Some(input.expr.clone()),
            _ => None,
        }
    }
}

impl Fold<Diagnostic> for UnsignedAbsRemover<'_> {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        let node = Expr::recurse_fold(node, self)?;
        Ok(self.unsigned_abs_argument(&node).unwrap_or(node))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::textual::*;
    use ironplc_dsl::visitor::Visitor;
    use std::convert::Infallible;

    /// Counts the calls to `ABS` in a library after analysis.
    #[derive(Default)]
    struct AbsCalls(usize);

    impl Visitor<Infallible> for AbsCalls {
        type Value = ();
        fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
            if node.name.original().eq_ignore_ascii_case("ABS") {
                self.0 += 1;
            }
            node.recurse_visit(self)
        }
    }

    fn abs_calls(source: &str) -> usize {
        let library = parse_and_resolve_types(source);
        let mut calls = AbsCalls::default();
        let Ok(()) = calls.walk(&library);
        calls.0
    }

    #[test]
    fn apply_when_abs_of_udint_then_call_removed() {
        assert_eq!(
            0,
            abs_calls("PROGRAM main VAR x : UDINT; y : UDINT; END_VAR y := ABS(x); END_PROGRAM")
        );
    }

    #[test]
    fn apply_when_abs_of_ulint_nested_then_call_removed() {
        assert_eq!(
            0,
            abs_calls(
                "PROGRAM main VAR x : ULINT; y : ULINT; END_VAR y := ABS(x) + ABS(x); END_PROGRAM"
            )
        );
    }

    #[test]
    fn apply_when_abs_of_dint_then_call_kept() {
        assert_eq!(
            1,
            abs_calls("PROGRAM main VAR x : DINT; y : DINT; END_VAR y := ABS(x); END_PROGRAM")
        );
    }

    #[test]
    fn apply_when_abs_of_real_then_call_kept() {
        assert_eq!(
            1,
            abs_calls("PROGRAM main VAR x : REAL; y : REAL; END_VAR y := ABS(x); END_PROGRAM")
        );
    }
}
