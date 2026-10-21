//! What the layout check compares of an entry that holds an analyzer type,
//! which the layout built from the lowered declarations does not have: a
//! structure's and an array of structures' (`lowered.rs`).

use ironplc_container::VarIndex;
use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};

use super::descriptors::Slots;
use super::region::{slots, string_shape};
use crate::compile_array_struct::StructArrayVarInfo;
use crate::compile_struct::StructVarInfo;

/// Where a structure variable is, and how each of its fields is reached.
#[derive(Debug, PartialEq)]
pub(super) struct StructEntry {
    var_index: VarIndex,
    data_offset: u32,
    total_slots: u32,
    /// Each field in declaration order: its lower-case name, its slot from
    /// the start of the structure, and a string's capacity.
    fields: Vec<(String, u32, Option<u16>)>,
    scratch_var_index: Option<VarIndex>,
    slots: SlotsEntry,
}

/// Where an array of structures is, and how its elements' string fields are
/// reached.
#[derive(Debug, PartialEq)]
pub(super) struct StructArrayEntry {
    var_index: VarIndex,
    data_offset: u32,
    /// The bounds of each dimension.
    dimensions: Vec<(i32, i32)>,
    /// How many slots each element takes.
    element_slots: Option<u32>,
    scratch_var_index: Option<VarIndex>,
    slots: SlotsEntry,
}

/// How a structure, or an array of structures, is reached as a flat array
/// of slots ([`Slots`]), with each field that is an array of strings in
/// name order.
#[derive(Debug, PartialEq)]
pub(super) struct SlotsEntry {
    desc_index: u16,
    string_arrays: Vec<(String, (u16, u32, u16))>,
    element_strings: Vec<(u32, u16)>,
}

impl StructEntry {
    /// The entry of a structure of the type `ty` the layout built from the
    /// lowered declarations records: in the slot `var_index`, with its run
    /// at `data_offset`, its scratch slot at `scratch_var_index`, and
    /// reached through `slots_of`.
    pub(super) fn lowered(
        types: &TypeTable,
        ty: TypeId,
        var_index: VarIndex,
        data_offset: u32,
        scratch_var_index: Option<VarIndex>,
        slots_of: Slots,
    ) -> Option<Self> {
        let mut fields = Vec::new();
        let mut slot = 0_u32;
        for field in types.fields(ty)? {
            let capacity = match types.kind(field.ty)? {
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
            };
            fields.push((field.name.to_string().to_lowercase(), slot, capacity));
            slot = slot.checked_add(slots(types, field.ty)?)?;
        }
        Some(Self {
            var_index,
            data_offset,
            total_slots: slots(types, ty)?,
            fields,
            scratch_var_index,
            slots: slots_of.into(),
        })
    }
}

impl StructArrayEntry {
    /// The entry of an array of structures of the type `ty` the layout
    /// built from the lowered declarations records, as
    /// [`StructEntry::lowered`] gives a structure's.
    pub(super) fn lowered(
        types: &TypeTable,
        ty: TypeId,
        var_index: VarIndex,
        data_offset: u32,
        scratch_var_index: Option<VarIndex>,
        slots_of: Slots,
    ) -> Option<Self> {
        let TypeKind::Array {
            element,
            dimensions,
        } = types.kind(ty)?
        else {
            return None;
        };
        Some(Self {
            var_index,
            data_offset,
            dimensions: dimensions.clone(),
            element_slots: slots(types, *element),
            scratch_var_index,
            slots: slots_of.into(),
        })
    }
}

impl From<Slots> for SlotsEntry {
    fn from(mut slots: Slots) -> Self {
        slots.string_arrays.sort();
        Self {
            desc_index: slots.desc_index,
            string_arrays: slots.string_arrays,
            element_strings: slots.element_strings,
        }
    }
}

impl From<&StructVarInfo> for StructEntry {
    fn from(info: &StructVarInfo) -> Self {
        Self {
            var_index: info.var_index,
            data_offset: info.data_offset,
            total_slots: info.total_slots.raw(),
            fields: info
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        field.slot_offset.raw(),
                        field.string_max_length,
                    )
                })
                .collect(),
            scratch_var_index: info.scratch_var_index,
            slots: Slots {
                desc_index: info.desc_index,
                string_arrays: info
                    .string_array_descs
                    .iter()
                    .map(|(name, desc)| (name.clone(), *desc))
                    .collect(),
                element_strings: element_strings(&info.element_strings),
            }
            .into(),
        }
    }
}

impl From<&StructArrayVarInfo> for StructArrayEntry {
    fn from(info: &StructArrayVarInfo) -> Self {
        Self {
            var_index: info.var_index,
            data_offset: info.data_offset,
            dimensions: info
                .dimensions
                .iter()
                .map(|dimension| (dimension.lower, dimension.upper))
                .collect(),
            element_slots: info.element_type.slot_count().ok(),
            scratch_var_index: info.scratch_var_index,
            slots: Slots {
                desc_index: info.desc_index,
                string_arrays: Vec::new(),
                element_strings: element_strings(&info.element_strings),
            }
            .into(),
        }
    }
}

/// The slot and the descriptor of each of `fields`.
fn element_strings(fields: &[crate::compile_array_struct::ElementStringField]) -> Vec<(u32, u16)> {
    fields
        .iter()
        .map(|field| (field.slot_offset, field.desc_index))
        .collect()
}
