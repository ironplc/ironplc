//! Emits a lowered `Copy`: a whole array or structure assigned from another,
//! and an instance stored in a field of another instance.
//!
//! An aggregate variable's slot holds the byte offset of its data region,
//! and `COPY_REGION` moves the bytes from the region whose offset is on the
//! stack to the region of the destination, sizing both from their array
//! descriptors. The bridge emits a `Copy` between the places the AST route
//! copies (`compile_aggregate.rs`): whole variables, and fields of the
//! current instance, that this backend's layout keeps as aggregate regions.
//! It also copies a whole array of structures, which the AST route does not:
//! it stores the source's slot, so the target becomes an alias of the source
//! (REQ-LOW-codegen-165).
//!
//! A `Copy` to what a reference variable refers to, `p^ := z`, is the
//! exception: the AST route stores `z`'s offset in the slot of the variable
//! `p` refers to, which makes that variable an alias of `z`. The bridge
//! copies `z` into it with `COPY_REGION_DEREF`, through the reference, sized
//! by the descriptor `p`'s declaration registers (REQ-LOW-codegen-193).
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
//! A `Copy` of a whole instance to another is `instance_copy.rs`'s, into a
//! field that holds one too. An instance it does not copy, one it does not
//! reach or of a block the layout keeps no instance of, is stored in such a
//! field as the AST route stores it: the field's slot takes the instance's
//! slot, which holds the data offset of its fields. Every other `Copy` is
//! left to the AST route.

use ironplc_container::VarIndex;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{Place as LoweredPlace, TypeKind};
use ironplc_lowering::PartialProgram;

use super::place::{instance_slot, referenced_by, variable_name};
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

/// The aggregate the reference in `ref_slot` refers to, and the descriptor
/// that sizes it.
struct Referenced {
    ref_slot: VarIndex,
    desc_index: u16,
}

/// Whether this backend emits the copy of `src` to `dst`.
pub(super) fn can_emit(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    dst: &LoweredPlace,
    src: &LoweredPlace,
) -> bool {
    let from_region = region(lowering, ctx, src).is_some();
    if region(lowering, ctx, dst).is_some() || referenced(lowering, ctx, dst).is_some() {
        return from_region;
    }
    instance_slot(lowering, ctx, dst).is_some()
        && (from_region
            || instance_slot(lowering, ctx, src).is_some()
            || instance(lowering, ctx, src).is_some())
}

/// Emits the copy of `src` to `dst`: the source's data offset, then
/// `COPY_REGION` into the destination, `COPY_REGION_DEREF` into what a
/// reference refers to, or `FB_STORE_PARAM` into the field of an instance.
pub(super) fn emit_copy(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &PartialProgram,
    dst: &LoweredPlace,
    src: &LoweredPlace,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if let Some(field) = instance_slot(lowering, ctx, dst) {
        return field.emit_assign_fitting(emitter, ctx, |emitter, ctx| {
            emit_source(emitter, ctx, lowering, src, span)
        });
    }
    if let Some(dst) = referenced(lowering, ctx, dst) {
        let src = region(lowering, ctx, src).ok_or_else(|| no_region(span))?;
        emit_offset(emitter, &src);
        emitter.emit_copy_region_deref(dst.ref_slot, dst.desc_index, src.desc_index);
        return Ok(());
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
    lowering: &PartialProgram,
    ctx: &CompileContext,
    dst: &LoweredPlace,
) -> bool {
    region(lowering, ctx, dst).is_some()
}

/// Whether this backend passes `src` as the structure, array or function
/// block input of a function or method: a place it keeps as a region, or an
/// instance it calls by name, whose slot holds the data offset of its fields.
pub(super) fn reaches_argument(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    src: &LoweredPlace,
) -> bool {
    region(lowering, ctx, src).is_some()
        || instance(lowering, ctx, src).is_some()
        || instance_slot(lowering, ctx, src).is_some()
}

/// Emits the copy into `dst` of the structure or array whose region's offset
/// is on the stack, sized by the descriptor `src_desc`: `COPY_REGION`, as
/// the route that reads the AST copies a function's structure result.
pub(super) fn emit_copy_from_stack(
    emitter: &mut Emitter,
    ctx: &CompileContext,
    lowering: &PartialProgram,
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
///
/// It is also what the route that reads the AST passes for a structure or
/// array input of a function or method, which so takes the caller's region
/// rather than a copy of it. Neither route compiles a read of such an input
/// in the callee's body, so no program tells the two apart.
pub(super) fn emit_source(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &PartialProgram,
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
pub(super) fn no_region(span: &SourceSpan) -> Diagnostic {
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
    lowering: &PartialProgram,
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
    ctx.fb_instance(&name).map(|info| info.var_index)
}

/// Whether this backend keeps a data region for `place`.
pub(super) fn has_region(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> bool {
    region(lowering, ctx, place).is_some()
}

/// The data region of `place`, when it is a whole variable or a field of the
/// current instance that this backend keeps as a structure, an array, or an
/// array of structures. A `REF_TO ARRAY` parameter's slot holds a variable
/// index, not a region, so it has none.
fn region(lowering: &PartialProgram, ctx: &CompileContext, place: &LoweredPlace) -> Option<Region> {
    let name = storage_name(lowering, ctx, place)?;
    if let Some(info) = ctx.struct_var(&name) {
        return Some(Region {
            var_index: info.var_index,
            desc_index: info.desc_index,
        });
    }
    // An array of structures: its descriptor covers the whole array, string
    // fields included, so `COPY_REGION` copies it by value, where the route
    // that reads the AST stores the source's slot and so makes the target
    // an alias of the source.
    if let Some(info) = ctx.struct_array_var(&name) {
        return Some(Region {
            var_index: info.var_index,
            desc_index: info.desc_index,
        });
    }
    let info = ctx.array_var(&name).filter(|info| !info.is_ref)?;
    Some(Region {
        var_index: info.var_index,
        desc_index: info.desc_index,
    })
}

/// What `place` is, when it is the array or structure a reference variable
/// refers to, `p^`, whose declaration registered the descriptor that sizes
/// it.
fn referenced(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Referenced> {
    let name = referenced_by(lowering, ctx, place)?;
    Some(Referenced {
        ref_slot: ctx.var_index(&name).ok()?,
        desc_index: ctx.region_ref(&name)?,
    })
}
