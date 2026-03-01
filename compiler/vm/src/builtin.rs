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
        _ => Err(Trap::InvalidBuiltinFunction(func_id)),
    }
}
