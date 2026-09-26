//! The typed time and date arithmetic functions: `ADD_TIME`, `SUB_DATE_DATE`,
//! `MUL_TIME` and the rest of IEC 61131-3 Table 30, plus `CONCAT_DATE_TOD`.
//!
//! Each function is one of a few instruction sequences, chosen by the units
//! its operands are stored in (ADR-0025): `TIME` and `TIME_OF_DAY` in
//! milliseconds, `DATE` and `DATE_AND_TIME` in seconds. [`time_arith_for`]
//! names the sequence for a function and [`compile_time_arith`] emits it for
//! two operand expressions, so the sequences do not depend on how the
//! operands were written.
//!
//! Each function also has a long form over `LTIME`, `LDATE`, `LTIME_OF_DAY`
//! and `LDATE_AND_TIME` (`ADD_LTIME`, `SUB_LDATE_LDATE`, ...). A long type
//! stores the same unit as its short type (ADR-0021, ADR-0025), so a long
//! form is the same sequence at 64-bit width.

use ironplc_container::opcode;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::Expr;

use super::compile::{CompileContext, OpType, OpWidth, Signedness};
use super::compile_expr::{
    compile_expr, emit_add, emit_div, emit_mul, emit_sub, op_type_from_expr,
};
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

/// Returns the instruction sequence for the typed time or date function
/// `name` (lower case) and the width it operates at, or `None` when `name`
/// is not one.
///
/// The width comes from the name: 32-bit for a short form, 64-bit for a
/// long one.
pub(crate) fn time_arith_for(name: &str) -> Option<(TimeArith, OpWidth)> {
    use OpWidth::{W32, W64};
    let found = match name {
        "add_time" | "add_tod_time" => (TimeArith::SameUnit(emit_add), W32),
        "add_ltime" | "add_ltod_ltime" => (TimeArith::SameUnit(emit_add), W64),
        "sub_time" | "sub_tod_time" | "sub_tod_tod" => (TimeArith::SameUnit(emit_sub), W32),
        "sub_ltime" | "sub_ltod_ltime" | "sub_ltod_ltod" => (TimeArith::SameUnit(emit_sub), W64),
        "add_dt_time" | "concat_date_tod" => (TimeArith::SecondsAndMillis(emit_add), W32),
        "add_ldt_ltime" => (TimeArith::SecondsAndMillis(emit_add), W64),
        "sub_dt_time" => (TimeArith::SecondsAndMillis(emit_sub), W32),
        "sub_ldt_ltime" => (TimeArith::SecondsAndMillis(emit_sub), W64),
        "sub_dt_dt" | "sub_date_date" => (TimeArith::SecondsDifference, W32),
        "sub_ldt_ldt" | "sub_ldate_ldate" => (TimeArith::SecondsDifference, W64),
        "mul_time" => (TimeArith::Multiply, W32),
        "mul_ltime" => (TimeArith::Multiply, W64),
        "div_time" => (TimeArith::Divide, W32),
        "div_ltime" => (TimeArith::Divide, W64),
        _ => return None,
    };
    Some(found)
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

/// Compiles an operand of a typed time or date function at `op_type`.
///
/// A `DATE`, `TIME_OF_DAY` or `DATE_AND_TIME` operand of a long form is
/// narrower than the operation and unsigned (ADR-0025), so it compiles at
/// its own width and is zero-extended; loading it at 64 bits directly would
/// sign-extend a date after 2038. Every other operand compiles at `op_type`,
/// which sign-extends a narrower `TIME` as ADR-0001 loads any narrower
/// integer.
fn compile_operand(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op_type: OpType,
    operand: &Expr,
) -> Result<(), Diagnostic> {
    let natural = op_type_from_expr(operand);
    if op_type.0 == OpWidth::W64 && natural == Some((OpWidth::W32, Signedness::Unsigned)) {
        compile_expr(emitter, ctx, operand, (OpWidth::W32, Signedness::Unsigned))?;
        emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        return Ok(());
    }
    compile_expr(emitter, ctx, operand, op_type)
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
    compile_operand(emitter, ctx, op_type, in1)?;
    compile_operand(emitter, ctx, op_type, in2)?;
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
    compile_operand(emitter, ctx, op_type, in1)?;
    compile_operand(emitter, ctx, op_type, in2)?;
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
    compile_operand(emitter, ctx, op_type, in1)?;
    compile_operand(emitter, ctx, op_type, in2)?;
    emit_sub(emitter, op_type);
    load_millis_per_second(emitter, ctx, op_type);
    emit_mul(emitter, op_type);
    Ok(())
}

/// Compiles MUL_TIME and DIV_TIME.
///
/// IN1 is TIME (i32 ms). IN2 is ANY_NUM — codegen inspects IN2's resolved type
/// to select the appropriate instruction sequence. For integer IN2 we use
/// direct i32 multiply/divide. For float IN2 we convert TIME to float, operate,
/// and convert back.
fn compile_mul_div_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let time_op = (OpWidth::W32, Signedness::Signed);

    let in2_op = op_type_from_expr(in2).unwrap_or(time_op);

    match in2_op.0 {
        OpWidth::W32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            compile_expr(emitter, ctx, in2, time_op)?;
            emit_fn(emitter, time_op);
        }
        OpWidth::F32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32);
            compile_expr(emitter, ctx, in2, (OpWidth::F32, Signedness::Signed))?;
            let f32_op = (OpWidth::F32, Signedness::Signed);
            emit_fn(emitter, f32_op);
            emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32);
        }
        OpWidth::F64 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64);
            compile_expr(emitter, ctx, in2, (OpWidth::F64, Signedness::Signed))?;
            let f64_op = (OpWidth::F64, Signedness::Signed);
            emit_fn(emitter, f64_op);
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32);
        }
        OpWidth::W64 => {
            // LINT/ULINT: promote TIME to f64, convert IN2 to f64, operate, convert back.
            // This avoids needing an i64→i32 truncation opcode.
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64);
            compile_expr(emitter, ctx, in2, (OpWidth::W64, in2_op.1))?;
            emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64);
            let f64_op = (OpWidth::F64, Signedness::Signed);
            emit_fn(emitter, f64_op);
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32);
        }
    }

    Ok(())
}

/// Compiles MUL_LTIME and DIV_LTIME.
///
/// IN1 is LTIME (i64 ms), or a TIME that compiles sign-extended. An integer
/// IN2 operates at 64 bits. A float IN2 operates at `LREAL` whatever its
/// width: a duration in milliseconds can need more than the 24 bits of a
/// `REAL` mantissa, so a `REAL` IN2 is widened rather than the duration
/// narrowed.
fn compile_mul_div_ltime(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let ltime_op = (OpWidth::W64, Signedness::Signed);
    let in2_op = op_type_from_expr(in2).unwrap_or(ltime_op);

    compile_operand(emitter, ctx, ltime_op, in1)?;
    match in2_op.0 {
        OpWidth::W32 | OpWidth::W64 => {
            compile_operand(emitter, ctx, (OpWidth::W64, in2_op.1), in2)?;
            emit_fn(emitter, ltime_op);
        }
        OpWidth::F32 | OpWidth::F64 => {
            let f64_op = (OpWidth::F64, Signedness::Signed);
            emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64);
            compile_expr(emitter, ctx, in2, in2_op)?;
            if in2_op.0 == OpWidth::F32 {
                emitter.emit_builtin(opcode::builtin::CONV_F32_TO_F64);
            }
            emit_fn(emitter, f64_op);
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I64);
        }
    }
    Ok(())
}
