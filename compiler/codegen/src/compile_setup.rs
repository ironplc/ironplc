//! The setup of the storage the layout gives a variable before its starting
//! value is stored, by `init` and by each prologue from the lowered program
//! (`body/init.rs`, `body/prologue.rs`). The layout itself is decided
//! first (`layout/`).

use crate::access::Named;
use ironplc_container::VarIndex;
use ironplc_ir::lowered::TypeTable;
use ironplc_ir::Diagnostic;

use super::compile::{CompileContext, OpType, OpWidth};
use super::compile_expr::emit_store_var;
use crate::emit::Emitter;

/// Sets up the storage the layout gives the variable `id`, before its
/// starting value is stored (REQ-LOW-codegen-200): the data-region offset
/// the slot of an aggregate or an instance holds, the offset of each
/// instance an instance's fields hold, and the header of each string it
/// holds, of its type in `types`. A scalar's slot needs none.
pub(crate) fn set_up_storage(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    types: &TypeTable,
    id: &Named,
) -> Result<(), Diagnostic> {
    let span = &id.span();
    if let Some(info) = ctx.struct_var(id).cloned() {
        return crate::compile_struct_init::set_up_struct_variable(
            emitter, ctx, types, &info, span,
        );
    }
    if let Some(info) = ctx.struct_array_var(id).cloned() {
        return crate::compile_struct_init::set_up_struct_array_variable(emitter, ctx, &info, span);
    }
    if let Some(array) = ctx.array_var(id).filter(|array| !array.is_ref) {
        let (data_offset, var_index, desc_index, is_string) = (
            array.data_offset,
            array.var_index,
            array.desc_index,
            array.is_string_element,
        );
        let offset_const = ctx.add_i32_constant(data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(var_index);
        if is_string {
            emitter.emit_str_init_array(var_index, desc_index);
        }
        return Ok(());
    }
    if let Some(info) = ctx.string_var(id) {
        emitter.emit_str_init(info.data_offset, info.max_length, info.char_width);
        return Ok(());
    }
    if let Some(info) = ctx.fb_instance(id) {
        let (data_offset, var_index, held) = (info.data_offset, info.var_index, info.held.clone());
        let offset_const = ctx.add_i32_constant(data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(var_index);
        crate::held_instances::emit_offsets(emitter, ctx, &held);
    }
    Ok(())
}

/// Clears the result slot of a method that returns nothing, which codegen
/// allocates for it, in the slot `var_index`, operated at `op_type`, as a
/// result is cleared.
pub(crate) fn emit_clear_result_slot(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    var_index: VarIndex,
    op_type: OpType,
) {
    emit_zero_const(emitter, ctx, op_type);
    emit_store_var(emitter, var_index, op_type);
}

/// Emits a LOAD_CONST instruction that pushes a zero value of the given type.
pub(crate) fn emit_zero_const(emitter: &mut Emitter, ctx: &mut CompileContext, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => {
            let pool_index = ctx.add_i32_constant(0);
            emitter.emit_load_const_i32(pool_index);
        }
        OpWidth::W64 => {
            let pool_index = ctx.add_i64_constant(0);
            emitter.emit_load_const_i64(pool_index);
        }
        OpWidth::F32 => {
            let pool_index = ctx.add_f32_constant(0.0);
            emitter.emit_load_const_f32(pool_index);
        }
        OpWidth::F64 => {
            let pool_index = ctx.add_f64_constant(0.0);
            emitter.emit_load_const_f64(pool_index);
        }
    }
}
