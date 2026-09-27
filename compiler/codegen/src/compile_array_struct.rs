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

use ironplc_analyzer::intermediate_type::{
    ArrayDimension, IntermediateStructField, IntermediateType,
};
use ironplc_container::{CharWidth, ContainerBuilder, FieldType, SlotIndex, VarIndex};

use ironplc_analyzer::TypeEnvironment;
use ironplc_dsl::common::SpecificationKind;

use super::compile::CompileContext;
use super::compile_array::{dimensions_from_intermediate, ResolvedAccess, StructStringElement};

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

/// Resolves a field selected from an element of an array-of-struct, e.g. the
/// `Trigger` in `MyBay.Devices.MeterQRScanner[i].Trigger` or in `Scanners[i].Trigger`.
///
/// The array itself is either a field of a structure or a variable in its own
/// right; [`locate_array_of_struct`] finds it, and [`struct_array_element_field`]
/// builds the access.
///
/// `field_subscripts` carries the subscripts applied to the selected field, so
/// that `a[i].values[j]` -- an array inside the element structure -- resolves
/// here too. It is empty for a plain `a[i].field`.
pub(crate) fn resolve_struct_array_element_field<'ctx, 'ast>(
    ctx: &'ctx CompileContext,
    structured: &'ast ironplc_dsl::textual::StructuredVariable,
    field_subscripts: Vec<&'ast Expr>,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
    let array = locate_array_of_struct(ctx, structured)?;
    struct_array_element_field(
        array.region,
        array.base_slot_offset,
        &array.element_type,
        &array.dimensions,
        &structured.field,
        array.subscripts,
        field_subscripts,
        &array.span,
    )
}

/// Returns the declared type of a field selected from an element of an
/// array-of-struct, the `LastCode` in `MyBay.Devices.MeterQRScanner[i].LastCode`.
///
/// Used where only the type matters, such as working out the encoding and
/// capacity of a STRING field.
pub(crate) fn struct_array_element_field_type(
    ctx: &CompileContext,
    structured: &ironplc_dsl::textual::StructuredVariable,
) -> Result<IntermediateType, Diagnostic> {
    let array = locate_array_of_struct(ctx, structured)?;
    let IntermediateType::Structure { fields } = &array.element_type else {
        return Err(Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            format!(
                "Cannot select field '{}' -- array elements are not a structure type",
                structured.field
            ),
        )));
    };
    let (_, field_type) = crate::compile_struct::find_field_in_type(
        fields,
        &structured.field,
        &structured.field.span(),
    )?;
    Ok(field_type)
}

/// An array of structures, located from the record of `<array>[i].field`.
struct LocatedArrayOfStruct<'ctx, 'ast> {
    /// The variable whose data region holds the array.
    region: ArrayOfStructRegion<'ctx>,
    /// Slot offset of element 0 within the region.
    base_slot_offset: u32,
    /// The element structure type.
    element_type: IntermediateType,
    /// Array bounds, in element units.
    dimensions: Vec<ArrayDimension>,
    /// The element subscripts, outermost first.
    subscripts: Vec<&'ast Expr>,
    /// Where the array is named, for diagnostics.
    span: SourceSpan,
}

/// Locates the array of structures that the record of `structured` indexes.
///
/// The array itself is either a field of a structure or a variable in its own
/// right.
fn locate_array_of_struct<'ctx, 'ast>(
    ctx: &'ctx CompileContext,
    structured: &'ast ironplc_dsl::textual::StructuredVariable,
) -> Result<LocatedArrayOfStruct<'ctx, 'ast>, Diagnostic> {
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
                dimensions,
            } = field_type
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

            Ok(LocatedArrayOfStruct {
                region: ArrayOfStructRegion {
                    var_index: struct_info.var_index,
                    desc_index: struct_info.desc_index,
                    scratch_var_index: struct_info.scratch_var_index,
                    element_strings: &struct_info.element_strings,
                },
                base_slot_offset: field_slot_offset.raw(),
                element_type: *element_type,
                dimensions,
                subscripts,
                span: base.field.span(),
            })
        }
        ArrayOfStructBase::Variable(name) => {
            let info = ctx.struct_array_vars.get(name).ok_or_else(|| {
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
                element_type: info.element_type.clone(),
                dimensions: info.dimensions.clone(),
                subscripts,
                span: name.span(),
            })
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
/// `base_slot_offset` is the slot offset of element 0 within the region: the
/// array field's own offset when the array is a struct field, and zero when
/// the variable *is* the array (the region holds nothing else).
///
/// A STRING leaf is the exception: it has no single-slot load or store, so it
/// resolves through [`string_element_field`] instead.
#[allow(clippy::too_many_arguments)]
fn struct_array_element_field<'ctx, 'ast>(
    region: ArrayOfStructRegion<'ctx>,
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

    if let (true, IntermediateType::String { char_width, .. }) =
        (field_subscripts.is_empty(), &leaf_type)
    {
        let slot_offset = base_slot_offset
            .checked_add(leaf_slot_offset.raw())
            .ok_or_else(|| {
                Diagnostic::not_supported(Label::span(array_span.clone(), "Array too large"))
            })?;
        return string_element_field(
            &region,
            slot_offset,
            *char_width,
            array_dims,
            element_subscripts,
            field,
        );
    }

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

    // `a[i].names[j]` -- an array of STRING inside the element. Its elements
    // step both by the structure (over `i`) and by the string (over `j`), and
    // a descriptor carries only one stride. Reject rather than emit a wrong
    // address (#1791).
    if matches!(value_type, IntermediateType::String { .. }) {
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
        var_index: region.var_index,
        desc_index: region.desc_index,
        field_slot_offset: SlotIndex::new(combined_offset),
        dimensions,
        subscripts,
        element_op_type,
        element_type: value_type,
    })
}

/// Builds the access for a STRING field of an array-of-struct element,
/// `<array-of-struct>[i].s`.
///
/// The field's copies sit one structure apart, and the descriptor registered
/// for it at declaration carries that stride, so the VM computes
/// `base + flat_index * stride` itself and bounds-checks the element index.
/// The flat index therefore counts elements: the dimensions stay unscaled.
fn string_element_field<'ctx, 'ast>(
    region: &ArrayOfStructRegion<'ctx>,
    slot_offset: u32,
    char_width: CharWidth,
    array_dims: &[ArrayDimension],
    element_subscripts: Vec<&'ast Expr>,
    field: &Id,
) -> Result<ResolvedAccess<'ctx, 'ast>, Diagnostic> {
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
            char_width,
            dimensions: dimensions_from_intermediate(array_dims),
            subscripts: element_subscripts,
        },
    ))
}

/// Registers the STRING fields of the elements of every array of structures
/// inside a structure, including inside its nested structure fields.
///
/// `base_slot` is the structure's slot offset within the variable's data
/// region. Entries are appended to `out` in declaration order.
pub(crate) fn register_struct_element_strings(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    fields: &[IntermediateStructField],
    base_slot: u32,
    span: &SourceSpan,
    out: &mut Vec<ElementStringField>,
) -> Result<(), Diagnostic> {
    let (field_infos, _) = crate::compile_struct::build_struct_fields(fields, span)?;
    for f in &field_infos {
        let slot = base_slot + f.slot_offset.raw();
        match &f.field_type {
            IntermediateType::Structure { fields } => {
                register_struct_element_strings(ctx, builder, fields, slot, span, out)?;
            }
            IntermediateType::Array {
                element_type,
                dimensions,
            } => {
                register_array_element_strings(
                    ctx,
                    builder,
                    element_type,
                    dimensions,
                    slot,
                    span,
                    out,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Registers the direct STRING fields of an array's element structure, given
/// the slot offset of element 0 within the variable's data region.
///
/// Registers nothing when the elements are not structures. STRING fields
/// nested deeper inside an element (in a structure or array field of the
/// element) are not registered: no access path can reach them yet (#1791).
fn register_array_element_strings(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
    base_slot: u32,
    span: &SourceSpan,
    out: &mut Vec<ElementStringField>,
) -> Result<(), Diagnostic> {
    let IntermediateType::Structure { fields } = element_type else {
        return Ok(());
    };
    let element_bytes = element_type
        .slot_count()
        .ok()
        .and_then(|slots| slots.checked_mul(8))
        .ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                span.clone(),
                "Array element structure is unsupported",
            ))
        })?;
    let total_elements = total_elements(dimensions, span)?;

    let (field_infos, _) = crate::compile_struct::build_struct_fields(fields, span)?;
    for f in &field_infos {
        let (IntermediateType::String { char_width, .. }, Some(max_len)) =
            (&f.field_type, f.string_max_length)
        else {
            continue;
        };
        let element_field_type = if char_width.is_wide() {
            FieldType::WString
        } else {
            FieldType::String
        };
        let desc_index = builder.add_strided_array_descriptor(
            element_field_type as u8,
            total_elements,
            max_len,
            element_bytes,
        );
        out.push(ElementStringField {
            slot_offset: base_slot + f.slot_offset.raw(),
            desc_index,
        });
        // Temp buffers must hold the longest string any access loads.
        if max_len > ctx.max_string_capacity {
            ctx.max_string_capacity = max_len;
        }
    }
    Ok(())
}

/// Returns the number of elements across all dimensions.
fn total_elements(dimensions: &[ArrayDimension], span: &SourceSpan) -> Result<u32, Diagnostic> {
    dimensions.iter().try_fold(1u32, |acc, dim| {
        let size = (dim.upper as i64 - dim.lower as i64 + 1).max(0) as u32;
        acc.checked_mul(size)
            .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Array too large")))
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
/// structure, and registers a strided descriptor for each STRING field of the
/// element structure. Element field values are not initialized: the data
/// region starts zeroed. Only the STRING headers are written, by
/// [`crate::compile_struct_init::initialize_element_strings`].
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

    let total_elements = total_elements(dimensions, span)?;

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

    let total_bytes = total_slots.checked_mul(8).ok_or_else(|| {
        Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow"))
    })?;
    let data_offset = crate::data_region::reserve(ctx, total_bytes, span)?;

    let desc_index = builder.add_array_descriptor(FieldType::Slot as u8, total_slots, 0);

    let mut element_strings = Vec::new();
    register_array_element_strings(
        ctx,
        builder,
        element_type,
        dimensions,
        0,
        span,
        &mut element_strings,
    )?;
    let scratch_var_index =
        (!element_strings.is_empty()).then(|| ctx.allocate_scratch_variable(&id.to_string()));

    ctx.struct_array_vars.insert(
        id.clone(),
        StructArrayVarInfo {
            var_index,
            desc_index,
            data_offset,
            element_type: element_type.clone(),
            dimensions: dimensions.to_vec(),
            scratch_var_index,
            element_strings,
        },
    );

    Ok((
        ironplc_container::debug_section::iec_type_tag::ARRAY,
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
