//! Built-in standard library function dispatch.
//!
//! Handles execution of the BUILTIN opcode by dispatching on the function ID.

use ironplc_container::opcode;

use crate::error::Trap;
use crate::stack::OperandStack;
use crate::value::Slot;

/// Dispatches a built-in function call by `func_id`.
///
/// Pops arguments from and pushes results onto the operand stack.
/// Returns `Err(Trap)` for invalid function IDs or runtime errors
/// (e.g. negative exponent).
#[inline]
pub fn dispatch(func_id: u16, stack: &mut OperandStack) -> Result<(), Trap> {
    match func_id {
        opcode::builtin::EXPT_I32 => {
            let b = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            if b < 0 {
                return Err(Trap::NegativeExponent);
            }
            stack.push(Slot::from_i32(a.wrapping_pow(b as u32)))?;
            Ok(())
        }
        opcode::builtin::EXPT_F32 => {
            let b = stack.pop()?.as_f32();
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.powf(b)))?;
            Ok(())
        }
        opcode::builtin::EXPT_F64 => {
            let b = stack.pop()?.as_f64();
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.powf(b)))?;
            Ok(())
        }
        opcode::builtin::ABS_I32 => {
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(a.wrapping_abs()))?;
            Ok(())
        }
        opcode::builtin::MIN_I32 => {
            let b = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(a.min(b)))?;
            Ok(())
        }
        opcode::builtin::MAX_I32 => {
            let b = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(a.max(b)))?;
            Ok(())
        }
        opcode::builtin::LIMIT_I32 => {
            let mx = stack.pop()?.as_i32();
            let in_val = stack.pop()?.as_i32();
            let mn = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(in_val.clamp(mn, mx)))?;
            Ok(())
        }
        opcode::builtin::SEL_I32 => {
            let in1 = stack.pop()?.as_i32();
            let in0 = stack.pop()?.as_i32();
            let g = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(if g == 0 { in0 } else { in1 }))?;
            Ok(())
        }
        opcode::builtin::SHL_I32 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32(a.wrapping_shl(n as u32)))?;
            Ok(())
        }
        opcode::builtin::SHL_I64 => {
            let n = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(a.wrapping_shl(n as u32)))?;
            Ok(())
        }
        opcode::builtin::SHR_I32 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            // Logical shift right: treat as unsigned to fill with zeros.
            stack.push(Slot::from_i32(((a as u32).wrapping_shr(n as u32)) as i32))?;
            Ok(())
        }
        opcode::builtin::SHR_I64 => {
            let n = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(((a as u64).wrapping_shr(n as u32)) as i64))?;
            Ok(())
        }
        opcode::builtin::ROL_I32 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32((a as u32).rotate_left(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ROL_I64 => {
            let n = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64((a as u64).rotate_left(n as u32) as i64))?;
            Ok(())
        }
        opcode::builtin::ROR_I32 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_i32((a as u32).rotate_right(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ROR_I64 => {
            let n = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64((a as u64).rotate_right(n as u32) as i64))?;
            Ok(())
        }
        opcode::builtin::ROL_U8 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32() as u8;
            stack.push(Slot::from_i32(a.rotate_left(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ROL_U16 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32() as u16;
            stack.push(Slot::from_i32(a.rotate_left(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ROR_U8 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32() as u8;
            stack.push(Slot::from_i32(a.rotate_right(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ROR_U16 => {
            let n = stack.pop()?.as_i32();
            let a = stack.pop()?.as_i32() as u16;
            stack.push(Slot::from_i32(a.rotate_right(n as u32) as i32))?;
            Ok(())
        }
        opcode::builtin::ABS_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.abs()))?;
            Ok(())
        }
        opcode::builtin::ABS_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.abs()))?;
            Ok(())
        }
        opcode::builtin::MIN_F32 => {
            let b = stack.pop()?.as_f32();
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.min(b)))?;
            Ok(())
        }
        opcode::builtin::MIN_F64 => {
            let b = stack.pop()?.as_f64();
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.min(b)))?;
            Ok(())
        }
        opcode::builtin::MAX_F32 => {
            let b = stack.pop()?.as_f32();
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.max(b)))?;
            Ok(())
        }
        opcode::builtin::MAX_F64 => {
            let b = stack.pop()?.as_f64();
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.max(b)))?;
            Ok(())
        }
        opcode::builtin::LIMIT_F32 => {
            let mx = stack.pop()?.as_f32();
            let in_val = stack.pop()?.as_f32();
            let mn = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(float_clamp_f32(in_val, mn, mx)))?;
            Ok(())
        }
        opcode::builtin::LIMIT_F64 => {
            let mx = stack.pop()?.as_f64();
            let in_val = stack.pop()?.as_f64();
            let mn = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(float_clamp_f64(in_val, mn, mx)))?;
            Ok(())
        }
        opcode::builtin::SEL_F32 => {
            let in1 = stack.pop()?.as_f32();
            let in0 = stack.pop()?.as_f32();
            let g = stack.pop()?.as_i32();
            stack.push(Slot::from_f32(if g == 0 { in0 } else { in1 }))?;
            Ok(())
        }
        opcode::builtin::SEL_F64 => {
            let in1 = stack.pop()?.as_f64();
            let in0 = stack.pop()?.as_f64();
            let g = stack.pop()?.as_i32();
            stack.push(Slot::from_f64(if g == 0 { in0 } else { in1 }))?;
            Ok(())
        }
        opcode::builtin::SQRT_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.sqrt()))?;
            Ok(())
        }
        opcode::builtin::SQRT_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.sqrt()))?;
            Ok(())
        }
        _ => Err(Trap::InvalidBuiltinFunction(func_id)),
    }
}

/// IEEE 754-safe clamp for f32. Unlike `f32::clamp`, this does not panic
/// when `mn`, `mx`, or `val` is NaN. NaN propagates: if any input is NaN
/// the result is NaN.
#[inline]
fn float_clamp_f32(val: f32, mn: f32, mx: f32) -> f32 {
    if val.is_nan() || mn.is_nan() || mx.is_nan() {
        return f32::NAN;
    }
    if val < mn {
        mn
    } else if val > mx {
        mx
    } else {
        val
    }
}

/// IEEE 754-safe clamp for f64. See [`float_clamp_f32`] for semantics.
#[inline]
fn float_clamp_f64(val: f64, mn: f64, mx: f64) -> f64 {
    if val.is_nan() || mn.is_nan() || mx.is_nan() {
        return f64::NAN;
    }
    if val < mn {
        mn
    } else if val > mx {
        mx
    } else {
        val
    }
}
