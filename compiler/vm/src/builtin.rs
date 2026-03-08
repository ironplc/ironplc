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
        opcode::builtin::LN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.ln()))?;
            Ok(())
        }
        opcode::builtin::LN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.ln()))?;
            Ok(())
        }
        opcode::builtin::LOG_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.log10()))?;
            Ok(())
        }
        opcode::builtin::LOG_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.log10()))?;
            Ok(())
        }
        opcode::builtin::EXP_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.exp()))?;
            Ok(())
        }
        opcode::builtin::EXP_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.exp()))?;
            Ok(())
        }
        opcode::builtin::SIN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.sin()))?;
            Ok(())
        }
        opcode::builtin::SIN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.sin()))?;
            Ok(())
        }
        opcode::builtin::COS_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.cos()))?;
            Ok(())
        }
        opcode::builtin::COS_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.cos()))?;
            Ok(())
        }
        opcode::builtin::TAN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.tan()))?;
            Ok(())
        }
        opcode::builtin::TAN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.tan()))?;
            Ok(())
        }
        opcode::builtin::ASIN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.asin()))?;
            Ok(())
        }
        opcode::builtin::ASIN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.asin()))?;
            Ok(())
        }
        opcode::builtin::ACOS_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.acos()))?;
            Ok(())
        }
        opcode::builtin::ACOS_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.acos()))?;
            Ok(())
        }
        opcode::builtin::ATAN_F32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f32(a.atan()))?;
            Ok(())
        }
        opcode::builtin::ATAN_F64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f64(a.atan()))?;
            Ok(())
        }
        opcode::builtin::EXPT_I64 => {
            let b = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            if b < 0 {
                return Err(Trap::NegativeExponent);
            }
            stack.push(Slot::from_i64(a.wrapping_pow(b as u32)))?;
            Ok(())
        }
        opcode::builtin::ABS_I64 => {
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(a.wrapping_abs()))?;
            Ok(())
        }
        opcode::builtin::MIN_I64 => {
            let b = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(a.min(b)))?;
            Ok(())
        }
        opcode::builtin::MAX_I64 => {
            let b = stack.pop()?.as_i64();
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(a.max(b)))?;
            Ok(())
        }
        opcode::builtin::LIMIT_I64 => {
            let mx = stack.pop()?.as_i64();
            let in_val = stack.pop()?.as_i64();
            let mn = stack.pop()?.as_i64();
            stack.push(Slot::from_i64(in_val.clamp(mn, mx)))?;
            Ok(())
        }
        opcode::builtin::SEL_I64 => {
            let in1 = stack.pop()?.as_i64();
            let in0 = stack.pop()?.as_i64();
            let g = stack.pop()?.as_i32();
            stack.push(Slot::from_i64(if g == 0 { in0 } else { in1 }))?;
            Ok(())
        }
        opcode::builtin::MIN_U32 => {
            let b = stack.pop()?.as_i32() as u32;
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i32(a.min(b) as i32))?;
            Ok(())
        }
        opcode::builtin::MAX_U32 => {
            let b = stack.pop()?.as_i32() as u32;
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i32(a.max(b) as i32))?;
            Ok(())
        }
        opcode::builtin::LIMIT_U32 => {
            let mx = stack.pop()?.as_i32() as u32;
            let in_val = stack.pop()?.as_i32() as u32;
            let mn = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i32(in_val.clamp(mn, mx) as i32))?;
            Ok(())
        }
        opcode::builtin::MIN_U64 => {
            let b = stack.pop()?.as_i64() as u64;
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_i64(a.min(b) as i64))?;
            Ok(())
        }
        opcode::builtin::MAX_U64 => {
            let b = stack.pop()?.as_i64() as u64;
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_i64(a.max(b) as i64))?;
            Ok(())
        }
        opcode::builtin::LIMIT_U64 => {
            let mx = stack.pop()?.as_i64() as u64;
            let in_val = stack.pop()?.as_i64() as u64;
            let mn = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_i64(in_val.clamp(mn, mx) as i64))?;
            Ok(())
        }
        // --- Type conversion opcodes ---
        opcode::builtin::CONV_I32_TO_F32 => {
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_I32_TO_F64 => {
            let a = stack.pop()?.as_i32();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_I64_TO_F32 => {
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_I64_TO_F64 => {
            let a = stack.pop()?.as_i64();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_F32 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_F64 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_U64_TO_F32 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U64_TO_F64 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_I32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i32(a as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_I64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i64(a as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_I32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i32(a as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_I64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i64(a as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_U32 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i32(a as u32 as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_U64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_U32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i32(a as u32 as i32))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_U64 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
        opcode::builtin::CONV_F32_TO_F64 => {
            let a = stack.pop()?.as_f32();
            stack.push(Slot::from_f64(a as f64))?;
            Ok(())
        }
        opcode::builtin::CONV_F64_TO_F32 => {
            let a = stack.pop()?.as_f64();
            stack.push(Slot::from_f32(a as f32))?;
            Ok(())
        }
        opcode::builtin::CONV_U32_TO_I64 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i64(a as u64 as i64))?;
            Ok(())
        }
        // --- BCD conversion opcodes ---
        opcode::builtin::BCD_TO_INT_8 => {
            let a = stack.pop()?.as_i32() as u8;
            stack.push(Slot::from_i32(bcd_to_int_u8(a) as i32))?;
            Ok(())
        }
        opcode::builtin::BCD_TO_INT_16 => {
            let a = stack.pop()?.as_i32() as u16;
            stack.push(Slot::from_i32(bcd_to_int_u16(a) as i32))?;
            Ok(())
        }
        opcode::builtin::BCD_TO_INT_32 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i32(bcd_to_int_u32(a) as i32))?;
            Ok(())
        }
        opcode::builtin::BCD_TO_INT_64 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_i64(bcd_to_int_u64(a) as i64))?;
            Ok(())
        }
        opcode::builtin::INT_TO_BCD_8 => {
            let a = stack.pop()?.as_i32() as u8;
            stack.push(Slot::from_i32(int_to_bcd_u8(a) as i32))?;
            Ok(())
        }
        opcode::builtin::INT_TO_BCD_16 => {
            let a = stack.pop()?.as_i32() as u16;
            stack.push(Slot::from_i32(int_to_bcd_u16(a) as i32))?;
            Ok(())
        }
        opcode::builtin::INT_TO_BCD_32 => {
            let a = stack.pop()?.as_i32() as u32;
            stack.push(Slot::from_i32(int_to_bcd_u32(a) as i32))?;
            Ok(())
        }
        opcode::builtin::INT_TO_BCD_64 => {
            let a = stack.pop()?.as_i64() as u64;
            stack.push(Slot::from_i64(int_to_bcd_u64(a) as i64))?;
            Ok(())
        }
        // MUX (multiplexer) for all type widths
        id if opcode::builtin::is_mux(id) => {
            let n = opcode::builtin::mux_info(id).unwrap() as usize;
            if id >= opcode::builtin::MUX_F64_BASE {
                dispatch_mux_f64(n, stack)
            } else if id >= opcode::builtin::MUX_F32_BASE {
                dispatch_mux_f32(n, stack)
            } else if id >= opcode::builtin::MUX_I64_BASE {
                dispatch_mux_i64(n, stack)
            } else {
                dispatch_mux_i32(n, stack)
            }
        }
        _ => Err(Trap::InvalidBuiltinFunction(func_id)),
    }
}

/// Dispatches MUX for 32-bit integer values.
///
/// Stack layout (top to bottom): IN(n-1), ..., IN1, IN0, K
/// Pops all n IN values and K, pushes IN[K] (clamped to 0..n-1).
fn dispatch_mux_i32(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    // Pop IN values in reverse order (last pushed = first popped)
    let mut inputs = vec![0i32; n];
    for i in (0..n).rev() {
        inputs[i] = stack.pop()?.as_i32();
    }
    let k = stack.pop()?.as_i32();
    // Clamp K to valid range
    let idx = (k.max(0) as usize).min(n - 1);
    stack.push(Slot::from_i32(inputs[idx]))?;
    Ok(())
}

/// Dispatches MUX for 64-bit integer values.
fn dispatch_mux_i64(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    let mut inputs = vec![0i64; n];
    for i in (0..n).rev() {
        inputs[i] = stack.pop()?.as_i64();
    }
    let k = stack.pop()?.as_i32(); // K is always i32
    let idx = (k.max(0) as usize).min(n - 1);
    stack.push(Slot::from_i64(inputs[idx]))?;
    Ok(())
}

/// Dispatches MUX for 32-bit float values.
fn dispatch_mux_f32(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    let mut inputs = vec![0.0f32; n];
    for i in (0..n).rev() {
        inputs[i] = stack.pop()?.as_f32();
    }
    let k = stack.pop()?.as_i32(); // K is always i32
    let idx = (k.max(0) as usize).min(n - 1);
    stack.push(Slot::from_f32(inputs[idx]))?;
    Ok(())
}

/// Dispatches MUX for 64-bit float values.
fn dispatch_mux_f64(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    let mut inputs = vec![0.0f64; n];
    for i in (0..n).rev() {
        inputs[i] = stack.pop()?.as_f64();
    }
    let k = stack.pop()?.as_i32(); // K is always i32
    let idx = (k.max(0) as usize).min(n - 1);
    stack.push(Slot::from_f64(inputs[idx]))?;
    Ok(())
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

// =============================================================================
// BCD conversion helpers
// =============================================================================

/// Decodes a BCD-encoded u8 (2 digits, max 99) to its integer value.
/// Invalid BCD nibbles (>9) are treated as 0.
#[inline]
fn bcd_to_int_u8(bcd: u8) -> u8 {
    let d1 = (bcd >> 4) & 0x0F;
    let d0 = bcd & 0x0F;
    let d1 = if d1 > 9 { 0 } else { d1 };
    let d0 = if d0 > 9 { 0 } else { d0 };
    d1 * 10 + d0
}

/// Decodes a BCD-encoded u16 (4 digits, max 9999) to its integer value.
#[inline]
fn bcd_to_int_u16(bcd: u16) -> u16 {
    let mut result: u16 = 0;
    let mut shift = 12;
    let mut multiplier: u16 = 1000;
    for _ in 0..3 {
        let nibble = (bcd >> shift) & 0x0F;
        let nibble = if nibble > 9 { 0 } else { nibble };
        result += nibble * multiplier;
        shift -= 4;
        multiplier /= 10;
    }
    let nibble = bcd & 0x0F;
    let nibble = if nibble > 9 { 0 } else { nibble };
    result + nibble
}

/// Decodes a BCD-encoded u32 (8 digits, max 99_999_999) to its integer value.
#[inline]
fn bcd_to_int_u32(bcd: u32) -> u32 {
    let mut result: u32 = 0;
    for i in 0..8 {
        let nibble = (bcd >> (4 * (7 - i))) & 0x0F;
        let nibble = if nibble > 9 { 0 } else { nibble };
        result = result * 10 + nibble;
    }
    result
}

/// Decodes a BCD-encoded u64 (16 digits) to its integer value.
#[inline]
fn bcd_to_int_u64(bcd: u64) -> u64 {
    let mut result: u64 = 0;
    for i in 0..16 {
        let nibble = (bcd >> (4 * (15 - i))) & 0x0F;
        let nibble = if nibble > 9 { 0 } else { nibble };
        result = result * 10 + nibble;
    }
    result
}

/// Encodes a u8 value (max 99) as BCD. Values > 99 wrap.
#[inline]
fn int_to_bcd_u8(val: u8) -> u8 {
    let val = val % 100;
    ((val / 10) << 4) | (val % 10)
}

/// Encodes a u16 value (max 9999) as BCD. Values > 9999 wrap.
#[inline]
fn int_to_bcd_u16(val: u16) -> u16 {
    let val = val % 10000;
    let mut result: u16 = 0;
    let mut remaining = val;
    for i in 0..4 {
        let digit = remaining % 10;
        result |= digit << (4 * i);
        remaining /= 10;
    }
    result
}

/// Encodes a u32 value (max 99_999_999) as BCD. Values > 99_999_999 wrap.
#[inline]
fn int_to_bcd_u32(val: u32) -> u32 {
    let val = val % 100_000_000;
    let mut result: u32 = 0;
    let mut remaining = val;
    for i in 0..8 {
        let digit = remaining % 10;
        result |= digit << (4 * i);
        remaining /= 10;
    }
    result
}

/// Encodes a u64 value as BCD (16 digits).
#[inline]
fn int_to_bcd_u64(val: u64) -> u64 {
    let val = val % 10_000_000_000_000_000;
    let mut result: u64 = 0;
    let mut remaining = val;
    for i in 0..16 {
        let digit = remaining % 10;
        result |= digit << (4 * i);
        remaining /= 10;
    }
    result
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
