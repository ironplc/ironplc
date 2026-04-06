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
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    let mut folder = ConstantFolder;
    folder.fold_library(lib).map_err(|e| vec![e])
}

struct ConstantFolder;

/// Extracts the value of an integer literal as an i128.
fn integer_value(lit: &IntegerLiteral) -> i128 {
    let unsigned = lit.value.value.value as i128;
    if lit.value.is_neg {
        -unsigned
    } else {
        unsigned
    }
}

/// Builds a `ConstantKind::IntegerLiteral` from an i128 result value.
fn make_integer_constant(value: i128) -> ConstantKind {
    let (unsigned, is_neg) = if value < 0 {
        ((-value) as u128, true)
    } else {
        (value as u128, false)
    };
    ConstantKind::IntegerLiteral(IntegerLiteral {
        value: SignedInteger {
            value: Integer {
                span: SourceSpan::default(),
                value: unsigned,
            },
            is_neg,
        },
        data_type: None,
    })
}

/// Builds a `ConstantKind::RealLiteral` from an f64 result value.
fn make_real_constant(value: f64) -> ConstantKind {
    ConstantKind::RealLiteral(RealLiteral {
        value,
        data_type: None,
    })
}

/// Attempts to fold a binary expression on two integer constants.
fn fold_integer_binary(op: &Operator, left: i128, right: i128) -> Option<i128> {
    match op {
        Operator::Add => left.checked_add(right),
        Operator::Sub => left.checked_sub(right),
        Operator::Mul => left.checked_mul(right),
        Operator::Div => {
            if right == 0 {
                None
            } else {
                left.checked_div(right)
            }
        }
        Operator::Mod => {
            if right == 0 {
                None
            } else {
                left.checked_rem(right)
            }
        }
        Operator::Pow => {
            if right < 0 {
                // Integer exponentiation with negative exponent is not meaningful.
                None
            } else {
                let exp = right as u32;
                left.checked_pow(exp)
            }
        }
    }
}

/// Attempts to fold a binary expression on two real constants.
fn fold_real_binary(op: &Operator, left: f64, right: f64) -> f64 {
    match op {
        Operator::Add => left + right,
        Operator::Sub => left - right,
        Operator::Mul => left * right,
        Operator::Div => left / right,
        Operator::Mod => left % right,
        Operator::Pow => left.powf(right),
    }
}

/// Extracts a constant as an f64, converting integers to float if needed.
fn const_as_f64(kind: &ExprKind) -> Option<f64> {
    match kind {
        ExprKind::Const(ConstantKind::RealLiteral(lit)) => Some(lit.value),
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => Some(integer_value(lit) as f64),
        _ => None,
    }
}

/// Tries to fold a `BinaryExpr` whose operands are both constants.
/// Returns `Some(folded_kind)` if folding succeeded, `None` otherwise.
fn try_fold_binary(binary: &BinaryExpr) -> Option<ExprKind> {
    match (&binary.left.kind, &binary.right.kind) {
        (
            ExprKind::Const(ConstantKind::IntegerLiteral(left)),
            ExprKind::Const(ConstantKind::IntegerLiteral(right)),
        ) => {
            let lv = integer_value(left);
            let rv = integer_value(right);
            let result = fold_integer_binary(&binary.op, lv, rv)?;
            Some(ExprKind::Const(make_integer_constant(result)))
        }
        (
            ExprKind::Const(ConstantKind::RealLiteral(left)),
            ExprKind::Const(ConstantKind::RealLiteral(right)),
        ) => {
            let result = fold_real_binary(&binary.op, left.value, right.value);
            Some(ExprKind::Const(make_real_constant(result)))
        }
        // Mixed integer + real: promote the integer to f64 and fold as real.
        (
            ExprKind::Const(ConstantKind::IntegerLiteral(_)),
            ExprKind::Const(ConstantKind::RealLiteral(_)),
        )
        | (
            ExprKind::Const(ConstantKind::RealLiteral(_)),
            ExprKind::Const(ConstantKind::IntegerLiteral(_)),
        ) => {
            let lv = const_as_f64(&binary.left.kind)?;
            let rv = const_as_f64(&binary.right.kind)?;
            let result = fold_real_binary(&binary.op, lv, rv);
            Some(ExprKind::Const(make_real_constant(result)))
        }
        _ => None,
    }
}

/// Tries to fold a `UnaryExpr` whose operand is a constant.
/// Returns `Some(folded_kind)` if folding succeeded, `None` otherwise.
fn try_fold_unary(unary: &UnaryExpr) -> Option<ExprKind> {
    match unary.op {
        UnaryOp::Neg => match &unary.term.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
                let value = integer_value(lit);
                Some(ExprKind::Const(make_integer_constant(-value)))
            }
            ExprKind::Const(ConstantKind::RealLiteral(lit)) => {
                Some(ExprKind::Const(make_real_constant(-lit.value)))
            }
            _ => None,
        },
        UnaryOp::Not => None,
    }
}

impl Fold<Diagnostic> for ConstantFolder {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        // Recurse into children first (bottom-up folding).
        let node = Expr::recurse_fold(node, self)?;

        let folded_kind = match &node.kind {
            ExprKind::BinaryOp(binary) => try_fold_binary(binary),
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
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::visitor::Visitor;

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
    fn fold_expr_when_div_by_zero_then_no_fold() {
        let lib = apply_fold("PROGRAM main VAR x : INT; END_VAR x := 10 / 0; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        let has_binary = exprs.iter().any(|e| matches!(e, ExprKind::BinaryOp(_)));
        assert!(has_binary, "Division by zero should not be folded");
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
        let lib = apply_fold("PROGRAM main VAR x : REAL; END_VAR x := -3.14; END_PROGRAM");
        let exprs = collect_exprs(&lib);
        assert_has_real_const(&exprs, -3.14);
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
