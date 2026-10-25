//! What a call of a user-defined function or method passes for an argument:
//! a string copied into the parameter's slot, and a reference to the caller's
//! variable for a `VAR_IN_OUT`.

use crate::access::Named;
use ironplc_ir::Diagnostic;

use super::compile::{CompileContext, StringParamInfo};
use crate::emit::Emitter;

/// Copies a string argument into the string parameter `param`, then pushes
/// a dummy value for the slot `CALL` or `METHOD_CALL` pops. `source` puts
/// the argument in a slot of the data region and returns the slot's offset;
/// it runs once the parameter's header is initialized, so the caller
/// decides how the argument is resolved.
pub(crate) fn copy_string_arg(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    param: &StringParamInfo,
    source: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<u32, Diagnostic>,
) -> Result<(), Diagnostic> {
    // Initialize the destination header, then copy the string data.
    emitter.emit_str_init(param.data_offset, param.max_length, param.char_width);
    let src_offset = source(emitter, ctx)?;
    emitter.emit_str_load_var(src_offset);
    emitter.emit_str_store_var(param.data_offset);

    let zero_idx = ctx.add_i32_constant(0);
    emitter.emit_load_const_i32(zero_idx);
    Ok(())
}

/// Whether this backend passes the variable `name` to a `VAR_IN_OUT`
/// parameter: a variable of an elementary type, which has type info and
/// lives in its own slot, unlike a string, array, structure or instance,
/// which lives in the data region. A `VAR_IN_OUT` parameter of the function
/// being compiled is one.
pub(crate) fn passes_by_reference(ctx: &CompileContext, name: &Named) -> bool {
    ctx.var_type_info(name).is_some()
        && !ctx.string_var(name).is_some()
        && !ctx.array_var(name).is_some()
        && !ctx.struct_var(name).is_some()
        && !ctx.struct_array_var(name).is_some()
        && !ctx.fb_instance(name).is_some()
}

/// Pushes a reference to the variable `name`, which
/// [`passes_by_reference`]: its own, or the one a `VAR_IN_OUT` parameter's
/// slot holds, passed on.
pub(crate) fn emit_reference(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    name: &Named,
) -> Result<(), Diagnostic> {
    if let Some(ref_slot) = ctx.in_out_ref_slot(name) {
        emitter.emit_load_var_i64(ref_slot);
    } else {
        let reference = crate::compile_reference::reference_to(ctx, name)?;
        let pool_index = ctx.add_i64_constant(reference as i64);
        emitter.emit_load_const_i64(pool_index);
    }
    Ok(())
}
