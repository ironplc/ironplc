//! The storage of the instances that the fields of a function block hold.
//!
//! An instance's slot holds the data region offset of its fields, one slot
//! each (`layout/storage.rs`). A field of a function block type holds an
//! instance too, `ob` in `VAR_OUTPUT ob : F; END_VAR`. The layout reserves
//! the fields of that instance after those of the instance that holds it,
//! and the setup block stores their offset in the field's slot, so the
//! field's slot holds what an instance's slot holds. An instance that a field
//! of a held instance holds is laid out the same way, to any depth: the
//! analyzer rejects a block that holds itself (P4005).

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::TypeEnvironment;
use ironplc_dsl::common::VarDecl;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use ironplc_container::FbTypeId;

use crate::compile::{CompileContext, FbFieldIndex};
use crate::compile_call::standard_fb_layout;
use crate::data_region::DataOffset;
use crate::emit::Emitter;

/// A field of a user-defined function block that holds an instance of a
/// block.
#[derive(Clone, Debug)]
pub(crate) struct HeldField {
    /// The field's name, in lowercase.
    pub(crate) name: String,
    /// The field's index.
    pub(crate) index: FbFieldIndex,
    /// The name of the block, in uppercase, as `user_fb_types` and
    /// `standard_blocks` know it.
    pub(crate) block: String,
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

/// The name of the block whose instance the variable `decl` declares, in
/// uppercase; `None` when `decl` declares no instance.
pub(crate) fn held_block(types: &TypeEnvironment, decl: &VarDecl) -> Option<String> {
    let type_id = decl.type_id?;
    let attributes = types.get_by_id(type_id)?;
    if !matches!(
        attributes.representation,
        SemanticType::FunctionBlock { .. }
    ) {
        return None;
    }
    types
        .name_of(type_id)
        .map(|name| name.to_string().to_uppercase())
}

/// The layout of the instances of the block named `block`, in uppercase.
pub(crate) fn layout<'c>(ctx: &'c CompileContext, block: &str) -> Option<BlockLayout<'c>> {
    if let Some(user_fb) = ctx.user_fb_type(block) {
        return Some(BlockLayout {
            type_id: user_fb.type_id,
            fields: user_fb.num_fields,
            held: &user_fb.held_fields,
        });
    }
    let (type_id, fields, _) = standard_fb_layout(*ctx.standard_blocks.get(block)?);
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

/// Reserves the fields of each instance that the fields of an instance of
/// the block named `block` hold, when that instance's own fields are at
/// `data_offset`, and of each instance their fields hold in turn. A field
/// whose block has no layout, as an instance of it has none, holds none.
pub(crate) fn reserve(
    ctx: &mut CompileContext,
    block: &str,
    data_offset: DataOffset,
    span: &SourceSpan,
) -> Result<Vec<HeldInstance>, Diagnostic> {
    let mut instances = Vec::new();
    reserve_into(ctx, block, data_offset, span, &mut instances)?;
    Ok(instances)
}

fn reserve_into(
    ctx: &mut CompileContext,
    block: &str,
    holder: DataOffset,
    span: &SourceSpan,
    instances: &mut Vec<HeldInstance>,
) -> Result<(), Diagnostic> {
    let held = layout(ctx, block)
        .map(|layout| layout.held.to_vec())
        .unwrap_or_default();
    for field in held {
        let Some(fields) = layout(ctx, &field.block).map(|layout| layout.fields) else {
            continue;
        };
        let data_offset = crate::data_region::reserve(ctx, fields as u32 * 8, span)?;
        instances.push(HeldInstance {
            holder,
            index: field.index,
            data_offset,
        });
        reserve_into(ctx, &field.block, data_offset, span, instances)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ironplc_dsl::core::SourceSpan;

    use super::{reserve, HeldField, HeldInstance};
    use crate::compile::{CompileContext, FbFieldIndex, UserFbTypeInfo};
    use ironplc_container::{FbTypeId, FunctionId};

    fn block(
        ctx: &mut CompileContext,
        name: &str,
        type_id: FbTypeId,
        fields: usize,
        held: &[(FbFieldIndex, &str)],
    ) {
        ctx.user_fb_types.insert(
            name.to_string(),
            UserFbTypeInfo {
                type_id,
                num_fields: fields,
                field_indices: HashMap::new(),
                function_id: FunctionId::new(2),
                var_offset: 0,
                field_types: HashMap::new(),
                held_fields: held
                    .iter()
                    .map(|(index, block)| HeldField {
                        name: format!("f{index}"),
                        index: *index,
                        block: block.to_string(),
                    })
                    .collect(),
                methods: HashMap::new(),
            },
        );
    }

    #[test]
    fn reserve_when_block_holds_instances_to_depth_two_then_each_reserved_after_its_holder() {
        let mut ctx = CompileContext::new();
        block(&mut ctx, "INNER", FbTypeId::new(0x1000), 2, &[]);
        block(
            &mut ctx,
            "MIDDLE",
            FbTypeId::new(0x1001),
            3,
            &[(1, "INNER")],
        );
        block(
            &mut ctx,
            "OUTER",
            FbTypeId::new(0x1002),
            4,
            &[(0, "MIDDLE"), (3, "INNER")],
        );
        ctx.data_region_offset = 100;

        let instances = reserve(&mut ctx, "OUTER", 64, &SourceSpan::default()).unwrap();

        assert_eq!(
            instances,
            [
                HeldInstance {
                    holder: 64,
                    index: 0,
                    data_offset: 100,
                },
                HeldInstance {
                    holder: 100,
                    index: 1,
                    data_offset: 124,
                },
                HeldInstance {
                    holder: 64,
                    index: 3,
                    data_offset: 140,
                },
            ]
        );
        assert_eq!(ctx.data_region_offset, 156);
    }

    #[test]
    fn reserve_when_held_block_has_no_layout_then_holds_none() {
        let mut ctx = CompileContext::new();
        block(
            &mut ctx,
            "OUTER",
            FbTypeId::new(0x1000),
            2,
            &[(0, "MISSING")],
        );

        let instances = reserve(&mut ctx, "OUTER", 0, &SourceSpan::default()).unwrap();

        assert!(instances.is_empty());
        assert_eq!(ctx.data_region_offset, 0);
    }
}
