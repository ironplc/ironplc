//! Pure arithmetic primitives for evaluating constant expressions over
//! literal `ConstantKind` values.
//!
//! Shared by `xform_fold_constant_expressions` (which folds constant
//! sub-expressions anywhere in a program) and
//! `xform_fold_initializer_expressions` (which folds constant-expression
//! `VAR` initializers, substituting named constant references first). Both
//! are transformation passes in their own right; this module holds the leaf
//! arithmetic they both depend on, so neither reaches into the other's
//! internals.

use ironplc_dsl::common::*;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::textual::*;

/// Extracts the value of an integer literal as an i128.
pub(crate) fn integer_value(lit: &IntegerLiteral) -> i128 {
    let unsigned = lit.value.value.value as i128;
    if lit.value.is_neg {
        -unsigned
    } else {
        unsigned
    }
}

/// Builds a `ConstantKind::IntegerLiteral` from an i128 result value.
pub(crate) fn make_integer_constant(value: i128) -> ConstantKind {
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
pub(crate) fn make_real_constant(value: f64) -> ConstantKind {
    ConstantKind::RealLiteral(RealLiteral {
        value,
        data_type: None,
    })
}

/// Attempts to fold a binary expression on two integer constants.
pub(crate) fn fold_integer_binary(op: &Operator, left: i128, right: i128) -> Option<i128> {
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
pub(crate) fn fold_real_binary(op: &Operator, left: f64, right: f64) -> f64 {
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
pub(crate) fn const_as_f64(kind: &ExprKind) -> Option<f64> {
    match kind {
        ExprKind::Const(ConstantKind::RealLiteral(lit)) => Some(lit.value),
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => Some(integer_value(lit) as f64),
        _ => None,
    }
}

/// Tries to fold a `BinaryExpr` whose operands are both constants.
/// Returns `Some(folded_kind)` if folding succeeded, `None` otherwise.
pub(crate) fn try_fold_binary(binary: &BinaryExpr) -> Option<ExprKind> {
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
pub(crate) fn try_fold_unary(unary: &UnaryExpr) -> Option<ExprKind> {
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
