//! Emits a lowered `Copy`: a whole array or structure assigned from another,
//! and an instance stored in a field of another instance.
//!
//! An aggregate variable's slot holds the byte offset of its data region,
//! and `COPY_REGION` moves the bytes from the region whose offset is on the
//! stack to the region of the destination, sizing both from their array
//! descriptors. The bridge emits a `Copy` only between the places the AST
//! route copies (`compile_aggregate.rs`): whole variables, and fields of the
//! current instance, that this backend's layout keeps as aggregate regions.
//!
//! A structure or array field of another function block instance has no
//! region in this layout. The AST route stores the region offset of the
//! value in the field's slot, as it stores a call's input
//! (`compile_fb_call.rs`): the source's offset, or the offset the source's
//! slot holds when the source is such a field too. So does the bridge for a
//! `Copy` to one (`place::instance_slot`). Neither route compiles a read of
//! what such a field holds, in the block's body, a method or a caller, so no
//! program tells the offset from a copy.
//!
//! So too for a function block instance stored in such a field, `g.inner :=
//! f` or the input of a call, `g(inner := f)`: an instance's slot holds the
//! data offset of its fields, and the field's slot takes it (issue 1553).
//! Neither route calls the instance a field holds, or reads its fields.
//!
//! A `Copy` of a whole instance to another is `instance_copy.rs`'s. Every
//! other `Copy` is left to the AST route.

use ironplc_container::VarIndex;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{Place as LoweredPlace, TypeKind};
use ironplc_lowering::Lowering;

use super::place::{instance_slot, variable_name};
use super::storage_name;
use crate::compile::{CompileContext, DEFAULT_OP_TYPE};
use crate::compile_expr::emit_load_var;
use crate::emit::Emitter;

/// Where an aggregate's data region is found, and the descriptor that sizes
/// it.
struct Region {
    var_index: VarIndex,
    desc_index: u16,
}

/// Whether this backend emits the copy of `src` to `dst`.
pub(super) fn can_emit(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    dst: &LoweredPlace,
    src: &LoweredPlace,
) -> bool {
    let from_region = region(lowering, ctx, src).is_some();
    if region(lowering, ctx, dst).is_some() {
        return from_region;
    }
    instance_slot(lowering, ctx, dst).is_some()
        && (from_region
            || instance_slot(lowering, ctx, src).is_some()
            || instance(lowering, ctx, src).is_some())
}

/// Emits the copy of `src` to `dst`: the source's data offset, then
/// `COPY_REGION` into the destination, or `FB_STORE_PARAM` into the field of
/// an instance.
pub(super) fn emit_copy(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    dst: &LoweredPlace,
    src: &LoweredPlace,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if let Some(field) = instance_slot(lowering, ctx, dst) {
        return field.emit_assign_fitting(emitter, ctx, |emitter, ctx| {
            emit_source(emitter, ctx, lowering, src, span)
        });
    }
    let (Some(dst), Some(src)) = (region(lowering, ctx, dst), region(lowering, ctx, src)) else {
        return Err(no_region(span));
    };
    emit_offset(emitter, &src);
    emitter.emit_copy_region(dst.var_index, dst.desc_index, src.desc_index);
    Ok(())
}

/// Whether this backend copies a structure or an array into `dst`: a whole
/// variable or a field of the current instance that it keeps as a region.
pub(super) fn reaches_region(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    dst: &LoweredPlace,
) -> bool {
    region(lowering, ctx, dst).is_some()
}

/// Emits the copy into `dst` of the structure or array whose region's offset
/// is on the stack, sized by the descriptor `src_desc`: `COPY_REGION`, as
/// the route that reads the AST copies a function's structure result.
pub(super) fn emit_copy_from_stack(
    emitter: &mut Emitter,
    ctx: &CompileContext,
    lowering: &Lowering<'_>,
    dst: &LoweredPlace,
    src_desc: u16,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let Some(dst) = region(lowering, ctx, dst) else {
        return Err(no_region(span));
    };
    emitter.emit_copy_region(dst.var_index, dst.desc_index, src_desc);
    Ok(())
}

/// Pushes the data offset of `src`, the source of a copy into a field of an
/// instance: the offset of its region, the offset its slot holds when it is
/// an instance, or the one the slot of the instance's field it is holds.
pub(super) fn emit_source(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    src: &LoweredPlace,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if let Some(region) = region(lowering, ctx, src) {
        emit_offset(emitter, &region);
        return Ok(());
    }
    if let Some(var_index) = instance(lowering, ctx, src) {
        emit_load_var(emitter, var_index, DEFAULT_OP_TYPE);
        return Ok(());
    }
    instance_slot(lowering, ctx, src)
        .ok_or_else(|| no_region(span))?
        .emit_load(emitter, ctx)
}

/// Pushes the data offset of `region`. An aggregate value has no operation
/// type, so its slot is loaded at the default slot type, as the AST route
/// loads it.
fn emit_offset(emitter: &mut Emitter, region: &Region) {
    emit_load_var(emitter, region.var_index, DEFAULT_OP_TYPE);
}

/// The internal error for a copy `can_emit` should have left to the AST
/// route.
fn no_region(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A lowered copy has no data region",
    ))
}

/// The slot of `place`, when it is a function block instance this backend
/// calls by name (`fb_call.rs`). A function's body sees the program's
/// instances by name, and its own structures may share their names, so
/// `place` must be an instance.
fn instance(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<VarIndex> {
    if !matches!(
        lowering.types().kind(place.ty()),
        Some(TypeKind::FunctionBlock { .. })
    ) {
        return None;
    }
    let name = variable_name(lowering, ctx, place)?;
    ctx.fb_instances.get(&name).map(|info| info.var_index)
}

/// The data region of `place`, when it is a whole variable or a field of the
/// current instance that this backend keeps as a structure or an array. A
/// `REF_TO ARRAY` parameter's slot holds a variable index, not a region, so
/// it has none.
fn region(lowering: &Lowering<'_>, ctx: &CompileContext, place: &LoweredPlace) -> Option<Region> {
    let name = storage_name(lowering, place)?;
    if let Some(info) = ctx.struct_vars.get(&name) {
        return Some(Region {
            var_index: info.var_index,
            desc_index: info.desc_index,
        });
    }
    let info = ctx.array_vars.get(&name).filter(|info| !info.is_ref)?;
    Some(Region {
        var_index: info.var_index,
        desc_index: info.desc_index,
    })
}
