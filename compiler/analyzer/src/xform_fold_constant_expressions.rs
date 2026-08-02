//! Transform that folds constant expressions at compile time.
//!
//! When both operands of a binary expression are compile-time constant
//! literals (integer or real), the pass evaluates the operation and replaces
//! the expression node with a single constant. Similarly, unary negation of
//! a constant literal is folded into the negated constant.
//!
//! This runs after `xform_resolve_expr_types` so that `resolved_type` is
//! available on every `Expr` node.
//!
//! ## Before
//!
//! ```ignore
//! x := 2 + 3;
//! ```
//!
//! ## After
//!
//! ```ignore
//! x := 5;
//! ```
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;

use crate::constant_folding::{fold_error_to_diagnostic, try_fold_binary, try_fold_unary};

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    let mut folder = ConstantFolder;
    folder.fold_library(lib).map_err(|e| vec![e])
}

struct ConstantFolder;

impl Fold<Diagnostic> for ConstantFolder {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        // Recurse into children first (bottom-up folding).
        let node = Expr::recurse_fold(node, self)?;

        let folded_kind = match &node.kind {
            ExprKind::BinaryOp(binary) => {
                try_fold_binary(binary).map_err(|e| fold_error_to_diagnostic(e, node.span()))?
            }
            ExprKind::UnaryOp(unary) => try_fold_unary(unary),
            _ => None,
        };

        match folded_kind {
            Some(kind) => Ok(Expr {
                kind,
                resolved_type: node.resolved_type,
            }),
            None => Ok(node),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant_folding::integer_value;
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_problems::Problem;

    fn apply_fold(program: &str) -> Library {
        let library = parse_and_resolve_types(program);
        apply(library).unwrap()
    }

    /// Extracts all `Expr` nodes from a library for inspection.
    struct ExprCollector {
        exprs: Vec<ExprKind>,
    }

    impl Visitor<Diagnostic> for ExprCollector {
        type Value = ();
        fn visit_expr(&mut self, node: &Expr) -> Result<(), Diagnostic> {
            self.exprs.push(node.kind.clone());
            node.recurse_visit(self)
        }
    }

    fn collect_exprs(library: &Library) -> Vec<ExprKind> {
        let mut collector = ExprCollector { exprs: vec![] };
        collector.walk(library).unwrap();
        collector.exprs
    }

    fn assert_has_integer_const(exprs: &[ExprKind], expected: i128) {
        let found = exprs.iter().any(|e| {
            if let ExprKind::Const(ConstantKind::IntegerLiteral(lit)) = e {
                integer_value(lit) == expected
            } else {
                false
            }
        });
        assert!(
            found,
            "Expected integer constant {} in expressions: {:?}",
            expected, exprs
        );
    }

    fn assert_has_real_const(exprs: &[ExprKind], expected: f64) {
        let found = exprs.iter().any(|e| {
            if let ExprKind::Const(ConstantKind::RealLiteral(lit)) = e {
                (lit.value - expected).abs() < f64::EPSILON
            } else {
                false
            }
        });
        assert!(
            found,
            "Expected real constant {} in expressions: {:?}",
            expected, exprs
        );
    }

    fn assert_no_binary_ops(exprs: &[ExprKind]) {
        let has_binary = exprs.iter().any(|e| matches!(e, ExprKind::BinaryOp(_)));
        assert!(
            !has_binary,
            "Expected no binary ops but found some in: {:?}",
            exprs
        );
    }

    // --- Binary integer folding ---

    #[test]
    fn fold_expr_when_add_two_integers_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 2 + 3; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 5);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_sub_two_integers_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 10 - 4; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 6);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_mul_two_integers_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 3 * 7; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 21);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_div_two_integers_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 20 / 4; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 5);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_mod_two_integers_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 17 MOD 5; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 2);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_int_div_by_zero_then_error() {
        let library =
            parse_and_resolve_types("PROGRAM main VAR x : INT; END_VAR x := 10 / 0; END_PROGRAM");
        let result = apply(library);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionDivisionByZero.code()));
    }

    #[test]
    fn fold_expr_when_int_mod_by_zero_then_error() {
        let library =
            parse_and_resolve_types("PROGRAM main VAR x : INT; END_VAR x := 10 MOD 0; END_PROGRAM");
        let result = apply(library);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionDivisionByZero.code()));
    }

    #[test]
    fn fold_expr_when_real_div_by_zero_then_error() {
        let library = parse_and_resolve_types(
            "PROGRAM main VAR x : LREAL; END_VAR x := 1.0 / 0.0; END_PROGRAM",
        );
        let result = apply(library);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionDivisionByZero.code()));
    }

    #[test]
    fn fold_expr_when_int_overflow_then_error() {
        let library = parse_and_resolve_types(
            "PROGRAM main VAR x : LINT; END_VAR x := 170141183460469231731687303715884105727 * 2; END_PROGRAM",
        );
        let result = apply(library);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .all(|d| d.code == Problem::ConstantExpressionOverflow.code()));
    }

    #[test]
    fn fold_expr_when_negative_integer_exponent_then_no_fold_no_error() {
        // Negative integer exponentiation is not meaningful for integers;
        // it stays unfolded and is not reported as an overflow.
        let library =
            parse_and_resolve_types("PROGRAM main VAR x : INT; END_VAR x := 2 ** -1; END_PROGRAM");
        let lib = apply(library).unwrap();
        let exprs = collect_exprs(&lib);
        let has_binary = exprs.iter().any(|e| matches!(e, ExprKind::BinaryOp(_)));
        assert!(has_binary, "Negative exponent should not be folded");
    }

    // --- Nested constant folding ---

    #[test]
    fn fold_expr_when_nested_binary_then_folds_completely() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := (2 + 3) * 4; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, 20);
        assert_no_binary_ops(&exprs);
    }

    // --- Binary real folding ---

    #[test]
    fn fold_expr_when_add_two_reals_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := 1.5 + 2.5; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, 4.0);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_mul_two_reals_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := 3.0 * 2.0; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, 6.0);
        assert_no_binary_ops(&exprs);
    }

    // --- Unary negation folding ---

    #[test]
    fn fold_expr_when_negate_integer_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := -5; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_integer_const(&exprs, -5);
        let has_unary = exprs.iter().any(|e| matches!(e, ExprKind::UnaryOp(_)));
        assert!(!has_unary, "Unary negation should be folded");
    }

    #[test]
    fn fold_expr_when_negate_real_then_produces_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := -3.25; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, -3.25);
        let has_unary = exprs.iter().any(|e| matches!(e, ExprKind::UnaryOp(_)));
        assert!(!has_unary, "Unary negation should be folded");
    }

    // --- Mixed integer + real folding ---

    #[test]
    fn fold_expr_when_add_integer_and_real_then_produces_real_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := 2 + 3.5; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, 5.5);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_add_real_and_integer_then_produces_real_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := 1.5 + 2; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, 3.5);
        assert_no_binary_ops(&exprs);
    }

    #[test]
    fn fold_expr_when_mul_integer_and_real_then_produces_real_constant() {
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := 3 * 2.5; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, 7.5);
        assert_no_binary_ops(&exprs);
    }

    // --- Non-constant operands are left unchanged ---

    #[test]
    fn fold_expr_when_variable_operand_then_no_fold() {
        let lib = apply_fold("PROGRAM main VAR x : INT; y : INT; END_VAR x := y + 3; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        let has_binary = exprs.iter().any(|e| matches!(e, ExprKind::BinaryOp(_)));
        assert!(has_binary, "Non-constant binary should not be folded");
    }
}
