//! The typed time and date arithmetic functions: `ADD_TIME`, `SUB_DATE_DATE`,
//! `MUL_TIME` and the rest of IEC 61131-3 Table 30, plus `CONCAT_DATE_TOD`.
//!
//! Each function is one of a few instruction sequences, chosen by the units
//! its operands are stored in (ADR-0025): `TIME` and `TIME_OF_DAY` in
//! milliseconds, `DATE` and `DATE_AND_TIME` in seconds. [`time_arith_for`]
//! names the sequence for a [`TimeFunction`] and [`compile_time_arith`] emits it for
//! two operand expressions, so the sequences do not depend on how the
//! operands were written.
//!
//! Each function also has a long form over `LTIME`, `LDATE`, `LTIME_OF_DAY`
//! and `LDATE_AND_TIME` (`ADD_LTIME`, `SUB_LDATE_LDATE`, ...). A long type
//! stores the same unit as its short type (ADR-0021, ADR-0025), so a long
//! form is the same sequence at 64-bit width.
//!
//! The analyzer converted each operand to the width its sequence computes at
//! (ADR-0056): a short `TIME` or `DATE` operand of a long form to its long
//! type, and the number `MUL` and `DIV` scale by to `LINT` or `LREAL` where
//! the sequence needs it. Each operand compiles at the type the analyzer
//! recorded for it; one of another width is an internal error rather than
//! widened here.

use ironplc_analyzer::TimeFunction;
use ironplc_container::opcode;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Expr;

use super::compile::{CompileContext, OpType, OpWidth, Signedness};
use super::compile_expr::{compile_expr, emit_add, emit_div, emit_mul, emit_sub, op_type};
use crate::emit::Emitter;

/// The instruction sequence a typed time or date function compiles to.
#[derive(Clone, Copy)]
pub(crate) enum TimeArith {
    /// Both operands are in milliseconds: the operator applies directly.
    SameUnit(fn(&mut Emitter, OpType)),
    /// IN1 is in seconds and IN2 in milliseconds: IN2 is converted to
    /// seconds before the operator applies.
    SecondsAndMillis(fn(&mut Emitter, OpType)),
    /// Both operands are in seconds and the result is a `TIME`: the
    /// difference is converted to milliseconds.
    SecondsDifference,
    /// A `TIME` multiplied by an `ANY_NUM`.
    Multiply,
    /// A `TIME` divided by an `ANY_NUM`.
    Divide,
}

/// Returns the instruction sequence for the time or date function
/// `function` and the width it operates at: 32-bit for a short form, 64-bit
/// for a long one.
pub(crate) fn time_arith_for(function: TimeFunction, long: bool) -> (TimeArith, OpWidth) {
    let arith = match function {
        TimeFunction::AddTime | TimeFunction::AddTodTime => TimeArith::SameUnit(emit_add),
        TimeFunction::SubTime | TimeFunction::SubTodTime | TimeFunction::SubTodTod => {
            TimeArith::SameUnit(emit_sub)
        }
        TimeFunction::AddDtTime | TimeFunction::ConcatDateTod => {
            TimeArith::SecondsAndMillis(emit_add)
        }
        TimeFunction::SubDtTime => TimeArith::SecondsAndMillis(emit_sub),
        TimeFunction::SubDtDt | TimeFunction::SubDateDate => TimeArith::SecondsDifference,
        TimeFunction::MulTime => TimeArith::Multiply,
        TimeFunction::DivTime => TimeArith::Divide,
    };
    let width = if long { OpWidth::W64 } else { OpWidth::W32 };
    (arith, width)
}

/// Compiles `arith` at `width` over the operands `in1` and `in2`, leaving
/// the result on the stack.
pub(crate) fn compile_time_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arith: TimeArith,
    width: OpWidth,
    in1: &Expr,
    in2: &Expr,
) -> Result<(), Diagnostic> {
    let op_type = (width, Signedness::Signed);
    match arith {
        TimeArith::SameUnit(emit_fn) => compile_same_unit(emitter, ctx, op_type, in1, in2, emit_fn),
        TimeArith::SecondsAndMillis(emit_fn) => {
            compile_dt_time_add_sub(emitter, ctx, op_type, in1, in2, emit_fn)
        }
        TimeArith::SecondsDifference => compile_sub_to_time(emitter, ctx, op_type, in1, in2),
        TimeArith::Multiply => compile_scale(emitter, ctx, width, in1, in2, emit_mul),
        TimeArith::Divide => compile_scale(emitter, ctx, width, in1, in2, emit_div),
    }
}

/// Compiles a duration scaled by a number, `emit_fn` being the multiply or
/// divide emitter, at `width`.
fn compile_scale(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    width: OpWidth,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    match width {
        OpWidth::W64 => compile_mul_div_ltime(emitter, ctx, in1, in2, emit_fn),
        _ => compile_mul_div_time(emitter, ctx, in1, in2, emit_fn),
    }
}

/// Compiles an operand of a typed time or date function at its own type,
/// which the analyzer converted to `width`, the width the function computes
/// at.
///
/// A `DATE`, `TIME_OF_DAY` or `DATE_AND_TIME` operand is unsigned and the
/// sequence signed (ADR-0025); at one width they hold the same bits.
fn compile_operand(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    width: OpWidth,
    operand: &Expr,
) -> Result<(), Diagnostic> {
    let own = op_type(ctx, operand)?;
    if own.0 != width {
        return Err(unconverted_operand(operand, own.0, width));
    }
    compile_expr(emitter, ctx, operand, own)
}

/// The internal error for an operand of a typed time or date function whose
/// width `own` is not `expected`, the width the function computes at, and
/// that the analyzer did not convert.
fn unconverted_operand(operand: &Expr, own: OpWidth, expected: OpWidth) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        operand.span(),
        format!(
            "Time function operand of width {own:?} where {expected:?} is expected, and the analyzer recorded no conversion"
        ),
    ))
}

/// Pushes the milliseconds in a second, 1000, at the width of `op_type`.
fn load_millis_per_second(emitter: &mut Emitter, ctx: &mut CompileContext, op_type: OpType) {
    if op_type.0 == OpWidth::W64 {
        let pool_idx = ctx.add_i64_constant(1000);
        emitter.emit_load_const_i64(pool_idx);
    } else {
        let pool_idx = ctx.add_i32_constant(1000);
        emitter.emit_load_const_i32(pool_idx);
    }
}

/// Compiles ADD_TIME, ADD_TOD_TIME, SUB_TIME, SUB_TOD_TIME and SUB_TOD_TOD,
/// and their long forms.
///
/// Both operands are in milliseconds, so the operator applies directly.
fn compile_same_unit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op_type: OpType,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    compile_operand(emitter, ctx, op_type.0, in1)?;
    compile_operand(emitter, ctx, op_type.0, in2)?;
    emit_fn(emitter, op_type);
    Ok(())
}

/// Compiles ADD_DT_TIME, SUB_DT_TIME, and CONCAT_DATE_TOD, and the long forms
/// ADD_LDT_LTIME and SUB_LDT_LTIME.
///
/// IN2 (TIME or TOD) is in milliseconds while IN1 (DT or DATE) is in seconds.
/// Converts IN2 from ms to seconds by dividing by 1000, then adds or subtracts.
fn compile_dt_time_add_sub(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op_type: OpType,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    compile_operand(emitter, ctx, op_type.0, in1)?;
    compile_operand(emitter, ctx, op_type.0, in2)?;
    load_millis_per_second(emitter, ctx, op_type);
    emit_div(emitter, op_type);
    emit_fn(emitter, op_type);
    Ok(())
}

/// Compiles SUB_DT_DT and SUB_DATE_DATE, and their long forms.
///
/// Subtracts two values in seconds, then multiplies by 1000 to produce TIME
/// in milliseconds.
fn compile_sub_to_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op_type: OpType,
    in1: &Expr,
    in2: &Expr,
) -> Result<(), Diagnostic> {
    compile_operand(emitter, ctx, op_type.0, in1)?;
    compile_operand(emitter, ctx, op_type.0, in2)?;
    emit_sub(emitter, op_type);
    load_millis_per_second(emitter, ctx, op_type);
    emit_mul(emitter, op_type);
    Ok(())
}

/// Compiles MUL_TIME and DIV_TIME.
///
/// IN1 is TIME (i32 ms). IN2 is ANY_NUM, compiled at its own type: an
/// integer of 32 bits scales the duration directly, and a real scales it
/// converted to the real's type and back. A 64-bit integer arrives converted
/// to `LREAL`, at which the analyzer recorded it is scaled.
fn compile_mul_div_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let in2_op = op_type(ctx, in2)?;
    compile_operand(emitter, ctx, OpWidth::W32, in1)?;
    let (to_float, from_float) = match in2_op.0 {
        OpWidth::W32 => {
            compile_expr(emitter, ctx, in2, in2_op)?;
            emit_fn(emitter, (OpWidth::W32, Signedness::Signed));
            return Ok(());
        }
        OpWidth::F32 => (
            opcode::builtin::CONV_I32_TO_F32,
            opcode::builtin::CONV_F32_TO_I32,
        ),
        OpWidth::F64 => (
            opcode::builtin::CONV_I32_TO_F64,
            opcode::builtin::CONV_F64_TO_I32,
        ),
        OpWidth::W64 => return Err(unconverted_operand(in2, in2_op.0, OpWidth::F64)),
    };
    emitter.emit_builtin(to_float);
    compile_expr(emitter, ctx, in2, in2_op)?;
    emit_fn(emitter, (in2_op.0, Signedness::Signed));
    emitter.emit_builtin(from_float);
    Ok(())
}

/// Compiles MUL_LTIME and DIV_LTIME.
///
/// IN1 is LTIME (i64 ms). IN2 arrives converted to a 64-bit integer, which
/// scales the duration directly, or to `LREAL`, which scales it converted to
/// `LREAL` and back: a duration in milliseconds can need more than the 24
/// bits of a `REAL` mantissa, so the analyzer widened a `REAL` IN2 rather
/// than the duration being narrowed.
fn compile_mul_div_ltime(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let in2_op = op_type(ctx, in2)?;
    compile_operand(emitter, ctx, OpWidth::W64, in1)?;
    match in2_op.0 {
        OpWidth::W64 => {
            compile_expr(emitter, ctx, in2, in2_op)?;
            emit_fn(emitter, (OpWidth::W64, Signedness::Signed));
        }
        OpWidth::F64 => {
            emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64);
            compile_expr(emitter, ctx, in2, in2_op)?;
            emit_fn(emitter, (OpWidth::F64, Signedness::Signed));
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I64);
        }
        OpWidth::W32 => return Err(unconverted_operand(in2, in2_op.0, OpWidth::W64)),
        OpWidth::F32 => return Err(unconverted_operand(in2, in2_op.0, OpWidth::F64)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Names the instruction sequence `arith`, so a test can compare it.
    fn sequence(arith: TimeArith) -> &'static str {
        match arith {
            TimeArith::SameUnit(_) => "same unit",
            TimeArith::SecondsAndMillis(_) => "seconds and millis",
            TimeArith::SecondsDifference => "seconds difference",
            TimeArith::Multiply => "multiply",
            TimeArith::Divide => "divide",
        }
    }

    /// Every time function compiles as the sequence for the units of its
    /// operands.
    #[rstest]
    #[case::add_time(TimeFunction::AddTime, "same unit")]
    #[case::sub_time(TimeFunction::SubTime, "same unit")]
    #[case::mul_time(TimeFunction::MulTime, "multiply")]
    #[case::div_time(TimeFunction::DivTime, "divide")]
    #[case::add_tod_time(TimeFunction::AddTodTime, "same unit")]
    #[case::sub_tod_time(TimeFunction::SubTodTime, "same unit")]
    #[case::add_dt_time(TimeFunction::AddDtTime, "seconds and millis")]
    #[case::sub_dt_time(TimeFunction::SubDtTime, "seconds and millis")]
    #[case::sub_dt_dt(TimeFunction::SubDtDt, "seconds difference")]
    #[case::sub_date_date(TimeFunction::SubDateDate, "seconds difference")]
    #[case::sub_tod_tod(TimeFunction::SubTodTod, "same unit")]
    #[case::concat_date_tod(TimeFunction::ConcatDateTod, "seconds and millis")]
    fn time_arith_for_when_time_function_then_sequence_for_its_units(
        #[case] function: TimeFunction,
        #[case] expected: &str,
    ) {
        let (arith, _) = time_arith_for(function, false);

        assert_eq!(sequence(arith), expected);
    }

    /// A long form is the same sequence at 64-bit width.
    #[rstest]
    #[case::short(false, OpWidth::W32)]
    #[case::long(true, OpWidth::W64)]
    fn time_arith_for_when_form_then_its_width(#[case] long: bool, #[case] expected: OpWidth) {
        let (short_arith, _) = time_arith_for(TimeFunction::SubDtDt, false);
        let (arith, width) = time_arith_for(TimeFunction::SubDtDt, long);

        assert_eq!(width, expected);
        assert_eq!(sequence(arith), sequence(short_arith));
    }
}
