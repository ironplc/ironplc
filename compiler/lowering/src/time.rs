//! The typed time and date functions: `ADD_TIME`, `SUB_DT_DT`, `MUL_TIME`
//! and the rest of IEC 61131-3 Table 30, `CONCAT_DATE_TOD`, `DT_TO_DATE` and
//! `DT_TO_TOD`.
//!
//! Each is scalar arithmetic on the integers the values are stored as
//! (ADR-0025): a `TIME` or `TIME_OF_DAY` in milliseconds, and a `DATE` or
//! `DATE_AND_TIME` in seconds. A typed function computes at the signed
//! integer of its width, 64 bits for a long form such as `ADD_LTIME`, to
//! which the analyzer converted its operands (ADR-0056).

use ironplc_analyzer::TimeFunction;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::Expr as AstExpr;
use ironplc_ir::lowered::{BinaryOp, Expr, ScalarType, Signedness};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

/// The milliseconds in a second.
const MILLIS_PER_SECOND: i128 = 1000;

/// The seconds in a day.
const SECONDS_PER_DAY: i128 = 86_400;

/// The arithmetic a typed time function is, chosen by the units of its
/// operands.
enum Sequence {
    /// Both operands are in the same unit: the operation applies directly.
    SameUnit(BinaryOp),
    /// `IN1` is in seconds and `IN2` in milliseconds, which are converted to
    /// seconds first.
    SecondsAndMillis(BinaryOp),
    /// Both operands are in seconds and the result is a `TIME`: their
    /// difference is converted to milliseconds.
    SecondsDifference,
    /// A duration multiplied or divided by a number.
    Scale(BinaryOp),
}

impl Lowering<'_> {
    /// The typed time function `function`, the expression `expr`, of `args`,
    /// computed at 64 bits when `long`.
    pub(crate) fn time_function(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: TimeFunction,
        long: bool,
        args: &[&AstExpr],
    ) -> Lowered {
        let span = expr.span();
        let [in1, in2] = args else {
            return Ok(None);
        };
        let Some((_, result)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let (Some(in1), Some(in2)) = (self.expr(frame, in1)?, self.expr(frame, in2)?) else {
            return Ok(None);
        };
        let at = if long {
            ScalarType::I64(Signedness::Signed)
        } else {
            ScalarType::I32(Signedness::Signed)
        };
        let Some(in1) = of_width(in1, at, &span)? else {
            return Ok(None);
        };
        let value = match sequence(function) {
            Sequence::SameUnit(op) => {
                let Some(in2) = of_width(in2, at, &span)? else {
                    return Ok(None);
                };
                Expr::binary(op, in1, in2, span.clone())?
            }
            Sequence::SecondsAndMillis(op) => {
                let Some(in2) = of_width(in2, at, &span)? else {
                    return Ok(None);
                };
                let millis = Expr::integer(MILLIS_PER_SECOND, at, span.clone())?;
                let seconds = Expr::binary(BinaryOp::Div, in2, millis, span.clone())?;
                Expr::binary(op, in1, seconds, span.clone())?
            }
            Sequence::SecondsDifference => {
                let Some(in2) = of_width(in2, at, &span)? else {
                    return Ok(None);
                };
                let seconds = Expr::binary(BinaryOp::Sub, in1, in2, span.clone())?;
                let millis = Expr::integer(MILLIS_PER_SECOND, at, span.clone())?;
                Expr::binary(BinaryOp::Mul, seconds, millis, span.clone())?
            }
            Sequence::Scale(op) => {
                let Some(scaled) = scale(op, in1, in2, &span)? else {
                    return Ok(None);
                };
                scaled
            }
        };
        of_width(value, result, &span)
    }

    /// `DT_TO_DATE(value)`, the expression `expr`: the date and time `value`
    /// in seconds, rounded down to its day as `(value / 86400) * 86400`,
    /// which reads `value` once.
    pub(crate) fn dt_to_date(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        value: &AstExpr,
    ) -> Lowered {
        self.of_day(frame, expr, value, |value, ty, span| {
            let day = Expr::integer(SECONDS_PER_DAY, ty, span.clone())?;
            let days = Expr::binary(BinaryOp::Div, value, day.clone(), span.clone())?;
            Expr::binary(BinaryOp::Mul, days, day, span)
        })
    }

    /// `DT_TO_TOD(value)`, the expression `expr`: the seconds of the date and
    /// time `value` since its midnight, in milliseconds.
    pub(crate) fn dt_to_tod(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        value: &AstExpr,
    ) -> Lowered {
        self.of_day(frame, expr, value, |value, ty, span| {
            let day = Expr::integer(SECONDS_PER_DAY, ty, span.clone())?;
            let seconds = Expr::binary(BinaryOp::Mod, value, day, span.clone())?;
            let millis = Expr::integer(MILLIS_PER_SECOND, ty, span.clone())?;
            Expr::binary(BinaryOp::Mul, seconds, millis, span)
        })
    }

    /// `part` of the date and time `value`, the expression `expr`, computed
    /// at the unsigned integer a `DATE_AND_TIME` is stored as, which is the
    /// type of the call's result too.
    fn of_day(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        value: &AstExpr,
        part: impl FnOnce(Expr, ScalarType, SourceSpan) -> Result<Expr, Diagnostic>,
    ) -> Lowered {
        let Some((_, result)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let Some(value) = self.expr(frame, value)? else {
            return Ok(None);
        };
        let at = ScalarType::I32(Signedness::Unsigned);
        if value.ty() != at || result != at {
            return Ok(None);
        }
        part(value, at, expr.span()).map(Some)
    }
}

/// The arithmetic of `function`.
fn sequence(function: TimeFunction) -> Sequence {
    match function {
        TimeFunction::AddTime | TimeFunction::AddTodTime => Sequence::SameUnit(BinaryOp::Add),
        TimeFunction::SubTime | TimeFunction::SubTodTime | TimeFunction::SubTodTod => {
            Sequence::SameUnit(BinaryOp::Sub)
        }
        TimeFunction::AddDtTime | TimeFunction::ConcatDateTod => {
            Sequence::SecondsAndMillis(BinaryOp::Add)
        }
        TimeFunction::SubDtTime => Sequence::SecondsAndMillis(BinaryOp::Sub),
        TimeFunction::SubDtDt | TimeFunction::SubDateDate => Sequence::SecondsDifference,
        TimeFunction::MulTime => Sequence::Scale(BinaryOp::Mul),
        TimeFunction::DivTime => Sequence::Scale(BinaryOp::Div),
    }
}

/// The duration `duration` multiplied or divided by `by`. An integer of the
/// duration's width scales it directly; a real scales it converted to the
/// real's type and back. A `REAL` does not scale a 64-bit duration, which
/// needs more digits than it holds: the analyzer converts one to `LREAL`.
fn scale(op: BinaryOp, duration: Expr, by: Expr, span: &SourceSpan) -> Lowered {
    let at = duration.ty();
    let real = by.ty();
    if !real.is_real() {
        let Some(by) = of_width(by, at, span)? else {
            return Ok(None);
        };
        return Expr::binary(op, duration, by, span.clone()).map(Some);
    }
    if real.bits() < at.bits() {
        return Ok(None);
    }
    let duration = Expr::convert(duration, real, span.clone())?;
    let scaled = Expr::binary(op, duration, by, span.clone())?;
    Expr::convert(scaled, at, span.clone()).map(Some)
}

/// `value` at the integer `ty`, which it differs from at most in signedness:
/// a `DATE`, `TIME_OF_DAY` or `DATE_AND_TIME` is unsigned, and the
/// arithmetic of a typed time function signed, and at one width they hold
/// the same bits. `None` for a value of another width, which the analyzer
/// did not convert to the width the function computes at.
fn of_width(value: Expr, ty: ScalarType, span: &SourceSpan) -> Lowered {
    if value.ty() == ty {
        return Ok(Some(value));
    }
    if !(value.ty().is_integer() && ty.is_integer() && value.ty().bits() == ty.bits()) {
        return Ok(None);
    }
    Expr::convert(value, ty, span.clone()).map(Some)
}
