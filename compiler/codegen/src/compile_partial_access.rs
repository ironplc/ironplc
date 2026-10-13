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

use super::compile::{CompileContext, OpWidth, DEFAULT_OP_TYPE};
use super::compile_expr::compile_expr;
use super::compile_place::Place;
use crate::emit::Emitter;

/// The bits a bit access or a partial access selects from its base.
pub(crate) struct PartialAccess<'ast> {
    /// The variable the bits are selected from.
    base: &'ast SymbolicVariableKind,
    /// Which bits of the base are selected.
    selection: Bits,
    /// The whole access, for diagnostics.
    span: SourceSpan,
}

/// `bits` bits of an integer value, starting at bit `shift`.
#[derive(Clone, Copy)]
pub(crate) struct Bits {
    /// How many bits are selected: 1 for a bit access.
    bits: u32,
    /// The bit the selection starts at.
    shift: u32,
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
                selection: Bits::new(bit_access.index.value as u32, 1),
                span: bit_access.span(),
            }),
            SymbolicVariableKind::PartialAccess(pa) => {
                let bits = pa.size.bit_width();
                Some(PartialAccess {
                    base: &pa.variable,
                    selection: Bits::new(pa.index.value as u32 * bits, bits),
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
            OpWidth::F32 | OpWidth::F64 if self.selection.bits == 1 => Ok(OpWidth::W32),
            OpWidth::F32 | OpWidth::F64 => Err(Diagnostic::internal_error_at(Label::span(
                self.span.clone(),
                "Partial access on a floating-point variable",
            ))),
        }
    }
}

impl Bits {
    /// `bits` bits starting at bit `shift`.
    pub(crate) fn new(shift: u32, bits: u32) -> Self {
        Self { bits, shift }
    }

    /// Whether the bits are shifted into or out of position. A shift by zero
    /// changes nothing; a partial access skips it, but a bit access has
    /// always emitted it, and still does.
    fn shifts(&self) -> bool {
        self.shift > 0 || self.bits == 1
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
            _ => {
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

    let selection = access.selection;
    match access.integer_width(base_width)? {
        OpWidth::W64 => {
            if selection.shifts() {
                let shift_const = ctx.add_i64_constant(selection.shift as i64);
                emitter.emit_load_const_i64(shift_const);
                emitter.emit_builtin(opcode::builtin::SHR_I64)?;
            }
            if selection.bits < 64 {
                let mask_const = ctx.add_i64_constant(selection.mask() as i64);
                emitter.emit_load_const_i64(mask_const);
                emitter.emit_bit_and_64();
            }
        }
        _ => {
            if selection.shifts() {
                let shift_const = ctx.add_i32_constant(selection.shift as i32);
                emitter.emit_load_const_i32(shift_const);
                emitter.emit_builtin(opcode::builtin::SHR_I32)?;
            }
            if selection.bits < 32 {
                let mask_const = ctx.add_i32_constant(selection.mask() as i32);
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

    access
        .selection
        .emit_insert(emitter, ctx, width, |emitter, ctx| {
            compile_expr(emitter, ctx, value)
        })?;
    place.emit_store(emitter, ctx)
}
