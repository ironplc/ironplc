//! Emits a lowered `Copy` of a whole function block instance (`f := g`,
//! `f := g.ob`, `g.ob := f`).
//!
//! An instance's slot holds the data region offset of its fields, so storing
//! one slot in the other would make the destination an alias of the source.
//! A `Copy` gives the destination the source's values in storage of its own
//! (REQ-LOW-codegen-165), so the backend copies the instance field by field,
//! with `FB_LOAD_PARAM` and `FB_STORE_PARAM`: every field of the block, its inputs, outputs and
//! variables, and for a standard block the fields the VM's block keeps for
//! itself, such as a timer's start time. A field is one slot, so each is
//! copied whole, except a field that holds an instance: its slot holds the
//! offset of that instance's fields (`held_instances.rs`), so the instance
//! it holds is copied in turn, into the one the destination's field holds.
//!
//! An instance is copied when this backend reaches it: as a variable, or as
//! the instance a field of one it reaches holds, to any depth (`g.ob`,
//! `h.og.ob`). Only an instance it reaches as a variable is called
//! (`fb_call.rs`).
//! In a function block's body or method, the instance that a field of the
//! instance it works on holds is reached too (`ob := loc`): the call copies
//! that instance's fields into the body's slots and back (`compile_fn.rs`),
//! so the field's slot holds the offset of the instance it holds.
//!
//! A `Copy` is taken for one of instances only when its places are function
//! block instances.

use crate::access::{Named, Stored};
use ironplc_container::{FbTypeId, VarIndex};
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{Place as LoweredPlace, Projection, TypeKind};

use super::place::field_path;
use super::slot_copy::is_block_reference;
use crate::compile::{CompileContext, FbFieldIndex};
use crate::emit::Emitter;
use crate::held_instances::{layout, layout_of_type};

/// The copy of one instance to another of its block.
pub(super) struct InstanceCopy {
    dst: Reached,
    src: Reached,
    /// What copies each field, in order.
    steps: Vec<Step>,
}

/// An instance this backend reaches: the instance whose slot is
/// `var_index`, or the instance that its field `fields[0]` holds, then the
/// one that field `fields[1]` of that instance holds, and so on.
struct Reached {
    var_index: VarIndex,
    fields: Vec<FbFieldIndex>,
}

/// One step of a copy, taken with the destination's instance on the stack.
enum Step {
    /// Copies the field, one slot, from the source's instance.
    Field(FbFieldIndex),
    /// Pushes the instance the field holds, which the steps up to the
    /// matching `Leave` copy into from the instance the source's field
    /// holds.
    Enter(FbFieldIndex),
    /// Pops the instance the last `Enter` pushed.
    Leave,
}

/// The copy of `src` to `dst` when both are instances this backend reaches,
/// of one block. `None` for any other `Copy`.
pub(super) fn instance_copy(
    lowering: &Program,
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
    let (dst, block) = reached(lowering, ctx, dst)?;
    let (src, src_block) = reached(lowering, ctx, src)?;
    if block != src_block {
        return None;
    }
    let mut steps = Vec::new();
    plan(ctx, block, &mut steps)?;
    Some(InstanceCopy { dst, src, steps })
}

/// Emits `copy`: the destination pushed once, then for each field the
/// source's field loaded and stored in the destination's, or the instance
/// the field holds copied in turn, then the destination popped.
pub(super) fn emit_instance_copy(emitter: &mut Emitter, copy: &InstanceCopy) {
    emit_load(emitter, &copy.dst, &[]);
    let mut entered = Vec::new();
    for step in &copy.steps {
        match step {
            Step::Field(field) => {
                emit_load(emitter, &copy.src, &entered);
                emitter.emit_fb_load_param(*field);
                emitter.emit_swap();
                emitter.emit_pop();
                emitter.emit_fb_store_param(*field);
            }
            Step::Enter(field) => {
                emitter.emit_fb_load_param(*field);
                entered.push(*field);
            }
            Step::Leave => {
                emitter.emit_pop();
                entered.pop();
            }
        }
    }
    emitter.emit_pop();
}

/// The instance `place` is, when this backend reaches it, and the type ID of
/// its block.
fn reached(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<(Reached, FbTypeId)> {
    let (name, path) = field_path(lowering, ctx, place)?;
    let (var_index, mut block) = match ctx.fb_instance(&name) {
        Some(instance) => (instance.var_index, instance.type_id),
        None => held_by_current(lowering, ctx, place, &name)?,
    };
    let mut fields = Vec::new();
    for field in path {
        let held = layout_of_type(ctx, block)?
            .held
            .iter()
            .find(|held| held.field == field)?;
        block = layout(ctx, held.block)?.type_id;
        fields.push(held.index);
    }
    Some((Reached { var_index, fields }, block))
}

/// Whether `place` is the whole of a field of the instance a function
/// block's body or method works on, and the field holds an instance.
pub(super) fn holds_instance(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> bool {
    matches!(place.path(), [Projection::Deref, Projection::Field(_)])
        && field_path(lowering, ctx, place)
            .is_some_and(|(name, _)| held_by_current(lowering, ctx, place, &name).is_some())
}

/// The slot of the field `name` of the instance a function block's body or
/// method works on, and the type ID of the block of the instance the field
/// holds, when `place` is in that field and the field holds an instance.
fn held_by_current(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
    name: &Named,
) -> Option<(VarIndex, FbTypeId)> {
    if !matches!(place.path(), [Projection::Deref, Projection::Field(_), ..]) {
        return None;
    }
    let root = lowering.variables().get(place.root())?;
    if !is_block_reference(lowering.types(), root.ty()) {
        return None;
    }
    // The current body is the block's, or one of its methods'.
    let current = super::current_pou(ctx)?;
    let block = lowering.pous().method_of(current).unwrap_or(current);
    let Stored::Field(_, field) = name.stored else {
        return None;
    };
    let held = layout(ctx, block)?
        .held
        .iter()
        .find(|held| held.field == field)?;
    Some((ctx.var_index(name).ok()?, layout(ctx, held.block)?.type_id))
}

/// Appends to `steps` the steps that copy each field of an instance of the
/// block whose type ID is `block`. `None` when the layout keeps no instance
/// of it. A block has at most `MAX_USER_FB_FIELDS` fields
/// (REQ-LOW-codegen-191), so each field's index fits the byte that
/// `FB_LOAD_PARAM` and `FB_STORE_PARAM` name it by.
fn plan(ctx: &CompileContext, block: FbTypeId, steps: &mut Vec<Step>) -> Option<()> {
    let instances = layout_of_type(ctx, block)?;
    for field in 0..instances.fields {
        let field = FbFieldIndex::try_from(field).ok()?;
        match instances.held.iter().find(|held| held.index == field) {
            Some(held) => {
                steps.push(Step::Enter(field));
                plan(ctx, layout(ctx, held.block)?.type_id, steps)?;
                steps.push(Step::Leave);
            }
            None => steps.push(Step::Field(field)),
        }
    }
    Some(())
}

/// Emits the load of the instance `reached` is, then of the instance that
/// each of its fields `entered` holds in turn.
fn emit_load(emitter: &mut Emitter, reached: &Reached, entered: &[FbFieldIndex]) {
    emitter.emit_fb_load_instance(reached.var_index);
    for field in reached.fields.iter().chain(entered) {
        emitter.emit_fb_load_param(*field);
        emitter.emit_swap();
        emitter.emit_pop();
    }
}
