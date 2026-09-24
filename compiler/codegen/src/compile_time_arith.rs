//! The typed time and date arithmetic functions: `ADD_TIME`, `SUB_DATE_DATE`,
//! `MUL_TIME` and the rest of IEC 61131-3 Table 30, plus `CONCAT_DATE_TOD`.
//!
//! Each function is one of a few instruction sequences, chosen by the units
//! its operands are stored in (ADR-0025): `TIME` and `TIME_OF_DAY` in
//! milliseconds, `DATE` and `DATE_AND_TIME` in seconds. [`time_arith_for`]
//! names the sequence for a function and [`compile_time_arith`] emits it for
//! two operand expressions, so the sequences do not depend on how the
//! operands were written.

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
    /// A `TIME` multiplied (`true`) or divided (`false`) by an `ANY_NUM`.
    Scale { is_mul: bool },
}

/// Returns the instruction sequence for the typed time or date function
/// `name` (lower case), or `None` when `name` is not one.
pub(crate) fn time_arith_for(name: &str) -> Option<TimeArith> {
    match name {
        "add_time" | "add_tod_time" => Some(TimeArith::SameUnit(emit_add)),
        "sub_time" | "sub_tod_time" | "sub_tod_tod" => Some(TimeArith::SameUnit(emit_sub)),
        "add_dt_time" | "concat_date_tod" => Some(TimeArith::SecondsAndMillis(emit_add)),
        "sub_dt_time" => Some(TimeArith::SecondsAndMillis(emit_sub)),
        "sub_dt_dt" | "sub_date_date" => Some(TimeArith::SecondsDifference),
        "mul_time" => Some(TimeArith::Scale { is_mul: true }),
        "div_time" => Some(TimeArith::Scale { is_mul: false }),
        _ => None,
    }
}

/// Compiles `arith` over the operands `in1` and `in2`, leaving the result on
/// the stack.
pub(crate) fn compile_time_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arith: TimeArith,
    in1: &Expr,
    in2: &Expr,
) -> Result<(), Diagnostic> {
    match arith {
        TimeArith::SameUnit(emit_fn) => compile_same_unit(emitter, ctx, in1, in2, emit_fn),
        TimeArith::SecondsAndMillis(emit_fn) => {
            compile_dt_time_add_sub(emitter, ctx, in1, in2, emit_fn)
        }
        TimeArith::SecondsDifference => compile_sub_to_time(emitter, ctx, in1, in2),
        TimeArith::Scale { is_mul } => compile_mul_div_time(emitter, ctx, in1, in2, is_mul),
    }
}

/// Compiles ADD_TIME, ADD_TOD_TIME, SUB_TIME, SUB_TOD_TIME and SUB_TOD_TOD.
///
/// Both operands are in milliseconds, so the operator applies directly.
fn compile_same_unit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let op_type = (OpWidth::W32, Signedness::Signed);
    compile_expr(emitter, ctx, in1, op_type)?;
    compile_expr(emitter, ctx, in2, op_type)?;
    emit_fn(emitter, op_type);
    Ok(())
}

/// Compiles ADD_DT_TIME, SUB_DT_TIME, and CONCAT_DATE_TOD.
///
/// IN2 (TIME or TOD) is in milliseconds while IN1 (DT or DATE) is in seconds.
/// Converts IN2 from ms to seconds by dividing by 1000, then adds or subtracts.
fn compile_dt_time_add_sub(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
    emit_fn: fn(&mut Emitter, OpType),
) -> Result<(), Diagnostic> {
    let op_type = (OpWidth::W32, Signedness::Signed);
    compile_expr(emitter, ctx, in1, op_type)?;
    compile_expr(emitter, ctx, in2, op_type)?;
    let pool_idx = ctx.add_i32_constant(1000);
    emitter.emit_load_const_i32(pool_idx);
    emit_div(emitter, op_type);
    emit_fn(emitter, op_type);
    Ok(())
}

/// Compiles SUB_DT_DT and SUB_DATE_DATE.
///
/// Subtracts two values in seconds, then multiplies by 1000 to produce TIME
/// in milliseconds.
fn compile_sub_to_time(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    in1: &Expr,
    in2: &Expr,
) -> Result<(), Diagnostic> {
    let op_type = (OpWidth::W32, Signedness::Signed);
    compile_expr(emitter, ctx, in1, op_type)?;
    compile_expr(emitter, ctx, in2, op_type)?;
    emit_sub(emitter, op_type);
    let pool_idx = ctx.add_i32_constant(1000);
    emitter.emit_load_const_i32(pool_idx);
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
    is_mul: bool,
) -> Result<(), Diagnostic> {
    let time_op = (OpWidth::W32, Signedness::Signed);

    let in2_op = op_type_from_expr(in2).unwrap_or(time_op);

    match in2_op.0 {
        OpWidth::W32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            compile_expr(emitter, ctx, in2, time_op)?;
            if is_mul {
                emit_mul(emitter, time_op);
            } else {
                emit_div(emitter, time_op);
            }
        }
        OpWidth::F32 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32);
            compile_expr(emitter, ctx, in2, (OpWidth::F32, Signedness::Signed))?;
            let f32_op = (OpWidth::F32, Signedness::Signed);
            if is_mul {
                emit_mul(emitter, f32_op);
            } else {
                emit_div(emitter, f32_op);
            }
            emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32);
        }
        OpWidth::F64 => {
            compile_expr(emitter, ctx, in1, time_op)?;
            emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64);
            compile_expr(emitter, ctx, in2, (OpWidth::F64, Signedness::Signed))?;
            let f64_op = (OpWidth::F64, Signedness::Signed);
            if is_mul {
                emit_mul(emitter, f64_op);
            } else {
                emit_div(emitter, f64_op);
            }
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
            if is_mul {
                emit_mul(emitter, f64_op);
            } else {
                emit_div(emitter, f64_op);
            }
            emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32);
        }
    }

    Ok(())
}
