//! The array descriptors the lowered declarations add to the container
//! (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! The container keeps each shape of array once, and numbers the shapes in
//! the order the layout first adds them: an array's
//! own; a structure's, as a flat array of slots, then one for each field
//! that is an array of strings, then one for each string field of the
//! elements of each array of structures it holds, strided by the element;
//! an array of structures' the same way; and the ones a reference reaches
//! its target through (`compile_reference.rs`).

use ironplc_container::{ArrayDescriptor, CharWidth, FieldType, VarIndex};
use ironplc_ir::lowered::{Field, FieldIdx, StringShape, TypeId, TypeKind, TypeTable};
use ironplc_ir::SourceSpan;

use super::region::{elements, slots, string_shape};
use crate::compile::{OpWidth, Signedness, VarTypeInfo};
use crate::compile_array::{compute_dimensions, var_type_info_to_type_byte, ArrayVarInfo};
use crate::compile_reference::StringRefInfo;

/// A reference's 64-bit slot, and an element of an array of references.
const REFERENCE_TYPE: VarTypeInfo = VarTypeInfo {
    op_width: OpWidth::W64,
    signedness: Signedness::Unsigned,
    storage_bits: 64,
};

/// What a variable is reached through, of the descriptors it adds.
#[derive(Default)]
pub(super) struct Reached {
    /// How it is reached as an array, when it is one or refers to one.
    pub(super) array: Option<ArrayVarInfo>,
    /// The flat array of slots it is, when it is a structure or an array of
    /// structures.
    pub(super) slots: Option<Slots>,
    /// The strings it refers to, when it is a reference to a string or to
    /// an array of strings.
    pub(super) string_ref: Option<StringRefInfo>,
    /// The descriptor that sizes the whole of what it refers to, when it is
    /// a reference to an array or a structure.
    pub(super) region_ref: Option<u16>,
}

/// A structure, or an array of structures, reached as a flat array of
/// slots.
pub(super) struct Slots {
    /// The descriptor of the flat array.
    pub(super) desc_index: u16,
    /// Each field of a structure that is an array of strings: its
    /// descriptor, how many strings it holds, and how many code units each
    /// holds.
    pub(super) string_arrays: Vec<(FieldIdx, (u16, u32, u16))>,
    /// Each string field of the elements of an array of structures it
    /// holds, or is: the slot of the field in the first element, from the
    /// start of the flat array, and its strided descriptor.
    pub(super) element_strings: Vec<(u32, u16)>,
}

/// The array descriptors added so far, by index.
pub(super) struct Descriptors<'a> {
    types: &'a TypeTable,
    added: Vec<ArrayDescriptor>,
}

impl<'a> Descriptors<'a> {
    /// No descriptors yet, for the types `types`.
    pub(super) fn new(types: &'a TypeTable) -> Self {
        Self {
            types,
            added: Vec::new(),
        }
    }

    /// The descriptors added, by index.
    pub(super) fn into_added(self) -> Vec<ArrayDescriptor> {
        self.added
    }

    /// Adds the descriptors of a global's or the program's variable of the
    /// type `ty`, whose slot is `var_index` and whose run of the data region
    /// starts at `data_offset`, and returns what it is reached through.
    pub(super) fn variable(
        &mut self,
        ty: TypeId,
        var_index: VarIndex,
        data_offset: Option<u32>,
    ) -> Option<Reached> {
        let types = self.types;
        match types.kind(ty)? {
            TypeKind::Structure { .. } => {
                return Some(Reached {
                    slots: Some(self.structure(ty)?),
                    ..Reached::default()
                })
            }
            TypeKind::Array { element, .. } => {
                if matches!(types.kind(*element)?, TypeKind::Structure { .. }) {
                    let total = elements(types, ty)?.checked_mul(slots(types, *element)?)?;
                    let desc_index = self.add(slot_array(total));
                    let mut element_strings = Vec::new();
                    self.element_strings(ty, 0, &mut element_strings)?;
                    return Some(Reached {
                        slots: Some(Slots {
                            desc_index,
                            string_arrays: Vec::new(),
                            element_strings,
                        }),
                        ..Reached::default()
                    });
                }
                return Some(Reached {
                    array: Some(self.array(ty, var_index, data_offset?)?),
                    ..Reached::default()
                });
            }
            TypeKind::Reference { target, .. } => return self.reference(*target, var_index),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. } => {}
        }
        Some(Reached::default())
    }

    /// Adds the descriptors of a structure of the type `ty`, and returns
    /// what it is reached through.
    pub(super) fn structure(&mut self, ty: TypeId) -> Option<Slots> {
        let types = self.types;
        let desc_index = self.add(slot_array(slots(types, ty)?));
        let fields = types.fields(ty)?;
        let mut string_arrays = Vec::new();
        for (position, field) in fields.iter().enumerate() {
            let TypeKind::Array { element, .. } = types.kind(field.ty)? else {
                continue;
            };
            if let TypeKind::String(shape) = types.kind(*element)? {
                let (max_length, char_width) = string_shape(*shape)?;
                let total = elements(types, field.ty)?;
                let desc = self.add(string_array(char_width, total, max_length));
                let idx = types.field_idx(ty, position)?;
                string_arrays.push((idx, (desc, total, max_length)));
            }
        }
        let mut element_strings = Vec::new();
        self.held_element_strings(fields, 0, &mut element_strings)?;
        Some(Slots {
            desc_index,
            string_arrays,
            element_strings,
        })
    }

    /// Adds the descriptors a reference to a `target` reaches it through,
    /// for the reference whose slot is `var_index`, and returns what the
    /// reference is reached through.
    pub(super) fn reference(&mut self, target: TypeId, var_index: VarIndex) -> Option<Reached> {
        let types = self.types;
        match types.kind(target)? {
            TypeKind::Array { element, .. } => {
                let total = elements(types, target)?;
                let (dimensions, _) =
                    compute_dimensions(&bounds(types, target)?, &no_span()).ok()?;
                let element_type = if matches!(types.kind(*element)?, TypeKind::Reference { .. }) {
                    REFERENCE_TYPE
                } else {
                    // An element that is not operated on as a value is a
                    // 32-bit unsigned element.
                    crate::body::slot_type(types, *element).unwrap_or(VarTypeInfo {
                        op_width: OpWidth::W32,
                        signedness: Signedness::Unsigned,
                        storage_bits: 32,
                    })
                };
                let desc_index = self.add(ArrayDescriptor::new(
                    var_type_info_to_type_byte(&element_type),
                    total,
                    0,
                ));
                // An array of strings, not of references to them, is sized
                // by its string descriptor.
                let string_ref = if let TypeKind::String(shape) = types.kind(*element)? {
                    Some(self.string_ref(*shape, total)?)
                } else {
                    None
                };
                let region_ref = string_ref
                    .as_ref()
                    .map_or(desc_index, |string| string.desc_index);
                let array = ArrayVarInfo {
                    var_index,
                    desc_index,
                    data_offset: 0,
                    element_var_type_info: element_type,
                    total_elements: total,
                    dimensions,
                    is_string_element: false,
                    string_max_len: 0,
                    string_char_width: CharWidth::Narrow,
                    is_ref: true,
                };
                Some(Reached {
                    array: Some(array),
                    string_ref,
                    region_ref: Some(region_ref),
                    ..Reached::default()
                })
            }
            TypeKind::Structure { .. } => Some(Reached {
                // A structure whose size is not known has no descriptor.
                region_ref: slots(types, target).map(|total| self.add(slot_array(total))),
                ..Reached::default()
            }),
            TypeKind::String(shape) => Some(Reached {
                string_ref: Some(self.string_ref(*shape, 1)?),
                ..Reached::default()
            }),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => Some(Reached::default()),
        }
    }

    /// Adds the descriptor of the `total` strings of the shape `shape` a
    /// reference refers to, and returns how it reaches them.
    fn string_ref(&mut self, shape: StringShape, total: u32) -> Option<StringRefInfo> {
        let (max_length, char_width) = string_shape(shape)?;
        Some(StringRefInfo {
            desc_index: self.add(string_array(char_width, total, max_length)),
            char_width,
        })
    }

    /// Adds the descriptor of an array of the type `ty`, whose elements are
    /// not structures, and returns how the array whose slot is `var_index`
    /// and whose run starts at `data_offset` is reached.
    fn array(&mut self, ty: TypeId, var_index: VarIndex, data_offset: u32) -> Option<ArrayVarInfo> {
        let types = self.types;
        let TypeKind::Array { element, .. } = types.kind(ty)? else {
            return None;
        };
        let (dimensions, total) = compute_dimensions(&bounds(types, ty)?, &no_span()).ok()?;
        let string = if let TypeKind::String(shape) = types.kind(*element)? {
            Some(string_shape(*shape)?)
        } else {
            None
        };
        let (element_type, descriptor) = if let Some((max_length, char_width)) = string {
            (
                // String elements are operated through the string
                // instructions, which take no type.
                VarTypeInfo {
                    op_width: OpWidth::W32,
                    signedness: Signedness::Unsigned,
                    storage_bits: 0,
                },
                string_array(char_width, total, max_length),
            )
        } else {
            let element_type = if matches!(types.kind(*element)?, TypeKind::Reference { .. }) {
                REFERENCE_TYPE
            } else {
                crate::body::slot_type(types, *element)?
            };
            let byte = var_type_info_to_type_byte(&element_type);
            (element_type, ArrayDescriptor::new(byte, total, 0))
        };
        let desc_index = self.add(descriptor);
        let (string_max_len, string_char_width) = string.unwrap_or((0, CharWidth::Narrow));
        Some(ArrayVarInfo {
            var_index,
            desc_index,
            data_offset,
            element_var_type_info: element_type,
            total_elements: total,
            dimensions,
            is_string_element: string.is_some(),
            string_max_len,
            string_char_width,
            is_ref: false,
        })
    }

    /// Adds a strided descriptor for each string field of the elements of
    /// each array of structures that the structure fields `fields`, from the
    /// slot `base`, hold, to any depth of structures, and records each in
    /// `out`.
    fn held_element_strings(
        &mut self,
        fields: &[Field],
        base: u32,
        out: &mut Vec<(u32, u16)>,
    ) -> Option<()> {
        let types = self.types;
        let mut slot = base;
        for field in fields {
            match types.kind(field.ty)? {
                TypeKind::Structure { fields } => self.held_element_strings(fields, slot, out)?,
                TypeKind::Array { .. } => self.element_strings(field.ty, slot, out)?,
                TypeKind::Bool
                | TypeKind::Integer { .. }
                | TypeKind::BitString { .. }
                | TypeKind::Real { .. }
                | TypeKind::Temporal { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Subrange { .. }
                | TypeKind::String(_)
                | TypeKind::FunctionBlock { .. }
                | TypeKind::Program { .. }
                | TypeKind::Reference { .. } => {}
            }
            slot = slot.checked_add(slots(types, field.ty)?)?;
        }
        Some(())
    }

    /// Adds a strided descriptor for each string field of the elements of
    /// the array of the type `ty`, from the slot `base`, when its elements
    /// are structures, and records each in `out`. A string held deeper in an
    /// element has none.
    fn element_strings(&mut self, ty: TypeId, base: u32, out: &mut Vec<(u32, u16)>) -> Option<()> {
        let types = self.types;
        let TypeKind::Array { element, .. } = types.kind(ty)? else {
            return None;
        };
        let TypeKind::Structure { fields } = types.kind(*element)? else {
            return Some(());
        };
        let stride = slots(types, *element)?.checked_mul(8)?;
        let total = elements(types, ty)?;
        let mut slot = base;
        for field in fields {
            if let TypeKind::String(shape) = types.kind(field.ty)? {
                let (max_length, char_width) = string_shape(*shape)?;
                let desc = self.add(ArrayDescriptor {
                    element_stride: stride,
                    ..string_array(char_width, total, max_length)
                });
                out.push((slot, desc));
            }
            slot = slot.checked_add(slots(types, field.ty)?)?;
        }
        Some(())
    }

    /// Adds `descriptor`, unless one equal to it was added before, and
    /// returns its index (`ContainerBuilder::add_array_descriptor`).
    fn add(&mut self, descriptor: ArrayDescriptor) -> u16 {
        if let Some(index) = self.added.iter().position(|added| *added == descriptor) {
            return index as u16;
        }
        self.added.push(descriptor);
        (self.added.len() - 1) as u16
    }
}

/// The descriptor of a structure, or an array of structures, as a flat
/// array of `total` slots.
fn slot_array(total: u32) -> ArrayDescriptor {
    ArrayDescriptor::new(FieldType::Slot as u8, total, 0)
}

/// The descriptor of `total` strings of `max_length` code units
/// `char_width` wide each, back to back.
fn string_array(char_width: CharWidth, total: u32, max_length: u16) -> ArrayDescriptor {
    ArrayDescriptor::new(FieldType::string(char_width) as u8, total, max_length)
}

/// The bounds of each dimension of the array of the type `ty`.
fn bounds(types: &TypeTable, ty: TypeId) -> Option<Vec<(i32, i32)>> {
    let TypeKind::Array { dimensions, .. } = types.kind(ty)? else {
        return None;
    };
    Some(dimensions.clone())
}

/// The span a limit of an array would be reported at: none, since an
/// array over a limit fails the limits check before the layout is built
/// ([`super::limits`]).
fn no_span() -> SourceSpan {
    SourceSpan::default()
}
