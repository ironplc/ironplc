//! Transform that removes `ABS` applied to an unsigned integer or to an
//! integer literal.
//!
//! IEC 61131-3 declares `ABS` on `ANY_NUM`, and the absolute value of an
//! unsigned value is the value itself. Removing the call here, before code
//! generation, means no back end has to know that: a back end that lowered
//! it with its signed absolute value would read a large unsigned value as
//! negative. So is removed `ABS` of a subrange of an unsigned integer.
//!
//! The absolute value of an integer literal is known here, so the call is
//! replaced by that constant: `ABS(-5)` is `5` and `ABS(DINT#-5)` is
//! `DINT#5`. An untyped literal has no type yet, since the conversions that
//! settle it run later, but whatever type it settles to, its absolute value
//! is the literal without its sign. A constant that its type cannot hold,
//! such as the `SINT#128` of `ABS(SINT#-128)`, is reported by
//! `rule_constant_range`, as `127 + 1` stored in a `SINT` is.
//!
//! This runs after `xform_resolve_expr_types`, which gives the argument its
//! type, and after `xform_fold_constant_expressions`, which folds `-5` into a
//! literal.
//!
//! ## Before
//!
//! ```ignore
//! y := ABS(x);   (* x : UDINT *)
//! z := ABS(-5);
//! ```
//!
//! ## After
//!
//! ```ignore
//! y := x;
//! z := 5;
//! ```
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;

use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;

pub fn apply(lib: Library, type_environment: &TypeEnvironment) -> Result<Library, Vec<Diagnostic>> {
    let mut remover = UnsignedAbsRemover { type_environment };
    remover.fold_library(lib).map_err(|e| vec![e])
}

struct UnsignedAbsRemover<'a> {
    type_environment: &'a TypeEnvironment,
}

impl UnsignedAbsRemover<'_> {
    /// What `node` is without its call when it is `ABS` of an integer
    /// literal, of an unsigned integer or of a subrange of one.
    fn without_abs(&self, node: &Expr) -> Option<Expr> {
        let ExprKind::Function(function) = &node.kind else {
            return None;
        };
        if !function.name.original().eq_ignore_ascii_case("ABS") {
            return None;
        }
        let [arg] = function.param_assignment.as_slice() else {
            return None;
        };
        let input = arg.input_expr()?;
        if let ExprKind::Const(ConstantKind::IntegerLiteral(literal)) = &input.kind {
            return Some(absolute_literal(literal, input, node));
        }
        let unsigned = self
            .type_environment
            .representation_of_expr(input)
            .is_some_and(|ty| matches!(ty.operated_as(), SemanticType::UInt { .. }));
        unsigned.then(|| input.clone())
    }
}

/// The absolute value of `literal`, the argument `input` of the `ABS` call
/// `call`: the literal without its sign, of the type `input` has.
///
/// The constant stands where the call stood, so it takes the call's span: a
/// diagnostic that `ABS(SINT#-128)` is not a `SINT` points at the call.
fn absolute_literal(literal: &IntegerLiteral, input: &Expr, call: &Expr) -> Expr {
    let mut literal = literal.clone();
    literal.value.is_neg = false;
    literal.value.value.span = call.span.clone();
    Expr {
        kind: ExprKind::Const(ConstantKind::IntegerLiteral(literal)),
        expr_type: input.expr_type.clone(),
        span: call.span.clone(),
    }
}

impl Fold<Diagnostic> for UnsignedAbsRemover<'_> {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        let node = Expr::recurse_fold(node, self)?;
        Ok(self.without_abs(&node).unwrap_or(node))
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::common::ConstantKind;
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

    /// The value of every assignment in a library after analysis, as written,
    /// with the type an integer literal names.
    #[derive(Default)]
    struct AssignedValues(Vec<String>);

    impl Visitor<Infallible> for AssignedValues {
        type Value = ();
        fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
            self.0.push(match &node.value.kind {
                ExprKind::Const(ConstantKind::IntegerLiteral(literal)) => literal.to_string(),
                other => other.to_string(),
            });
            node.recurse_visit(self)
        }
    }

    fn assigned_values(source: &str) -> Vec<String> {
        let library = parse_and_resolve_types(source);
        let mut values = AssignedValues::default();
        let Ok(()) = values.walk(&library);
        values.0
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
    fn apply_when_abs_of_unsigned_subrange_then_call_removed() {
        assert_eq!(
            0,
            abs_calls(
                "TYPE US : UDINT(1..4000000000); END_TYPE
                 PROGRAM main VAR x : US := 1; y : UDINT; END_VAR y := ABS(x); END_PROGRAM"
            )
        );
    }

    #[test]
    fn apply_when_abs_of_signed_subrange_then_call_kept() {
        assert_eq!(
            1,
            abs_calls(
                "TYPE S : DINT(-5..5); END_TYPE
                 PROGRAM main VAR x : S; y : DINT; END_VAR y := ABS(x); END_PROGRAM"
            )
        );
    }

    #[test]
    fn apply_when_abs_of_untyped_literal_then_call_removed() {
        assert_eq!(
            0,
            abs_calls(
                "PROGRAM main VAR y : UDINT; z : DINT; END_VAR
                 y := ABS(3000000000); z := ABS(5) + z; END_PROGRAM"
            )
        );
    }

    #[test]
    fn apply_when_abs_of_negative_or_typed_literal_then_absolute_value() {
        assert_eq!(
            vec!["5", "DINT#5", "DINT#5", "3000000000"],
            assigned_values(
                "PROGRAM main VAR y : DINT; z : DINT; w : DINT; u : UDINT; END_VAR
                 y := ABS(-5); z := ABS(DINT#-5); w := ABS(DINT#5);
                 u := ABS(-3000000000); END_PROGRAM"
            )
        );
    }

    #[test]
    fn apply_when_abs_of_abs_of_literal_then_absolute_value() {
        assert_eq!(
            vec!["5"],
            assigned_values("PROGRAM main VAR y : DINT; END_VAR y := ABS(ABS(-5)); END_PROGRAM")
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
