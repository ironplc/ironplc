//! Initialization of the elements of an array.
//!
//! Every element of an array is written: first the element type's default,
//! then the listed initial values on top. An element past the end of a short
//! initializer, or of an array without one, takes the element type's default
//! (IEC 61131-3 2.4.3.2), so the same code serves the one-time
//! initialization and the reset of a `VAR_TEMP` array on every execution of
//! its POU, when the data region still holds the previous values.

use ironplc_analyzer::intermediate_type::{ArrayDimension, IntermediateType};
use ironplc_container::{SlotIndex, VarIndex};
use ironplc_dsl::common::{ArrayInitialElementKind, ConstantKind};
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use super::compile::{emit_string_literal_load, CompileContext, DEFAULT_OP_TYPE};
use super::compile_array::flatten_array_initial_values;
use super::compile_default::LeafDefault;
use super::compile_expr::{compile_constant, emit_truncation};
use super::compile_struct::{
    build_struct_fields, emit_truncation_for_field, resolve_field_op_type,
};
use super::compile_struct_init::{initialize_struct_fields, FieldInitInfo};
use crate::emit::Emitter;

/// What an array's data region holds before its initialization runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegionState {
    /// Zeros: the one-time initialization of the region.
    Zeroed,
    /// The values of a previous execution: the reset of a temporary.
    Stale,
}

/// Emits the initialization of the plain array variable `id`: stores its
/// data-region offset into its slot, then writes every element.
///
/// Over a `Zeroed` region, elements whose default is zero are not written.
pub(crate) fn initialize_array_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    id: &Id,
    initial_values: &[ArrayInitialElementKind],
    region: RegionState,
) -> Result<(), Diagnostic> {
    let Some(info) = ctx.array_vars.get(id) else {
        return Ok(());
    };
    let var_index = info.var_index;
    let desc_index = info.desc_index;
    let data_offset = info.data_offset;
    let element_vti = info.element_var_type_info;
    let element_default = info.element_default;
    let total_elements = info.total_elements as usize;
    let string_char_width = info.is_string_element.then_some(info.string_char_width);

    let offset_const = ctx.add_i32_constant(data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(var_index);

    let values = flatten_array_initial_values(initial_values)?;

    if let Some(char_width) = string_char_width {
        // Every element becomes the empty string, then the listed values are
        // stored. String literals are encoded at the element width so the
        // element's encoding check passes.
        emitter.emit_str_init_array(var_index, desc_index);
        for (i, value) in values.iter().enumerate() {
            if let ConstantKind::CharacterString(lit) = value {
                emit_string_literal_load(emitter, ctx, &lit.value, char_width);
            } else {
                compile_constant(emitter, ctx, value, DEFAULT_OP_TYPE)?;
            }
            let idx_const = ctx.add_i32_constant(i as i32);
            emitter.emit_load_const_i32(idx_const);
            emitter.emit_str_store_array_elem(var_index, desc_index);
        }
        return Ok(());
    }

    let op_type = (element_vti.op_width, element_vti.signedness);
    if region == RegionState::Stale || element_default != LeafDefault::Zero {
        for i in values.len()..total_elements {
            element_default.emit(emitter, ctx, op_type);
            let idx_const = ctx.add_i32_constant(i as i32);
            emitter.emit_load_const_i32(idx_const);
            emitter.emit_store_array(var_index, desc_index);
        }
    }
    for (i, value) in values.iter().enumerate() {
        compile_constant(emitter, ctx, value, op_type)?;
        emit_truncation(emitter, element_vti);
        let idx_const = ctx.add_i32_constant(i as i32);
        emitter.emit_load_const_i32(idx_const);
        emitter.emit_store_array(var_index, desc_index);
    }
    Ok(())
}

/// A data region laid out in 8-byte slots, a structure or an array of
/// structures, addressed through its slot descriptor.
pub(crate) struct SlotRegion {
    /// Variable table index holding the region's data offset.
    pub var_index: VarIndex,
    /// Slot-typed descriptor over the whole region.
    pub desc_index: u16,
    /// Data region byte offset of the region.
    pub data_offset: u32,
}

/// Returns the element type and dimensions of `ty` when it is an array laid
/// out in slots by [`initialize_slot_array`], that is an array whose
/// elements are not STRING values, at any depth of nested arrays.
pub(crate) fn slot_array(ty: &IntermediateType) -> Option<(&IntermediateType, &[ArrayDimension])> {
    let IntermediateType::Array {
        element_type,
        dimensions,
    } = ty
    else {
        return None;
    };
    let mut leaf = element_type.as_ref();
    while let IntermediateType::Array { element_type, .. } = leaf {
        leaf = element_type.as_ref();
    }
    if matches!(leaf, IntermediateType::String { .. }) {
        return None;
    }
    Some((element_type.as_ref(), dimensions.as_slice()))
}

/// Emits the initialization of every element of an array that starts at
/// `first_slot` of `region`: a single-slot element gets its default, then
/// `initial_values` are stored on top; a structure element gets its
/// fields' defaults; a nested array element is initialized the same way.
///
/// The headers of STRING values inside the elements are written by
/// `initialize_struct_fields`, which this calls for structure elements.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_slot_array(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    region: &SlotRegion,
    first_slot: u32,
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
    initial_values: &[ArrayInitialElementKind],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let element_slots = element_type.slot_count().map_err(|_| {
        Diagnostic::not_implemented(Label::span(span.clone(), "Unsupported array element type"))
    })?;
    let total_elements = dimensions.iter().try_fold(1u32, |acc, dim| {
        u32::try_from(dim.upper as i64 - dim.lower as i64 + 1)
            .ok()
            .and_then(|size| acc.checked_mul(size))
    });
    let total_elements = total_elements
        .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Array too large")))?;
    let slot_of = |i: u32| first_slot + i * element_slots;

    if let Some(op_type) = resolve_field_op_type(element_type) {
        let values = flatten_array_initial_values(initial_values)?;
        let default = LeafDefault::of(element_type);
        for i in values.len() as u32..total_elements {
            default.emit(emitter, ctx, op_type);
            store_slot(emitter, ctx, region, slot_of(i));
        }
        for (i, value) in values.iter().enumerate() {
            compile_constant(emitter, ctx, value, op_type)?;
            emit_truncation_for_field(emitter, element_type);
            store_slot(emitter, ctx, region, slot_of(i as u32));
        }
        return Ok(());
    }

    if !initial_values.is_empty() {
        return Err(Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Initial values for an array of structures or of arrays",
        )));
    }
    match element_type {
        IntermediateType::Structure { fields } => {
            let (fields, _) = build_struct_fields(fields, span)?;
            for i in 0..total_elements {
                let base = slot_of(i);
                let infos: Vec<FieldInitInfo> = fields
                    .iter()
                    .map(|f| FieldInitInfo {
                        name: f.name.clone(),
                        slot_offset: SlotIndex::new(base + f.slot_offset.raw()),
                        field_type: f.field_type.clone(),
                        op_type: f.op_type,
                        string_max_length: f.string_max_length,
                    })
                    .collect();
                initialize_struct_fields(
                    emitter,
                    ctx,
                    region.var_index,
                    region.desc_index,
                    region.data_offset,
                    &infos,
                    &[],
                    span,
                )?;
            }
        }
        IntermediateType::Array {
            element_type: inner,
            dimensions: inner_dimensions,
        } => {
            for i in 0..total_elements {
                initialize_slot_array(
                    emitter,
                    ctx,
                    region,
                    slot_of(i),
                    inner,
                    inner_dimensions,
                    &[],
                    span,
                )?;
            }
        }
        // STRING and the types that cannot be array elements of a slot
        // region have nothing to write here.
        _ => {}
    }
    Ok(())
}

/// Stores the value on the stack into `slot` of `region`.
fn store_slot(emitter: &mut Emitter, ctx: &mut CompileContext, region: &SlotRegion, slot: u32) {
    let idx_const = ctx.add_i32_constant(slot as i32);
    emitter.emit_load_const_i32(idx_const);
    emitter.emit_store_array(region.var_index, region.desc_index);
}
