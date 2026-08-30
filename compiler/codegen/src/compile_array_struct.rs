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

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, SymbolicVariableKind};

use ironplc_analyzer::intermediate_type::{ArrayDimension, IntermediateType};
use ironplc_container::{ContainerBuilder, SlotIndex, VarIndex};

use ironplc_analyzer::TypeEnvironment;
use ironplc_dsl::common::SpecificationKind;

use super::compile::CompileContext;
use super::compile_array::{dimensions_from_intermediate, ResolvedAccess};

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
    /// The element structure type.
    pub element_type: IntermediateType,
    /// Array bounds, in element (not slot) units.
    pub dimensions: Vec<ArrayDimension>,
}

/// Resolves a field selected from an element of an array-of-struct, e.g. the
/// `Trigger` in `MyBay.Devices.MeterQRScanner[i].Trigger` or in `Scanners[i].Trigger`.
///
/// The array itself is either a field of a structure or a variable in its own
/// right; [`struct_array_element_field`] handles both once the base is known.
///
/// `field_subscripts` carries the subscripts applied to the selected field, so
/// that `a[i].values[j]` -- an array inside the element structure -- resolves
/// here too. It is empty for a plain `a[i].field`.
pub(crate) fn resolve_struct_array_element_field<'ctx, 'ast>(
    ctx: &'ctx CompileContext,
    structured: &'ast ironplc_dsl::textual::StructuredVariable,
    field_subscripts: Vec<&'ast Expr>,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
    let SymbolicVariableKind::Array(array_var) = structured.record.as_ref() else {
        return Err(Diagnostic::todo_with_span(structured.span()));
    };

    // Collect subscript groups innermost-first, then reverse -- the same
    // walk `resolve_access` performs for plain array chains.
    let mut levels: Vec<&[Expr]> = Vec::new();
    let mut current = array_var;
    let base = loop {
        levels.push(&current.subscripts);
        match current.subscripted_variable.as_ref() {
            SymbolicVariableKind::Array(inner) => current = inner,
            SymbolicVariableKind::Structured(base) => break ArrayOfStructBase::Field(base),
            SymbolicVariableKind::Named(named) => break ArrayOfStructBase::Variable(&named.name),
            other => {
                return Err(Diagnostic::todo_with_span(other.span()));
            }
        }
    };
    levels.reverse();
    let subscripts: Vec<&Expr> = levels.into_iter().flatten().collect();

    match base {
        ArrayOfStructBase::Field(base) => {
            let (root_name, field_slot_offset, field_type) =
                crate::compile_struct::walk_struct_chain(ctx, &base.record, &base.field, 0)?;

            let IntermediateType::Array {
                element_type,
                dimensions: array_dims,
            } = &field_type
            else {
                return Err(Diagnostic::not_implemented(Label::span(
                    base.field.span(),
                    format!("Field '{}' is not an array type", base.field),
                )));
            };

            let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    structured.span(),
                    format!("Variable '{}' is not a structure", root_name),
                ))
            })?;

            struct_array_element_field(
                struct_info.var_index,
                struct_info.desc_index,
                field_slot_offset.raw(),
                element_type,
                array_dims,
                &structured.field,
                subscripts,
                field_subscripts,
                &base.field.span(),
            )
        }
        ArrayOfStructBase::Variable(name) => {
            let info = ctx.struct_array_vars.get(name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    name.span(),
                    format!("Variable '{}' is not an array of structures", name),
                ))
            })?;

            struct_array_element_field(
                info.var_index,
                info.desc_index,
                0,
                &info.element_type,
                &info.dimensions,
                &structured.field,
                subscripts,
                field_subscripts,
                &name.span(),
            )
        }
    }
}

/// What an array-of-struct subscript chain bottoms out in.
enum ArrayOfStructBase<'ast> {
    /// An array field of a structure, as in `holder.items[i]`.
    Field(&'ast ironplc_dsl::textual::StructuredVariable),
    /// A variable that is itself an array of structures, as in `items[i]`.
    Variable(&'ast Id),
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
/// `base_slot_offset` is the slot offset of element 0 within the region
/// `desc_index` addresses: the array field's own offset when the array is a
/// struct field, and zero when the variable *is* the array (the region holds
/// nothing else).
#[allow(clippy::too_many_arguments)]
fn struct_array_element_field<'ctx, 'ast>(
    var_index: VarIndex,
    desc_index: u16,
    base_slot_offset: u32,
    element_type: &IntermediateType,
    array_dims: &[ArrayDimension],
    field: &Id,
    element_subscripts: Vec<&'ast Expr>,
    field_subscripts: Vec<&'ast Expr>,
    array_span: &SourceSpan,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
    let IntermediateType::Structure {
        fields: element_fields,
    } = element_type
    else {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            format!(
                "Cannot select field '{}' -- array elements are not a structure type",
                field
            ),
        )));
    };

    let element_slots = element_type.slot_count().map_err(|_| {
        Diagnostic::not_implemented(Label::span(
            array_span.clone(),
            "Array element type is unsupported",
        ))
    })?;

    let (leaf_slot_offset, leaf_type) =
        crate::compile_struct::find_field_in_type(element_fields, field, &field.span())?;

    // Scale strides so the emitted flat index counts slots, not elements.
    let mut dimensions = dimensions_from_intermediate(array_dims);
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
        let IntermediateType::Array {
            element_type: inner_element_type,
            dimensions: inner_dims,
        } = &leaf_type
        else {
            return Err(Diagnostic::not_implemented(Label::span(
                field.span(),
                format!("Field '{}' is not an array type", field),
            )));
        };
        dimensions.extend(dimensions_from_intermediate(inner_dims));
        subscripts.extend(field_subscripts);
        inner_element_type.as_ref().clone()
    };

    // A STRING value needs an element stride of `element_slots * 8`, which the
    // 8-byte ArrayDescriptor cannot express (the VM derives a string array's
    // stride from `element_extra`). Tracked separately; reject explicitly
    // rather than emitting a wrong address.
    if matches!(value_type, IntermediateType::String { .. }) {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            format!(
                "STRING field '{}' of an array-of-struct element is not yet supported",
                field
            ),
        )));
    }

    // A composite value has no single-slot load or store, and the appended
    // strides above assume one slot per innermost element.
    let element_op_type =
        crate::compile_struct::resolve_field_op_type(&value_type).ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                field.span(),
                format!(
                    "Field '{}' of an array-of-struct element is composite (nested struct or array)",
                    field
                ),
            ))
        })?;

    let combined_offset = base_slot_offset
        .checked_add(leaf_slot_offset.raw())
        .ok_or_else(|| {
            Diagnostic::not_supported(Label::span(array_span.clone(), "Array too large"))
        })?;

    Ok(ResolvedAccess::StructFieldArrayElement {
        var_index,
        desc_index,
        field_slot_offset: SlotIndex::new(combined_offset),
        dimensions,
        subscripts,
        element_op_type,
        element_type: value_type,
    })
}

/// Detects an array declaration whose element type is a user-defined
/// structure, returning the element type, the debug type name to record for
/// the variable, and the array bounds.
///
/// Returns `None` for every other array — including `ARRAY[..] OF REF_TO
/// <struct>`, whose elements are one-slot references and so belong on the
/// ordinary array path.
#[allow(clippy::type_complexity)]
pub(crate) fn struct_array_declaration(
    types: &TypeEnvironment,
    spec: &SpecificationKind<ironplc_dsl::common::ArraySubranges>,
    span: &ironplc_dsl::core::SourceSpan,
) -> Result<Option<(IntermediateType, String, Vec<ArrayDimension>)>, Diagnostic> {
    match spec {
        SpecificationKind::Inline(subranges) => {
            if subranges.ref_to.is_some() {
                return Ok(None);
            }
            let element_name = subranges.type_name.to_type_name();
            let Some(element_type) = types.resolve_struct_type(&element_name) else {
                return Ok(None);
            };
            // Reuse the inline bounds parsing rather than repeating it.
            let array_spec = super::compile_array::array_spec_from_inline(subranges, span)?;
            let dimensions = array_spec
                .dimensions
                .iter()
                .map(|&(lower, upper)| ArrayDimension { lower, upper })
                .collect();
            Ok(Some((
                element_type.clone(),
                format!("ARRAY OF {}", element_name.to_string().to_uppercase()),
                dimensions,
            )))
        }
        SpecificationKind::Named(type_name) => {
            let Some(IntermediateType::Array {
                element_type,
                dimensions,
            }) = types.resolve_array_type(type_name)
            else {
                return Ok(None);
            };
            if !matches!(element_type.as_ref(), IntermediateType::Structure { .. }) {
                return Ok(None);
            }
            // Named array specifications are expanded to inline ones before
            // codegen, so this arm is defensive. It cannot name the element:
            // `IntermediateType::Structure` is structural and carries no
            // declared name, so the debug entry falls back to the array type's.
            Ok(Some((
                element_type.as_ref().clone(),
                type_name.to_string().to_uppercase(),
                dimensions.clone(),
            )))
        }
    }
}

/// Registers a top-level `ARRAY OF <struct>` variable.
///
/// Allocates one contiguous data region run of `total_elements *
/// element_slots` slots and a slot-typed descriptor over it, mirroring how
/// [`crate::compile_struct::allocate_struct_variable`] lays out a single
/// structure. Element fields are not initialized here: the data region starts
/// zeroed, which matches what an array-of-struct *field* of a structure gets
/// today.
///
/// Returns the debug type tag and type name, like
/// [`register_array_variable`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn register_struct_array_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    element_type: &IntermediateType,
    debug_type_name: &str,
    dimensions: &[ArrayDimension],
    span: &SourceSpan,
) -> Result<(u8, String), Diagnostic> {
    let element_slots = element_type.slot_count().map_err(|_| {
        Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Array element structure is unsupported",
        ))
    })?;

    // Reject an element type the field walker cannot describe (for example a
    // duplicate field name) at declaration time rather than at first access.
    crate::compile_struct::build_struct_fields(struct_fields(element_type, span)?, span)?;

    let mut total_elements: u32 = 1;
    for dim in dimensions {
        let size = (dim.upper as i64 - dim.lower as i64 + 1).max(0) as u32;
        total_elements = total_elements.checked_mul(size).ok_or_else(|| {
            Diagnostic::not_supported(Label::span(span.clone(), "Array too large"))
        })?;
    }

    let total_slots = total_elements
        .checked_mul(element_slots)
        .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Array too large")))?;

    // The flat index is computed in slots, so the slot count -- not the
    // element count -- is what must stay within i32 arithmetic.
    if total_slots > super::compile::MAX_DATA_REGION_SLOTS {
        return Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Array exceeds maximum 32768 slots",
        )));
    }

    let data_offset = ctx.data_region_offset;
    let total_bytes = total_slots.checked_mul(8).ok_or_else(|| {
        Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow"))
    })?;
    ctx.data_region_offset = ctx
        .data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| {
            Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow"))
        })?;

    // The offset is stored in the variable slot via LOAD_CONST_I32.
    if ctx.data_region_offset > i32::MAX as u32 {
        return Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Data region exceeds 2 GiB limit",
        )));
    }

    let desc_index =
        builder.add_array_descriptor(ironplc_container::FieldType::Slot as u8, total_slots, 0);

    ctx.struct_array_vars.insert(
        id.clone(),
        StructArrayVarInfo {
            var_index,
            desc_index,
            data_offset,
            element_type: element_type.clone(),
            dimensions: dimensions.to_vec(),
        },
    );

    Ok((
        ironplc_container::debug_section::iec_type_tag::OTHER,
        debug_type_name.to_string(),
    ))
}

/// Returns the field list of a structure `IntermediateType`.
fn struct_fields<'a>(
    element_type: &'a IntermediateType,
    span: &SourceSpan,
) -> Result<&'a [ironplc_analyzer::intermediate_type::IntermediateStructField], Diagnostic> {
    match element_type {
        IntermediateType::Structure { fields } => Ok(fields),
        _ => Err(Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Array element type is not a structure",
        ))),
    }
}
