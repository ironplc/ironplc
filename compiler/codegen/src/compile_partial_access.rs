//! Bit access (`x.3`, `x.%X3`) and partial access (`x.%B1`, `x.%W0`, ...).
//!
//! Both select `bits` bits of an integer value, starting at bit `shift`; a
//! bit access is the one-bit case. An assignment, `AssignBits`, loads the
//! base, replaces the bits, and stores the base back; a read is the lowered
//! program's shift and mask.

use ironplc_container::opcode;
use ironplc_ir::Diagnostic;

use super::compile::{CompileContext, OpWidth};
use crate::emit::Emitter;

/// `bits` bits of an integer value, starting at bit `shift`.
#[derive(Clone, Copy)]
pub(crate) struct Bits {
    /// How many bits are selected: 1 for a bit access.
    bits: u32,
    /// The bit the selection starts at.
    shift: u32,
}

impl Bits {
    /// `bits` bits starting at bit `shift`.
    pub(crate) fn new(shift: u32, bits: u32) -> Self {
        Self { bits, shift }
    }

    /// Whether the bits are shifted into position. A shift by zero changes
    /// nothing, so it is skipped.
    fn shifts(&self) -> bool {
        self.shift > 0
    }

    /// A mask of `bits` set bits, starting at bit 0.
    ///
    /// Built at 128 bits so that a selection exactly as wide as its operand
    /// -- `%D` of a `DWORD`, `%L` of an `LWORD` -- does not overflow the
    /// shift. Callers narrow the result to the width they use it at.
    fn mask(&self) -> u128 {
        (1u128 << self.bits) - 1
    }

    /// Replaces these bits of the integer value on the stack, held at
    /// `width`, with the low bits of the value `emit_value` pushes, and
    /// leaves the result. That value is 64 bits wide when more than 32 bits
    /// are selected, and 32 bits wide or a `BOOL` otherwise.
    pub(crate) fn emit_insert(
        self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        width: OpWidth,
        emit_value: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        let value_wide = self.bits > 32;
        match width {
            OpWidth::W64 => {
                let clear_mask = !((self.mask() << self.shift) as u64) as i64;
                let clear_const = ctx.add_i64_constant(clear_mask);
                emitter.emit_load_const_i64(clear_const);
                emitter.emit_bit_and_64();

                emit_value(emitter, ctx)?;
                if value_wide {
                    let mask_const = ctx.add_i64_constant(self.mask() as u64 as i64);
                    emitter.emit_load_const_i64(mask_const);
                    emitter.emit_bit_and_64();
                } else {
                    let mask_const = ctx.add_i32_constant(self.mask() as u32 as i32);
                    emitter.emit_load_const_i32(mask_const);
                    emitter.emit_bit_and_32();
                    emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64)?;
                }
                if self.shifts() {
                    let shift_const = ctx.add_i32_constant(self.shift as i32);
                    emitter.emit_load_const_i32(shift_const);
                    emitter.emit_builtin(opcode::builtin::SHL_I64)?;
                }
                emitter.emit_bit_or_64();
            }
            OpWidth::W32 | OpWidth::F32 | OpWidth::F64 => {
                let clear_mask = !((self.mask() << self.shift) as u32) as i32;
                let clear_const = ctx.add_i32_constant(clear_mask);
                emitter.emit_load_const_i32(clear_const);
                emitter.emit_bit_and_32();

                emit_value(emitter, ctx)?;
                let mask_const = ctx.add_i32_constant(self.mask() as u32 as i32);
                emitter.emit_load_const_i32(mask_const);
                emitter.emit_bit_and_32();
                if self.shifts() {
                    let shift_const = ctx.add_i32_constant(self.shift as i32);
                    emitter.emit_load_const_i32(shift_const);
                    emitter.emit_builtin(opcode::builtin::SHL_I32)?;
                }
                emitter.emit_bit_or_32();
            }
        }
        Ok(())
    }
}
