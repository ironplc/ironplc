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
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::*;
use ironplc_problems::Problem;

/// A constant expression whose operands are both known at compile time,
/// but whose operation has no defined result (as opposed to an expression
/// that simply isn't constant at all, which is `try_fold_binary`/
/// `try_fold_unary` returning `Ok(None)` and is not an error here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FoldError {
    DivisionByZero,
    Overflow,
}

/// Converts a `FoldError` into a user-facing diagnostic at the given span.
pub(crate) fn fold_error_to_diagnostic(err: FoldError, span: SourceSpan) -> Diagnostic {
    match err {
        FoldError::DivisionByZero => Diagnostic::problem(
            Problem::ConstantExpressionDivisionByZero,
            Label::span(span, "Division or modulo by zero"),
        ),
        FoldError::Overflow => Diagnostic::problem(
            Problem::ConstantExpressionOverflow,
            Label::span(span, "Arithmetic overflow"),
        ),
    }
}

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
///
/// The caller has already established that both operands are constant
/// literals, so every arithmetic path here either succeeds or names a
/// specific `FoldError` -- there is no silent "can't fold" case (the one
/// exception, `Pow` with a negative exponent, is handled by the caller
/// before this is invoked, since it isn't an error, just out of scope for
/// integer exponentiation).
///
/// Folding works in `i128` headroom and does not range-check the result
/// against the declared type; codegen does that, two stages later, and is
/// what emits `P2026`.
pub(crate) fn fold_integer_binary(
    op: &Operator,
    left: i128,
    right: i128,
) -> Result<i128, FoldError> {
    match op {
        Operator::Add => left.checked_add(right).ok_or(FoldError::Overflow),
        Operator::Sub => left.checked_sub(right).ok_or(FoldError::Overflow),
        Operator::Mul => left.checked_mul(right).ok_or(FoldError::Overflow),
        Operator::Div => {
            if right == 0 {
                Err(FoldError::DivisionByZero)
            } else {
                left.checked_div(right).ok_or(FoldError::Overflow)
            }
        }
        Operator::Mod => {
            if right == 0 {
                Err(FoldError::DivisionByZero)
            } else {
                left.checked_rem(right).ok_or(FoldError::Overflow)
            }
        }
        Operator::Pow => {
            // The caller guarantees `right >= 0` -- see `try_fold_binary`.
            let exp = right as u32;
            left.checked_pow(exp).ok_or(FoldError::Overflow)
        }
    }
}

/// Attempts to fold a binary expression on two real constants.
pub(crate) fn fold_real_binary(op: &Operator, left: f64, right: f64) -> Result<f64, FoldError> {
    match op {
        Operator::Add => Ok(left + right),
        Operator::Sub => Ok(left - right),
        Operator::Mul => Ok(left * right),
        Operator::Div => {
            if right == 0.0 {
                Err(FoldError::DivisionByZero)
            } else {
                Ok(left / right)
            }
        }
        Operator::Mod => {
            if right == 0.0 {
                Err(FoldError::DivisionByZero)
            } else {
                Ok(left % right)
            }
        }
        Operator::Pow => Ok(left.powf(right)),
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
///
/// Returns `Ok(Some(folded_kind))` if folding succeeded, `Ok(None)` if the
/// operands aren't both constant literals (nothing to fold, not an
/// error), and `Err(FoldError)` if both operands are constant but the
/// operation itself has no defined result (e.g. division by zero,
/// overflow).
pub(crate) fn try_fold_binary(binary: &BinaryExpr) -> Result<Option<ExprKind>, FoldError> {
    match (&binary.left.kind, &binary.right.kind) {
        (
            ExprKind::Const(ConstantKind::IntegerLiteral(left)),
            ExprKind::Const(ConstantKind::IntegerLiteral(right)),
        ) => {
            let lv = integer_value(left);
            let rv = integer_value(right);
            if matches!(binary.op, Operator::Pow) && rv < 0 {
                // Integer exponentiation with a negative exponent isn't
                // meaningful; leave unfolded rather than misreporting it
                // as an overflow.
                return Ok(None);
            }
            let result = fold_integer_binary(&binary.op, lv, rv)?;
            Ok(Some(ExprKind::Const(make_integer_constant(result))))
        }
        (
            ExprKind::Const(ConstantKind::RealLiteral(left)),
            ExprKind::Const(ConstantKind::RealLiteral(right)),
        ) => {
            let result = fold_real_binary(&binary.op, left.value, right.value)?;
            Ok(Some(ExprKind::Const(make_real_constant(result))))
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
            let (Some(lv), Some(rv)) = (
                const_as_f64(&binary.left.kind),
                const_as_f64(&binary.right.kind),
            ) else {
                return Ok(None);
            };
            let result = fold_real_binary(&binary.op, lv, rv)?;
            Ok(Some(ExprKind::Const(make_real_constant(result))))
        }
        _ => Ok(None),
    }
}

/// Tries to fold a `UnaryExpr` whose operand is a constant.
/// Returns `Some(folded_kind)` if folding succeeded, `None` otherwise.
/// Unary negation of a literal cannot fail (no division, no overflow --
/// the operand is negated to a wider intermediate representation), so
/// this stays infallible.
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
