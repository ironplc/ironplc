//! Emits a lowered `Copy` of a whole function block instance (`f := g`).
//!
//! An instance's slot holds the data region offset of its fields, so storing
//! one slot in the other would make the destination an alias of the source,
//! as the route that reads the AST does. A `Copy` gives the destination the
//! source's values in storage of its own (REQ-LOW-codegen-165), so the bridge
//! copies the instance field by field, with `FB_LOAD_PARAM` and
//! `FB_STORE_PARAM`: every field of the block, its inputs, outputs and
//! variables, and for a standard block the fields the VM's block keeps for
//! itself, such as a timer's start time. A field is one slot, so each is
//! copied whole. Only an instance this backend reaches by name is copied, as
//! only one is called (`fb_call.rs`).
//!
//! A function's body sees the program's instances by name, and its own
//! structures may share their names, so a `Copy` is taken for one of
//! instances only when its places are function block instances.

use ironplc_container::{FbTypeId, VarIndex};
use ironplc_dsl::core::Id;
use ironplc_ir::lowered::{Place as LoweredPlace, TypeKind};
use ironplc_lowering::Lowering;

use super::place::variable_name;
use crate::compile::CompileContext;
use crate::compile_call::standard_fb_layout;
use crate::emit::Emitter;

/// The copy of one instance to another of its block.
pub(super) struct InstanceCopy {
    dst: VarIndex,
    src: VarIndex,
    /// How many fields the block's instances have; at most 256, as
    /// `FB_LOAD_PARAM` and `FB_STORE_PARAM` name a field by a byte.
    fields: u16,
}

/// The copy of `src` to `dst` when both are instances this backend reaches
/// by name, of one block. `None` for any other `Copy`.
pub(super) fn instance_copy(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    dst: &LoweredPlace,
    src: &LoweredPlace,
) -> Option<InstanceCopy> {
    // A `Copy`'s places have one type.
    if !matches!(
        lowering.types().kind(dst.ty()),
        Some(TypeKind::FunctionBlock { .. })
    ) {
        return None;
    }
    let instance = |place| {
        let name: Id = variable_name(lowering, ctx, place)?;
        ctx.fb_instance(&name)
    };
    let (dst, src) = (instance(dst)?, instance(src)?);
    if dst.type_id != src.type_id {
        return None;
    }
    let fields = field_count(ctx, dst.type_id)?;
    Some(InstanceCopy {
        dst: dst.var_index,
        src: src.var_index,
        fields: u16::try_from(fields).ok().filter(|fields| *fields <= 256)?,
    })
}

/// Emits `copy`: the destination pushed once, then for each field the
/// source's field loaded and stored in the destination's, then the
/// destination popped.
pub(super) fn emit_instance_copy(emitter: &mut Emitter, copy: &InstanceCopy) {
    emitter.emit_fb_load_instance(copy.dst);
    for field in (0..copy.fields).filter_map(|field| u8::try_from(field).ok()) {
        emitter.emit_fb_load_instance(copy.src);
        emitter.emit_fb_load_param(field);
        emitter.emit_swap();
        emitter.emit_pop();
        emitter.emit_fb_store_param(field);
    }
    emitter.emit_pop();
}

/// How many fields an instance of the block `type_id` dispatches to has: a
/// user-defined block's, or the VM's standard block's.
fn field_count(ctx: &CompileContext, type_id: FbTypeId) -> Option<usize> {
    if let Some(user_fb) = ctx
        .user_fb_types
        .values()
        .find(|info| info.type_id == type_id)
    {
        return Some(user_fb.num_fields);
    }
    ctx.standard_blocks
        .values()
        .map(|standard| standard_fb_layout(*standard))
        .find(|(id, _, _)| *id == type_id)
        .map(|(_, fields, _)| fields)
}
