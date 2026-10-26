//! What the layout built from the lowered declarations records of a
//! structure and of an array of structures (`declarations.rs`): where it is, how
//! its fields and elements are reached, and the capacity of each string it
//! holds, which sizes the temporary buffers.

use ironplc_container::{ArrayDescriptor, VarIndex};
use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};

use super::descriptors::Slots;
use super::region::string_shape;
use crate::compile_array_struct::{ElementStringField, StructArrayVarInfo};
use crate::compile_struct::StructVarInfo;

/// The entry of a structure of the type `ty`: in the slot `var_index`, with
/// its run at `data_offset`, its scratch slot at `scratch_var_index`, and
/// reached through `slots`.
pub(super) fn structure(
    ty: TypeId,
    var_index: VarIndex,
    data_offset: u32,
    scratch_var_index: Option<VarIndex>,
    slots: Slots,
) -> StructVarInfo {
    StructVarInfo {
        var_index,
        data_offset,
        desc_index: slots.desc_index,
        ty,
        scratch_var_index,
        string_array_descs: slots.string_arrays.into_iter().collect(),
        element_strings: element_strings(slots.element_strings),
    }
}

/// The entry of an array of structures of the type `ty`, as [`structure`]
/// gives a structure's; `None` when `ty` is not an array.
pub(super) fn struct_array(
    types: &TypeTable,
    ty: TypeId,
    var_index: VarIndex,
    data_offset: u32,
    scratch_var_index: Option<VarIndex>,
    slots: Slots,
) -> Option<StructArrayVarInfo> {
    let TypeKind::Array {
        element,
        dimensions,
    } = types.kind(ty)?
    else {
        return None;
    };
    Some(StructArrayVarInfo {
        var_index,
        desc_index: slots.desc_index,
        data_offset,
        element: *element,
        dimensions: dimensions.clone(),
        scratch_var_index,
        element_strings: element_strings(slots.element_strings),
    })
}

/// The capacity of each string a field of the structure `info` holds, as
/// the temporary buffers are sized for: a string field's, an array of
/// strings field's, and a string field's of the elements of an array of
/// structures it holds, which `descriptors` describe.
pub(super) fn struct_capacities<'a>(
    types: &'a TypeTable,
    info: &'a StructVarInfo,
    descriptors: &'a [ArrayDescriptor],
) -> impl Iterator<Item = u16> + 'a {
    types
        .fields(info.ty)
        .unwrap_or_default()
        .iter()
        .filter_map(|field| match types.kind(field.ty)? {
            TypeKind::String(shape) => Some(string_shape(*shape)?.0),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        })
        .chain(
            info.string_array_descs
                .values()
                .map(|(_, _, capacity)| *capacity),
        )
        .chain(element_capacities(&info.element_strings, descriptors))
}

/// The capacity of each string field of the elements of the array of
/// structures `info`, as [`struct_capacities`] gives a structure's.
pub(super) fn struct_array_capacities<'a>(
    info: &'a StructArrayVarInfo,
    descriptors: &'a [ArrayDescriptor],
) -> impl Iterator<Item = u16> + 'a {
    element_capacities(&info.element_strings, descriptors)
}

/// The capacity of the strings of each of `fields`, which `descriptors`
/// describe.
fn element_capacities<'a>(
    fields: &'a [ElementStringField],
    descriptors: &'a [ArrayDescriptor],
) -> impl Iterator<Item = u16> + 'a {
    fields.iter().filter_map(|field| {
        descriptors
            .get(usize::from(field.desc_index))
            .map(|descriptor| descriptor.element_extra)
    })
}

/// Each string field of the elements of an array of structures, from its
/// slot in the first element and its strided descriptor.
fn element_strings(fields: Vec<(u32, u16)>) -> Vec<ElementStringField> {
    fields
        .into_iter()
        .map(|(slot_offset, desc_index)| ElementStringField {
            slot_offset,
            desc_index,
        })
        .collect()
}
