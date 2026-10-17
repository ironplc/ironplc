//! Emits a lowered `Copy`: a whole array or structure assigned from another.
//!
//! An aggregate variable's slot holds the byte offset of its data region,
//! and `COPY_REGION` moves the bytes from the region whose offset is on the
//! stack to the region of the destination, sizing both from their array
//! descriptors. The bridge emits a `Copy` only between the places the AST
//! route copies (`compile_aggregate.rs`): whole variables, and fields of the
//! current instance, that this backend's layout keeps as aggregate regions.
//! Every other `Copy` is left to the AST route.

use ironplc_container::VarIndex;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::Place as LoweredPlace;
use ironplc_lowering::Lowering;

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
    region(lowering, ctx, dst).is_some() && region(lowering, ctx, src).is_some()
}

/// Emits the copy of `src` to `dst`: the source's data offset, then
/// `COPY_REGION` into the destination.
pub(super) fn emit_copy(
    emitter: &mut Emitter,
    ctx: &CompileContext,
    lowering: &Lowering<'_>,
    dst: &LoweredPlace,
    src: &LoweredPlace,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let (Some(dst), Some(src)) = (region(lowering, ctx, dst), region(lowering, ctx, src)) else {
        return Err(Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "A lowered copy has no data region",
        )));
    };
    // An aggregate value has no operation type, so its slot is loaded at
    // the default slot type, as the AST route loads it.
    emit_load_var(emitter, src.var_index, DEFAULT_OP_TYPE);
    emitter.emit_copy_region(dst.var_index, dst.desc_index, src.desc_index);
    Ok(())
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
