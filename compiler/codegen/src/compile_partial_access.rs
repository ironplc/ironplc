//! Bit access (`x.3`, `x.%X3`) and partial access (`x.%B1`, `x.%W0`, ...).
//!
//! Both select `bits` bits of an integer value, starting at bit `shift`; a
//! bit access is the one-bit case. A read loads the base and extracts the
//! bits. An assignment loads the base, replaces the bits, and stores the base
//! back. The base is addressed by [`Place`], whatever its shape.

use ironplc_container::opcode;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, SymbolicVariableKind, Variable};

use super::compile::{CompileContext, OpType, OpWidth, Signedness, DEFAULT_OP_TYPE};
use super::compile_expr::compile_expr;
use super::compile_place::Place;
use crate::emit::Emitter;

/// The bits a bit access or a partial access selects from its base.
pub(crate) struct PartialAccess<'ast> {
    /// The variable the bits are selected from.
    base: &'ast SymbolicVariableKind,
    /// How many bits are selected: 1 for a bit access.
    bits: u32,
    /// The bit the selection starts at.
    shift: u32,
    /// The whole access, for diagnostics.
    span: SourceSpan,
}

impl<'ast> PartialAccess<'ast> {
    /// Returns the access `variable` makes, if it is a bit or partial access.
    pub(crate) fn of(variable: &'ast Variable) -> Option<Self> {
        match variable {
            Variable::Symbolic(symbolic) => Self::of_symbolic(symbolic),
            Variable::Direct(_) => None,
        }
    }

    fn of_symbolic(symbolic: &'ast SymbolicVariableKind) -> Option<Self> {
        match symbolic {
            SymbolicVariableKind::BitAccess(bit_access) => Some(PartialAccess {
                base: &bit_access.variable,
                bits: 1,
                shift: bit_access.index.value as u32,
                span: bit_access.span(),
            }),
            SymbolicVariableKind::PartialAccess(pa) => {
                let bits = pa.size.bit_width();
                Some(PartialAccess {
                    base: &pa.variable,
                    bits,
                    shift: pa.index.value as u32 * bits,
                    span: pa.span(),
                })
            }
            _ => None,
        }
    }

    /// The integer width the bits are read and written at, for a base held
    /// at `base_width`.
    ///
    /// Bits are selected from an integer or bit-string value. The analyzer
    /// rejects a `REAL` or `LREAL` base (P4069), so a float base reaches here
    /// only when analysis was skipped. A partial access of one is refused
    /// rather than masking a float as an integer. A bit access of one has
    /// always been compiled at 32 bits, and still is.
    fn integer_width(&self, base_width: OpWidth) -> Result<OpWidth, Diagnostic> {
        match base_width {
            OpWidth::W32 | OpWidth::W64 => Ok(base_width),
            OpWidth::F32 | OpWidth::F64 if self.bits == 1 => Ok(OpWidth::W32),
            OpWidth::F32 | OpWidth::F64 => Err(Diagnostic::internal_error_at(Label::span(
                self.span.clone(),
                "Partial access on a floating-point variable",
            ))),
        }
    }

    /// Whether the bits are shifted into or out of position. A shift by zero
    /// changes nothing; a partial access skips it, but a bit access has
    /// always emitted it, and still does.
    fn shifts(&self) -> bool {
        self.shift > 0 || self.bits == 1
    }

    /// The type a value written to the bits is compiled at: `BOOL` for one
    /// bit, otherwise the bit string as wide as the bits, so that a `%L`
    /// access takes a 64-bit value and every narrower one a 32-bit value.
    fn value_op_type(&self) -> OpType {
        match self.bits {
            1 => DEFAULT_OP_TYPE,
            2..=32 => (OpWidth::W32, Signedness::Unsigned),
            _ => (OpWidth::W64, Signedness::Unsigned),
        }
    }

    /// A mask of `bits` set bits, starting at bit 0.
    ///
    /// Built at 128 bits so that a selection exactly as wide as its operand
    /// -- `%D` of a `DWORD`, `%L` of an `LWORD` -- does not overflow the
    /// shift. Callers narrow the result to the width they use it at.
    fn mask(&self) -> u128 {
        (1u128 << self.bits) - 1
    }
}

/// Compiles a read of a bit or partial access, leaving the selected bits on
/// the stack, shifted down to bit 0.
pub(crate) fn compile_partial_access_read(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    access: &PartialAccess,
) -> Result<(), Diagnostic> {
    let base_width = match PartialAccess::of_symbolic(access.base) {
        // `d.%B1.3` selects bits of the bits `d.%B1` selects: a value, not a
        // place. It is read at 32 bits.
        Some(inner) => {
            compile_partial_access_read(emitter, ctx, &inner)?;
            DEFAULT_OP_TYPE.0
        }
        None => {
            let place = Place::resolve(ctx, access.base)?;
            place.emit_load(emitter, ctx)?;
            place.op_type().0
        }
    };

    match access.integer_width(base_width)? {
        OpWidth::W64 => {
            if access.shifts() {
                let shift_const = ctx.add_i64_constant(access.shift as i64);
                emitter.emit_load_const_i64(shift_const);
                emitter.emit_builtin(opcode::builtin::SHR_I64);
            }
            if access.bits < 64 {
                let mask_const = ctx.add_i64_constant(access.mask() as i64);
                emitter.emit_load_const_i64(mask_const);
                emitter.emit_bit_and_64();
            }
        }
        _ => {
            if access.shifts() {
                let shift_const = ctx.add_i32_constant(access.shift as i32);
                emitter.emit_load_const_i32(shift_const);
                emitter.emit_builtin(opcode::builtin::SHR_I32);
            }
            if access.bits < 32 {
                let mask_const = ctx.add_i32_constant(access.mask() as i32);
                emitter.emit_load_const_i32(mask_const);
                emitter.emit_bit_and_32();
            }
        }
    }
    Ok(())
}

/// Compiles `access := value`: loads the base, replaces the selected bits
/// with the low bits of `value`, and stores the base back.
pub(crate) fn compile_partial_access_assignment(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    access: &PartialAccess,
    value: &Expr,
) -> Result<(), Diagnostic> {
    let place = Place::resolve(ctx, access.base)?;
    let width = access.integer_width(place.op_type().0)?;
    place.emit_load(emitter, ctx)?;

    let value_wide = access.bits > 32;
    match width {
        OpWidth::W64 => {
            let clear_mask = !((access.mask() << access.shift) as u64) as i64;
            let clear_const = ctx.add_i64_constant(clear_mask);
            emitter.emit_load_const_i64(clear_const);
            emitter.emit_bit_and_64();

            compile_expr(emitter, ctx, value, access.value_op_type())?;
            if value_wide {
                let mask_const = ctx.add_i64_constant(access.mask() as u64 as i64);
                emitter.emit_load_const_i64(mask_const);
                emitter.emit_bit_and_64();
            } else {
                let mask_const = ctx.add_i32_constant(access.mask() as u32 as i32);
                emitter.emit_load_const_i32(mask_const);
                emitter.emit_bit_and_32();
                emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
            }
            if access.shifts() {
                let shift_const = ctx.add_i32_constant(access.shift as i32);
                emitter.emit_load_const_i32(shift_const);
                emitter.emit_builtin(opcode::builtin::SHL_I64);
            }
            emitter.emit_bit_or_64();
        }
        _ => {
            let clear_mask = !((access.mask() << access.shift) as u32) as i32;
            let clear_const = ctx.add_i32_constant(clear_mask);
            emitter.emit_load_const_i32(clear_const);
            emitter.emit_bit_and_32();

            compile_expr(emitter, ctx, value, access.value_op_type())?;
            let mask_const = ctx.add_i32_constant(access.mask() as u32 as i32);
            emitter.emit_load_const_i32(mask_const);
            emitter.emit_bit_and_32();
            if access.shifts() {
                let shift_const = ctx.add_i32_constant(access.shift as i32);
                emitter.emit_load_const_i32(shift_const);
                emitter.emit_builtin(opcode::builtin::SHL_I32);
            }
            emitter.emit_bit_or_32();
        }
    }

    place.emit_store(emitter, ctx)
}
