//! Code generation for arrays whose element type is a user-defined structure.
//!
//! Structures occupy a contiguous run of slots, so an array of them is a flat
//! slot array rather than one slot per element. That layout is shared by the
//! two places such an array can appear -- as a field of a structure
//! (`holder.items[i].a`) and as a variable in its own right
//! (`items[i].a`) -- so both the declaration and the access paths live here.
//!
//! Separated from `compile_array.rs`, whose arrays hold one value per slot, to
//! keep module sizes within the 1000-line guideline.

use crate::access::{Access, Field, Named};
use ironplc_ir::{Diagnostic, Label};
use ironplc_ir::{Id, Located, SourceSpan};

use ironplc_container::{SlotIndex, VarIndex};

use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};

use super::compile::CompileContext;
use super::compile_array::{dimensions_of_bounds, ResolvedAccess, StructStringElement};

/// Metadata for a top-level `ARRAY OF <struct>` variable.
///
/// A structure occupies a contiguous run of slots, so an array of them is a
/// flat slot array — the same shape a structure variable has. The variable
/// slot holds the data-region byte offset and `desc_index` is a slot-typed
/// descriptor over the whole region, which lets `arr[i].field` reuse
/// [`ResolvedAccess::StructFieldArrayElement`].
///
/// Kept apart from [`ArrayVarInfo`], whose elements occupy exactly one slot
/// each and are loaded and stored as single values.
#[derive(Clone)]
pub(crate) struct StructArrayVarInfo {
    /// Variable table index holding the data region byte offset.
    pub var_index: VarIndex,
    /// Slot-typed array descriptor covering the whole array.
    pub desc_index: u16,
    /// Data region byte offset where element 0 starts.
    pub data_offset: u32,
    /// The element structure's lowered type.
    pub element: TypeId,
    /// The bounds of each dimension, `(lower, upper)`, in element (not slot)
    /// units.
    pub dimensions: Vec<(i32, i32)>,
    /// Scratch variable for STRING field access. `Some` exactly when
    /// `element_strings` is non-empty.
    pub scratch_var_index: Option<VarIndex>,
    /// The STRING fields of the element structure.
    pub element_strings: Vec<ElementStringField>,
}

/// A STRING field of each element of an array of structures.
///
/// The field repeats once per element, one structure apart, so every copy is
/// reached through a single strided STRING descriptor (ADR-0054). Registered
/// when the variable holding the array is declared, so that both the
/// initialization and each access use the same descriptor.
#[derive(Clone)]
pub(crate) struct ElementStringField {
    /// Slot offset of element 0's copy of the field, from the start of the
    /// variable's data region. Distinct for every such field in a region, so
    /// it identifies the field.
    pub slot_offset: u32,
    /// STRING descriptor over every element's copy of the field.
    pub desc_index: u16,
}

/// The variable whose data region holds an array of structures: either a
/// structure with an array field, or the array itself.
struct ArrayOfStructRegion<'ctx> {
    /// Variable table index holding the region's data offset.
    var_index: VarIndex,
    /// Slot-typed descriptor over the whole region.
    desc_index: u16,
    /// Scratch variable for STRING field access, when the region has any.
    scratch_var_index: Option<VarIndex>,
    /// The STRING fields of array-of-struct elements in the region.
    element_strings: &'ctx [ElementStringField],
}

/// The field `structured` selects from an element of an array, through any
/// number of structure fields: `a[i].f` and `a[i].inner.f`. Gives the access
/// to the element's field, whose record is the array element, and the fields
/// selected after it, outermost first, ending with `structured`'s own; `None`
/// when no array element is under the chain of fields.
pub(crate) fn element_field_chain<S>(structured: &Field<S>) -> Option<(&Field<S>, Vec<&Field<S>>)> {
    let mut fields = vec![structured];
    let mut current = structured;
    loop {
        match current.record.as_ref() {
            Access::Index(_) => {
                fields.reverse();
                return Some((current, fields));
            }
            Access::Field(record) => {
                fields.push(record);
                current = record;
            }
            Access::Named(_) | Access::Deref(_) => return None,
        }
    }
}

/// Resolves a field selected from an element of an array-of-struct, e.g. the
/// `Trigger` in `MyBay.Devices.MeterQRScanner[i].Trigger` or in `Scanners[i].Trigger`,
/// or a field of a structure field of the element, `Scanners[i].Config.Rate`.
///
/// The array itself is either a field of a structure or a variable in its own
/// right; [`locate_array_of_struct`] finds it, and [`struct_array_element_field`]
/// builds the access.
///
/// `field_subscripts` carries the subscripts applied to the selected field, so
/// that `a[i].values[j]` -- an array inside the element structure -- resolves
/// here too. It is empty for a plain `a[i].field`.
pub(crate) fn resolve_struct_array_element_field<'ctx, S: Clone>(
    types: &TypeTable,
    ctx: &'ctx CompileContext,
    structured: &Field<S>,
    field_subscripts: Vec<S>,
) -> Result<ResolvedAccess<'ctx, S>, Diagnostic> {
    let (element, fields) = element_field_chain(structured).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            structured.field.span(),
            "Structure field access is not on an array element",
        ))
    })?;
    let array = locate_array_of_struct(types, ctx, element)?;
    struct_array_element_field(types, array, &fields, field_subscripts)
}

/// The slot offset, from the start of an element of the type `element`, of
/// the field `fields` selects, through structure fields, and its type.
fn element_field<S>(
    types: &TypeTable,
    element: TypeId,
    fields: &[&Field<S>],
) -> Result<(SlotIndex, TypeId), Diagnostic> {
    let mut offset = 0u32;
    let mut ty = element;
    for field in fields {
        let (field_offset, field_type) = crate::compile_struct::field_in(types, ty, field)?;
        offset = offset.checked_add(field_offset.raw()).ok_or_else(|| {
            Diagnostic::not_supported(Label::span(field.field.span(), "Structure too large"))
        })?;
        ty = field_type;
    }
    Ok((SlotIndex::new(offset), ty))
}

/// An array of structures, located from the record of `<array>[i].field`.
struct LocatedArrayOfStruct<'ctx, S> {
    /// The variable whose data region holds the array.
    region: ArrayOfStructRegion<'ctx>,
    /// Slot offset of element 0 within the region.
    base_slot_offset: u32,
    /// The element structure's lowered type.
    element: TypeId,
    /// The bounds of each dimension, in element units.
    dimensions: Vec<(i32, i32)>,
    /// The element subscripts, outermost first.
    subscripts: Vec<S>,
    /// Where the array is named, for diagnostics.
    span: SourceSpan,
}

/// Locates the array of structures that the record of `structured` indexes.
///
/// The array itself is either a field of a structure or a variable in its own
/// right.
fn locate_array_of_struct<'ctx, S: Clone>(
    types: &TypeTable,
    ctx: &'ctx CompileContext,
    structured: &Field<S>,
) -> Result<LocatedArrayOfStruct<'ctx, S>, Diagnostic> {
    // Every caller checks that `record` is an array subscript.
    let Access::Index(array_var) = structured.record.as_ref() else {
        return Err(Diagnostic::internal_error_at(Label::span(
            structured.field.span(),
            "Structure field access is not on an array element",
        )));
    };

    // Collect subscript groups innermost-first, then reverse -- the same
    // walk `resolve_access` performs for plain array chains.
    let mut levels: Vec<&[S]> = Vec::new();
    let mut current = array_var;
    let base = loop {
        levels.push(&current.subscripts);
        match current.array.as_ref() {
            Access::Index(inner) => current = inner,
            Access::Field(base) => break ArrayOfStructBase::Field(base),
            Access::Named(name) => break ArrayOfStructBase::Variable(name),
            other @ Access::Deref(_) => {
                return Err(Diagnostic::todo_with_span(other.span()));
            }
        }
    };
    levels.reverse();
    let subscripts: Vec<S> = levels.into_iter().flatten().cloned().collect();

    match base {
        ArrayOfStructBase::Field(base) => {
            let (root_name, field_slot_offset, field_type) =
                crate::compile_struct::walk_struct_chain(types, ctx, base, 0)?;

            let Some(TypeKind::Array {
                element,
                dimensions,
            }) = types.kind(field_type)
            else {
                return Err(Diagnostic::not_implemented(Label::span(
                    base.field.span(),
                    format!("Field '{}' is not an array type", base.field),
                )));
            };

            let struct_info = ctx.struct_var(&root_name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    structured.field.span(),
                    format!("Variable '{}' is not a structure", root_name),
                ))
            })?;

            Ok(LocatedArrayOfStruct {
                region: ArrayOfStructRegion {
                    var_index: struct_info.var_index,
                    desc_index: struct_info.desc_index,
                    scratch_var_index: struct_info.scratch_var_index,
                    element_strings: &struct_info.element_strings,
                },
                base_slot_offset: field_slot_offset.raw(),
                element: *element,
                dimensions: dimensions.clone(),
                subscripts,
                span: base.field.span(),
            })
        }
        ArrayOfStructBase::Variable(name) => {
            let info = ctx.struct_array_var(name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    name.span(),
                    format!("Variable '{}' is not an array of structures", name),
                ))
            })?;

            Ok(LocatedArrayOfStruct {
                region: ArrayOfStructRegion {
                    var_index: info.var_index,
                    desc_index: info.desc_index,
                    scratch_var_index: info.scratch_var_index,
                    element_strings: &info.element_strings,
                },
                base_slot_offset: 0,
                element: info.element,
                dimensions: info.dimensions.clone(),
                subscripts,
                span: name.span(),
            })
        }
    }
}

/// What an array-of-struct subscript chain bottoms out in.
enum ArrayOfStructBase<'a, S> {
    /// An array field of a structure, as in `holder.items[i]`.
    Field(&'a Field<S>),
    /// A variable that is itself an array of structures, as in `items[i]`.
    Variable(&'a Named),
}

/// Builds the access for `<array-of-struct>[i].field`.
///
/// Structures occupy a contiguous run of slots, so element `k` starts at
/// `base_slot_offset + k * element_slots` and the leaf field sits a further
/// compile-time `leaf_offset` into it:
///
/// ```text
/// slot = base_slot_offset + leaf_offset      (compile-time constant)
///      + flat_index * element_slots          (runtime)
/// ```
///
/// `emit_flat_index` already multiplies each subscript by its dimension
/// stride, so scaling every stride by `element_slots` makes the emitted flat
/// index a slot offset directly. That lets this reuse
/// [`ResolvedAccess::StructFieldArrayElement`] unchanged -- no new opcode and
/// no new emission path. Bounds checks are unaffected because they validate
/// against the unscaled `lower_bound`/`size`.
///
/// `base_slot_offset` is the slot offset of element 0 within the region: the
/// array field's own offset when the array is a struct field, and zero when
/// the variable *is* the array (the region holds nothing else).
///
/// A STRING leaf is the exception: it has no single-slot load or store, so it
/// resolves through [`string_element_field`] instead.
fn struct_array_element_field<'ctx, S>(
    types: &TypeTable,
    array: LocatedArrayOfStruct<'ctx, S>,
    fields: &[&Field<S>],
    field_subscripts: Vec<S>,
) -> Result<ResolvedAccess<'ctx, S>, Diagnostic> {
    let LocatedArrayOfStruct {
        region,
        base_slot_offset,
        element,
        dimensions: array_dims,
        subscripts: element_subscripts,
        span: array_span,
    } = array;
    let array_span = &array_span;
    // The field selected last, which the access reads or writes.
    let Some(field) = fields.last().map(|selected| &selected.field) else {
        return Err(Diagnostic::internal_error_at(Label::span(
            array_span.clone(),
            "An element access selects no field",
        )));
    };
    if !matches!(types.kind(element), Some(TypeKind::Structure { .. })) {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            format!(
                "Cannot select field '{}' -- array elements are not a structure type",
                field
            ),
        )));
    }

    let element_slots = crate::layout::slots(types, element).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            array_span.clone(),
            "Array element type is unsupported",
        ))
    })?;

    let (leaf_slot_offset, leaf_type) = element_field(types, element, fields)?;

    if let (true, Some(TypeKind::String(_))) = (field_subscripts.is_empty(), types.kind(leaf_type))
    {
        let slot_offset = base_slot_offset
            .checked_add(leaf_slot_offset.raw())
            .ok_or_else(|| {
                Diagnostic::not_supported(Label::span(array_span.clone(), "Array too large"))
            })?;
        return string_element_field(&region, slot_offset, &array_dims, element_subscripts, field);
    }

    // Scale strides so the emitted flat index counts slots, not elements.
    let mut dimensions = dimensions_of_bounds(&array_dims);
    for dim in &mut dimensions {
        dim.stride = dim.stride.checked_mul(element_slots).ok_or_else(|| {
            Diagnostic::not_supported(Label::span(array_span.clone(), "Array too large"))
        })?;
    }
    let mut subscripts = element_subscripts;

    // `a[i].values[j]` -- the selected field is itself an array, so its own
    // dimensions extend the index computation. Its elements are single slots
    // sitting side by side inside the element structure, so their strides need
    // no scaling: appending them after the (scaled) element dimensions makes
    // `emit_flat_index` produce `i * element_slots + j` in one pass, and the
    // field's own offset is still the compile-time part.
    let value_type = if field_subscripts.is_empty() {
        leaf_type
    } else {
        let Some(TypeKind::Array {
            element: inner_element,
            dimensions: inner_dims,
        }) = types.kind(leaf_type)
        else {
            return Err(Diagnostic::not_implemented(Label::span(
                field.span(),
                format!("Field '{}' is not an array type", field),
            )));
        };
        dimensions.extend(dimensions_of_bounds(inner_dims));
        subscripts.extend(field_subscripts);
        *inner_element
    };

    // `a[i].names[j]` -- an array of STRING inside the element. Its elements
    // step both by the structure (over `i`) and by the string (over `j`), and
    // a descriptor carries only one stride. Reject rather than emit a wrong
    // address (#1791).
    if matches!(types.kind(value_type), Some(TypeKind::String(_))) {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            format!(
                "STRING array field '{}' of an array-of-struct element is not yet supported",
                field
            ),
        )));
    }

    // A composite value has no single-slot load or store, and the appended
    // strides above assume one slot per innermost element.
    let Some(type_info) = crate::body::slot_type(types, value_type) else {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            format!(
                "Field '{}' of an array-of-struct element is composite (nested struct or array)",
                field
            ),
        )));
    };

    let combined_offset = base_slot_offset
        .checked_add(leaf_slot_offset.raw())
        .ok_or_else(|| {
            Diagnostic::not_supported(Label::span(array_span.clone(), "Array too large"))
        })?;

    Ok(ResolvedAccess::StructFieldArrayElement {
        var_index: region.var_index,
        desc_index: region.desc_index,
        field_slot_offset: SlotIndex::new(combined_offset),
        dimensions,
        subscripts,
        type_info,
    })
}

/// Builds the access for a STRING field of an array-of-struct element,
/// `<array-of-struct>[i].s`.
///
/// The field's copies sit one structure apart, and the descriptor registered
/// for it at declaration carries that stride, so the VM computes
/// `base + flat_index * stride` itself and bounds-checks the element index.
/// The flat index therefore counts elements: the dimensions stay unscaled.
fn string_element_field<'ctx, S>(
    region: &ArrayOfStructRegion<'ctx>,
    slot_offset: u32,
    array_dims: &[(i32, i32)],
    element_subscripts: Vec<S>,
    field: &Id,
) -> Result<ResolvedAccess<'ctx, S>, Diagnostic> {
    let entry = region
        .element_strings
        .iter()
        .find(|f| f.slot_offset == slot_offset);
    // Declaration registers every STRING field the resolver can reach, so a
    // miss is a compiler defect rather than an unsupported program.
    let (Some(entry), Some(scratch_var_index)) = (entry, region.scratch_var_index) else {
        return Err(Diagnostic::internal_error_at(Label::span(
            field.span(),
            format!(
                "STRING field '{}' of an array-of-struct element has no registered descriptor",
                field
            ),
        )));
    };
    let field_byte_offset = slot_offset.checked_mul(8).ok_or_else(|| {
        Diagnostic::not_supported(Label::span(field.span(), "Data region overflow"))
    })?;

    Ok(ResolvedAccess::StructFieldStringArrayElement(
        StructStringElement {
            var_index: region.var_index,
            scratch_var_index,
            string_desc_index: entry.desc_index,
            field_byte_offset,
            dimensions: dimensions_of_bounds(array_dims),
            subscripts: element_subscripts,
        },
    ))
}
