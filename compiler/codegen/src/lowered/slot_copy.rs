//! Emits a lowered `Copy` between variables of a function, function block or
//! method body that this backend's layout keeps no storage for beyond their
//! slots.
//!
//! The scope a function, function block or method body is compiled against
//! registers no data region for its own structures and arrays, and calls no
//! instance it declares (`scope.rs`). Neither route compiles a read inside
//! such a variable there, nor a call of such an instance, so what its slot
//! holds is never followed. The route that reads the AST stores the source's
//! slot in the destination's for `lp := lq`, and so does the bridge for a
//! `Copy` of one: no program tells that from a copy of the values
//! (REQ-LOW-codegen-165).
//!
//! A program's own variables are left to `aggregate.rs` and
//! `instance_copy.rs`, which copy their values, and so is a field of a
//! block's instance that holds an instance, which has storage of its own
//! (`held_instances.rs`).

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{Owner, Place as LoweredPlace, Projection, TypeId, TypeKind, TypeTable};
use ironplc_lowering::PartialProgram;

use super::aggregate;
use super::place::{resolve, LoweredSubscript};
use crate::compile::CompileContext;
use crate::compile_place::Place;
use crate::emit::Emitter;

/// The copy of one slot to another.
pub(super) struct SlotCopy<'l> {
    dst: Place<LoweredSubscript<'l>>,
    src: Place<LoweredSubscript<'l>>,
}

/// The copy of `src` to `dst` when `dst` is the whole of a variable of the
/// body being compiled, a function, a function block or a method, of a
/// structure, array or function block type, that this backend keeps no data
/// region for, and `src` the whole of a variable the body names. Such an
/// instance is none this backend calls, as it calls only a program's
/// (`fb_call.rs`). A global source keeps its region, whose offset its slot
/// holds. `None` for any other `Copy`.
pub(super) fn slot_copy<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    dst: &'l LoweredPlace,
    src: &'l LoweredPlace,
) -> Option<SlotCopy<'l>> {
    // A `Copy`'s places have one shape.
    let types = lowering.types();
    let aggregate_type = matches!(
        types.kind(dst.ty()),
        Some(TypeKind::Structure { .. } | TypeKind::Array { .. } | TypeKind::FunctionBlock { .. })
    );
    if !aggregate_type {
        return None;
    }
    if !body_variable(lowering, dst) || aggregate::has_region(lowering, ctx, dst) {
        return None;
    }
    // A field that holds an instance has storage of its own, which a caller
    // copies from (`g(ob => f)`): another slot stored in it would make it an
    // alias. A `Copy` to one is `instance_copy.rs`'s.
    if super::instance_copy::holds_instance(lowering, ctx, dst) {
        return None;
    }
    let slot = |place| {
        let slot = resolve(lowering, ctx, place, &SourceSpan::default()).ok()?;
        slot.variable().map(|_| slot)
    };
    Some(SlotCopy {
        dst: slot(dst)?,
        src: slot(src)?,
    })
}

/// Emits `copy`: the source's slot loaded and stored in the destination's.
pub(super) fn emit_slot_copy(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    copy: &SlotCopy<'_>,
) -> Result<(), Diagnostic> {
    copy.dst.emit_assign_fitting(emitter, ctx, |emitter, ctx| {
        copy.src.emit_load(emitter, ctx)
    })
}

/// Whether `place` is the whole of a variable of a function, function block
/// or method body: a local or parameter of a function or method, a
/// `VAR_TEMP` of a function block, or a field of the function block instance
/// a body works on.
fn body_variable(lowering: &PartialProgram, place: &LoweredPlace) -> bool {
    let types = lowering.types();
    let Some(root) = lowering.variables().get(place.root()) else {
        return false;
    };
    match place.path() {
        [] => match root.owner() {
            Owner::Pou(pou) => !types.iter().any(
                |(_, entry)| matches!(entry.kind, TypeKind::Program { body, .. } if body == pou),
            ),
            Owner::Global => false,
        },
        [Projection::Deref, Projection::Field(_)] => is_block_reference(types, root.ty()),
        _ => false,
    }
}

/// Whether `ty` is a reference to a function block, the type of the
/// instance parameter of a function block's body or method.
pub(super) fn is_block_reference(types: &TypeTable, ty: TypeId) -> bool {
    types.reference(ty).is_some_and(|(target, _)| {
        matches!(types.kind(target), Some(TypeKind::FunctionBlock { .. }))
    })
}
