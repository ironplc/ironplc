//! Arithmetic on times and dates: the typed functions `ADD_TIME`,
//! `SUB_DT_DT`, `MUL_TIME` and the rest of IEC 61131-3 Table 30, the
//! operators and function forms that resolve to them (`dt + t`,
//! `ADD(t1, t2)`), `CONCAT_DATE_TOD`, `DT_TO_DATE` and `DT_TO_TOD`.
//!
//! Each is scalar arithmetic on the integers the values are stored as
//! (ADR-0025): a `TIME` or `TIME_OF_DAY` counts milliseconds, and a `DATE` or
//! `DATE_AND_TIME` seconds. An operation computes at the signed integer of
//! its result's width, to which the analyzer converted its operands
//! (ADR-0056). Its arithmetic follows from the types the analyzer recorded:
//! the second operand is converted to the unit the first counts in, the
//! operation applies, and the result is converted to the unit its own type
//! counts in. So `dt + t` is `dt + t / 1000`, and `d1 - d2` on dates is
//! `(d1 - d2) * 1000`.

use ironplc_analyzer::TimeFunction;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::Expr as AstExpr;
use ironplc_ir::lowered::{BinaryOp, Expr, ScalarType, Signedness, TemporalKind, TypeId, TypeKind};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::Lowering;

/// The milliseconds in a second.
const MILLIS_PER_SECOND: i128 = 1000;

/// The seconds in a day.
const SECONDS_PER_DAY: i128 = 86_400;

/// What a time or date value counts.
#[derive(Clone, Copy)]
enum Unit {
    Millis,
    Seconds,
}

impl Lowering<'_> {
    /// Whether the analyzer recorded a time or a date for `expr`.
    pub(crate) fn is_temporal(&mut self, expr: &AstExpr) -> Result<bool, Diagnostic> {
        let unit = self.recorded_type(expr)?.and_then(|ty| self.unit(ty));
        Ok(unit.is_some())
    }

    /// Whether the analyzer recorded a duration, a `TIME` or an `LTIME`, for
    /// `expr`. A date or a time of day counts from an epoch, so it is not.
    pub(crate) fn is_duration(&mut self, expr: &AstExpr) -> Result<bool, Diagnostic> {
        let Some(ty) = self.recorded_type(expr)? else {
            return Ok(false);
        };
        Ok(matches!(
            self.program.types().kind(ty),
            Some(TypeKind::Temporal {
                kind: TemporalKind::Time,
                ..
            })
        ))
    }

    /// The typed time function `function`, the expression `expr`, of `args`.
    pub(crate) fn time_function(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        function: TimeFunction,
        args: &[&AstExpr],
    ) -> Lowered {
        let [in1, in2] = args else {
            return Ok(None);
        };
        self.time_arithmetic(frame, expr, operation(function), in1, in2)
    }

    /// `in1 op in2`, the expression `expr`, whose result is a time or a
    /// date. `in1` is a time or a date, and `in2` one too, or the number a
    /// duration is scaled by.
    pub(crate) fn time_arithmetic(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        op: BinaryOp,
        in1: &AstExpr,
        in2: &AstExpr,
    ) -> Lowered {
        let span = expr.span();
        let Some((result_type, result)) = self.operated_type(expr)? else {
            return Ok(None);
        };
        let (Some(in1_type), Some(in2_type)) = (self.recorded_type(in1)?, self.recorded_type(in2)?)
        else {
            return Ok(None);
        };
        let (Some(result_unit), Some(in1_unit)) = (self.unit(result_type), self.unit(in1_type))
        else {
            return Ok(None);
        };
        let in2_unit = self.unit(in2_type);
        let (Some(in1), Some(in2)) = (self.expr(frame, in1)?, self.expr(frame, in2)?) else {
            return Ok(None);
        };
        let Some(at) = signed(result) else {
            return Ok(None);
        };
        let Some(in1) = of_width(in1, at, &span)? else {
            return Ok(None);
        };
        let value = match in2_unit {
            Some(in2_unit) => {
                let Some(in2) = of_width(in2, at, &span)? else {
                    return Ok(None);
                };
                let in2 = rescale(in2, in2_unit, in1_unit, &span)?;
                Expr::binary(op, in1, in2, span.clone())?
            }
            // A duration multiplied or divided by a number.
            None => {
                let Some(scaled) = scale(op, in1, in2, &span)? else {
                    return Ok(None);
                };
                scaled
            }
        };
        let value = rescale(value, in1_unit, result_unit, &span)?;
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

    /// The unit a value of the type `ty` counts in; `None` unless it is a
    /// time or a date.
    fn unit(&self, ty: TypeId) -> Option<Unit> {
        let Some(TypeKind::Temporal { kind, .. }) = self.program.types().kind(ty) else {
            return None;
        };
        Some(match kind {
            TemporalKind::Time | TemporalKind::TimeOfDay => Unit::Millis,
            TemporalKind::Date | TemporalKind::DateAndTime => Unit::Seconds,
        })
    }
}

/// The operation of `function`.
fn operation(function: TimeFunction) -> BinaryOp {
    match function {
        TimeFunction::AddTime
        | TimeFunction::AddTodTime
        | TimeFunction::AddDtTime
        | TimeFunction::ConcatDateTod => BinaryOp::Add,
        TimeFunction::SubTime
        | TimeFunction::SubTodTime
        | TimeFunction::SubTodTod
        | TimeFunction::SubDtTime
        | TimeFunction::SubDtDt
        | TimeFunction::SubDateDate => BinaryOp::Sub,
        TimeFunction::MulTime => BinaryOp::Mul,
        TimeFunction::DivTime => BinaryOp::Div,
    }
}

/// `value`, which counts `from`, converted to count `to`.
fn rescale(value: Expr, from: Unit, to: Unit, span: &SourceSpan) -> Result<Expr, Diagnostic> {
    let op = match (from, to) {
        (Unit::Millis, Unit::Millis) | (Unit::Seconds, Unit::Seconds) => return Ok(value),
        (Unit::Seconds, Unit::Millis) => BinaryOp::Mul,
        (Unit::Millis, Unit::Seconds) => BinaryOp::Div,
    };
    let millis = Expr::integer(MILLIS_PER_SECOND, value.ty(), span.clone())?;
    Expr::binary(op, value, millis, span.clone())
}

/// The signed integer of the width of `ty`, at which an operation of a
/// result of type `ty` computes; `None` unless `ty` is an integer.
fn signed(ty: ScalarType) -> Option<ScalarType> {
    match ty {
        ScalarType::I32(_) => Some(ScalarType::I32(Signedness::Signed)),
        ScalarType::I64(_) => Some(ScalarType::I64(Signedness::Signed)),
        ScalarType::Bool
        | ScalarType::F32
        | ScalarType::F64
        | ScalarType::Ref
        | ScalarType::Interface => None,
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
/// arithmetic on it signed, and at one width they hold the same bits. `None`
/// for a value of another width, which the analyzer did not convert to the
/// width the operation computes at.
fn of_width(value: Expr, ty: ScalarType, span: &SourceSpan) -> Lowered {
    if value.ty() == ty {
        return Ok(Some(value));
    }
    if !(value.ty().is_integer() && ty.is_integer() && value.ty().bits() == ty.bits()) {
        return Ok(None);
    }
    Expr::convert(value, ty, span.clone()).map(Some)
}
