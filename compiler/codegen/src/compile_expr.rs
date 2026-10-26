//! The typed instructions a value is moved and compared with: a slot loaded
//! or stored at its operation type, a narrow store truncated to its storage
//! width, `NULL`, what a `VAR_IN_OUT` refers to, and the fused compare and
//! branch. The lowered program's expressions and statements are emitted
//! through them (`body/`).

use crate::access::Named;
use ironplc_container::{opcode, VarIndex};
use ironplc_ir::Diagnostic;
use paste::paste;

use super::compile::{CompileContext, OpType, OpWidth, Signedness, VarTypeInfo};
use crate::emit::Emitter;

/// Pushes the null reference, which the VM holds as `u64::MAX`.
pub(crate) fn emit_null(emitter: &mut Emitter, ctx: &mut CompileContext) {
    let pool_index = ctx.add_i64_constant(u64::MAX as i64);
    emitter.emit_load_const_i64(pool_index);
}

/// Loads the value of the variable a `VAR_IN_OUT` parameter refers to.
pub(crate) fn emit_load_in_out(emitter: &mut Emitter, ref_slot: VarIndex) {
    emitter.emit_load_var_i64(ref_slot);
    emitter.emit_load_indirect();
}

/// Result of classifying a comparison expression as a fusable
/// `var <cmp> const` shape eligible for emission via `CMP_BR_*`.
#[derive(Clone, Copy)]
pub(crate) struct ClassifiedCmp {
    /// `cmp_op` byte (one of `opcode::cmp_op::*`), already adjusted so the
    /// variable is on the LHS and the constant on the RHS.
    pub cmp_op_byte: u8,
    /// Variable table index of the LHS.
    pub var_index: VarIndex,
    /// Constant pool index of the RHS literal.
    pub const_idx: u16,
    /// Operand op width: `W32` (→ `CMP_BR_I32`) or `W64` (→ `CMP_BR_I64`).
    pub op_width: OpWidth,
}

pub(crate) fn classify_with_named(
    ctx: &mut CompileContext,
    name: &Named,
    value: i64,
    cmp_op_byte: u8,
) -> Option<ClassifiedCmp> {
    let info = ctx.var_type_info(name)?;
    if info.signedness != Signedness::Signed {
        return None;
    }
    let var_index = ctx.var_index(name).ok()?;
    match info.op_width {
        OpWidth::W32 => {
            let v32 = i32::try_from(value).ok()?;
            let const_idx = ctx.add_i32_constant(v32);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W32,
            })
        }
        OpWidth::W64 => {
            let const_idx = ctx.add_i64_constant(value);
            Some(ClassifiedCmp {
                cmp_op_byte,
                var_index,
                const_idx,
                op_width: OpWidth::W64,
            })
        }
        OpWidth::F32 | OpWidth::F64 => None,
    }
}

/// Emits a `CMP_BR_*` instruction for a previously classified comparison,
/// branching to `target` when the (possibly negated) predicate evaluates
/// to true. When `branch_when_true` is `false`, the comparison operator
/// is negated so the branch fires on the false-polarity (e.g. for
/// "branch to END if NOT cond" zero-trip and IF skip patterns).
pub(crate) fn emit_classified_cmp_br(
    emitter: &mut crate::emit::Emitter,
    classified: ClassifiedCmp,
    branch_when_true: bool,
    target: crate::emit::Label,
) -> Result<(), Diagnostic> {
    let cmp_op_byte = if branch_when_true {
        classified.cmp_op_byte
    } else {
        opcode::cmp_op::negate(classified.cmp_op_byte).ok_or_else(Diagnostic::internal_error)?
    };
    match classified.op_width {
        OpWidth::W32 => emitter.emit_cmp_br_i32(
            cmp_op_byte,
            classified.var_index,
            classified.const_idx,
            target,
        ),
        OpWidth::W64 => emitter.emit_cmp_br_i64(
            cmp_op_byte,
            classified.var_index,
            classified.const_idx,
            target,
        ),
        OpWidth::F32 | OpWidth::F64 => return Err(Diagnostic::internal_error()),
    }
    Ok(())
}

// --- Typed opcode emission helpers ---
//
// Each helper selects the correct opcode based on the operation type
// (width and/or signedness).

pub(crate) fn emit_truncation(emitter: &mut Emitter, type_info: VarTypeInfo) {
    match (
        type_info.op_width,
        type_info.signedness,
        type_info.storage_bits,
    ) {
        (OpWidth::W32, Signedness::Signed, 8) => emitter.emit_trunc_i8(),
        (OpWidth::W32, Signedness::Signed, 16) => emitter.emit_trunc_i16(),
        (OpWidth::W32, Signedness::Unsigned, 8) => emitter.emit_trunc_u8(),
        (OpWidth::W32, Signedness::Unsigned, 16) => emitter.emit_trunc_u16(),
        // 32-bit and 64-bit types fill their native width; no truncation needed.
        _ => {}
    }
}

pub(crate) fn emit_load_var(emitter: &mut Emitter, var_index: VarIndex, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_load_var_i32(var_index),
        OpWidth::W64 => emitter.emit_load_var_i64(var_index),
        OpWidth::F32 => emitter.emit_load_var_f32(var_index),
        OpWidth::F64 => emitter.emit_load_var_f64(var_index),
    }
}

pub(crate) fn emit_store_var(emitter: &mut Emitter, var_index: VarIndex, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_store_var_i32(var_index),
        OpWidth::W64 => emitter.emit_store_var_i64(var_index),
        OpWidth::F32 => emitter.emit_store_var_f32(var_index),
        OpWidth::F64 => emitter.emit_store_var_f64(var_index),
    }
}

// The `emit_<op>` dispatch functions below map an operand `OpType` to the
// matching typed `Emitter` method. They share a handful of identical shapes,
// so the three `macro_rules!` generators fold each shape into a single
// definition. Every generated function keeps the same `pub(crate)` name and
// `(emitter, op_type)` signature that existing call sites and function-pointer
// uses in `compile_call.rs` / `compile_stmt.rs` rely on.

/// Generates a width-only dispatch `emit_<stem>` mapping the operand width to
/// `emit_<stem>_{i32,i64,f32,f64}`. Used for operators whose signedness does
/// not change the opcode.
macro_rules! emit_width_op {
    ($stem:ident) => {
        paste! {
            pub(crate) fn [<emit_ $stem>](emitter: &mut Emitter, op_type: OpType) {
                match op_type.0 {
                    OpWidth::W32 => emitter.[<emit_ $stem _i32>](),
                    OpWidth::W64 => emitter.[<emit_ $stem _i64>](),
                    OpWidth::F32 => emitter.[<emit_ $stem _f32>](),
                    OpWidth::F64 => emitter.[<emit_ $stem _f64>](),
                }
            }
        }
    };
}

/// Generates a width+signedness dispatch `emit_<stem>` mapping
/// `(OpWidth, Signedness)` to `emit_<stem>_{i32,u32,i64,u64,f32,f64}`. Integer
/// widths pick signed/unsigned variants; float widths ignore signedness.
macro_rules! emit_signed_op {
    ($stem:ident) => {
        paste! {
            pub(crate) fn [<emit_ $stem>](emitter: &mut Emitter, op_type: OpType) {
                match op_type {
                    (OpWidth::W32, Signedness::Signed) => emitter.[<emit_ $stem _i32>](),
                    (OpWidth::W32, Signedness::Unsigned) => emitter.[<emit_ $stem _u32>](),
                    (OpWidth::W64, Signedness::Signed) => emitter.[<emit_ $stem _i64>](),
                    (OpWidth::W64, Signedness::Unsigned) => emitter.[<emit_ $stem _u64>](),
                    (OpWidth::F32, _) => emitter.[<emit_ $stem _f32>](),
                    (OpWidth::F64, _) => emitter.[<emit_ $stem _f64>](),
                }
            }
        }
    };
}

/// Generates a logical/bitwise dispatch `emit_<stem>` for AND/OR/XOR. Unsigned
/// integer operands emit the bitwise opcode (`emit_bit_<stem>_32/_64`); every
/// other type (BOOL and signed integers) emits `emit_bool_<stem>`.
macro_rules! emit_logical_op {
    ($stem:ident) => {
        paste! {}
    };
}

emit_width_op!(add);
emit_width_op!(sub);
emit_width_op!(mul);
emit_width_op!(neg);
emit_width_op!(eq);
emit_width_op!(ne);

emit_signed_op!(div);
emit_signed_op!(lt);
emit_signed_op!(le);
emit_signed_op!(gt);
emit_signed_op!(ge);

emit_logical_op!(and);
emit_logical_op!(or);
emit_logical_op!(xor);

// Hand-written one-offs that do not fit the generated shapes above:

/// MOD dispatch. Fits the width+signedness shape for integers, but IEC 61131-3
/// MOD is integer-only, so the float arms are a no-op rather than a call to a
/// (nonexistent) `emit_mod_f32`. The analyzer rejects a float MOD before
/// codegen: the function form through the `MOD` row of the operator-form
/// table (P4026), the operator through `rule_operator_operand_type_check`
/// (P4049).
pub(crate) fn emit_mod(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_mod_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_mod_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_mod_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_mod_u64(),
        // Unreachable from source: the analyzer has already rejected a float
        // MOD (see above). Emitting nothing leaves the operand stack
        // unbalanced, which the bytecode verifier reports as an internal
        // error, so a gap in the analyzer cannot produce a silent miscompile.
        (OpWidth::F32, _) | (OpWidth::F64, _) => {}
    }
}
