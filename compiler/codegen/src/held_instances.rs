//! The storage of the instances that the fields of a function block hold.
//!
//! An instance's slot holds the data region offset of its fields, one slot
//! each. A field of a function block type holds an
//! instance too, `ob` in `VAR_OUTPUT ob : F; END_VAR`. The layout reserves
//! the fields of that instance after those of the instance that holds it,
//! and the setup block stores their offset in the field's slot, so the
//! field's slot holds what an instance's slot holds. An instance that a field
//! of a held instance holds is laid out the same way, to any depth: the
//! analyzer rejects a block that holds itself (P4005).

use ironplc_container::FbTypeId;
use ironplc_ir::lowered::{FieldIdx, PouId};

use crate::compile::{CompileContext, FbFieldIndex};
use crate::compile_call::standard_fb_layout;
use crate::emit::Emitter;

/// A field of a user-defined function block that holds an instance of a
/// block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeldField {
    /// The field, of the block's lowered type.
    pub(crate) field: FieldIdx,
    /// The field's index.
    pub(crate) index: FbFieldIndex,
    /// The lowered POU of the block, as `user_fb_types` and
    /// `standard_blocks` know it.
    pub(crate) block: PouId,
}

/// What the layout keeps for each instance of a block.
pub(crate) struct BlockLayout<'c> {
    /// The block's type ID, which `FB_CALL` dispatches on.
    pub(crate) type_id: FbTypeId,
    /// How many fields each instance has, one slot each.
    pub(crate) fields: usize,
    /// The fields that hold an instance. A standard block's hold none.
    pub(crate) held: &'c [HeldField],
}

/// A byte offset into the data region: where the fields of an instance
/// start.
pub(crate) type DataOffset = u32;

/// An instance that a field holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeldInstance {
    /// The data region offset of the fields of the instance whose field
    /// holds it.
    holder: DataOffset,
    /// The index of the field that holds it.
    index: FbFieldIndex,
    /// The data region offset of its own fields.
    data_offset: DataOffset,
}

impl HeldInstance {
    /// The instance whose fields are at `data_offset`, which the field
    /// `index` of the instance whose fields are at `holder` holds.
    pub(crate) fn new(holder: DataOffset, index: FbFieldIndex, data_offset: DataOffset) -> Self {
        Self {
            holder,
            index,
            data_offset,
        }
    }
}

/// The layout of the instances of the block whose lowered POU is `block`.
pub(crate) fn layout<'c>(ctx: &'c CompileContext, block: PouId) -> Option<BlockLayout<'c>> {
    if let Some(user_fb) = ctx.user_fb_type(block) {
        return Some(BlockLayout {
            type_id: user_fb.type_id,
            fields: user_fb.num_fields,
            held: &user_fb.held_fields,
        });
    }
    let (type_id, fields, _) = standard_fb_layout(*ctx.standard_blocks.get(&block)?);
    Some(BlockLayout {
        type_id,
        fields,
        held: &[],
    })
}

/// The layout of the instances of the block whose type ID is `type_id`.
pub(crate) fn layout_of_type<'c>(
    ctx: &'c CompileContext,
    type_id: FbTypeId,
) -> Option<BlockLayout<'c>> {
    if let Some(user_fb) = ctx
        .user_fb_types
        .values()
        .find(|user_fb| user_fb.type_id == type_id)
    {
        return Some(BlockLayout {
            type_id,
            fields: user_fb.num_fields,
            held: &user_fb.held_fields,
        });
    }
    ctx.standard_blocks
        .values()
        .map(|standard| standard_fb_layout(*standard))
        .find(|(id, _, _)| *id == type_id)
        .map(|(_, fields, _)| BlockLayout {
            type_id,
            fields,
            held: &[],
        })
}

/// Emits the stores that give each field that holds one of `instances` the
/// offset of that instance's fields: the holder's offset pushed as the
/// instance `FB_STORE_PARAM` stores in, the held instance's offset stored in
/// the field, and the holder's offset popped.
pub(crate) fn emit_offsets(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    instances: &[HeldInstance],
) {
    for instance in instances {
        let holder = ctx.add_i32_constant(instance.holder as i32);
        emitter.emit_load_const_i32(holder);
        let data_offset = ctx.add_i32_constant(instance.data_offset as i32);
        emitter.emit_load_const_i32(data_offset);
        emitter.emit_fb_store_param(instance.index);
        emitter.emit_pop();
    }
}
