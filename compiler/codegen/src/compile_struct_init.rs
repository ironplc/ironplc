//! The setup of a structure variable's, or an array of structures', data
//! region before `init` or a prologue stores its starting value: the
//! region's offset in the variable's slot, and the header of every STRING it
//! holds, however deeply nested. Separated from `compile_struct.rs` to keep
//! module sizes within the 1000-line guideline.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_container::{SlotIndex, VarIndex};

use super::compile::{string_region_size, CompileContext, DEFAULT_STRING_MAX_LENGTH};
use super::compile_array_struct::{ElementStringField, StructArrayVarInfo};
use super::compile_struct::{build_struct_fields, StructVarInfo};
use crate::emit::Emitter;

/// Sets up the storage the layout gives a structure variable, before `init`
/// stores its fields' values (REQ-LOW-codegen-200): its data-region offset
/// into its slot, the header of every STRING its fields hold, and of every
/// element's copy of each STRING field of the arrays of structures it holds.
pub(crate) fn set_up_struct_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    info: &StructVarInfo,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let offset_const = ctx.add_i32_constant(info.data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(info.var_index);
    let field_types = struct_field_types(&info.fields);
    string_headers(emitter, &field_types, 0, info.data_offset, span)?;
    initialize_element_strings(
        emitter,
        ctx,
        info.data_offset,
        info.scratch_var_index,
        &info.element_strings,
        span,
    )
}

/// Sets up the storage the layout gives a variable that is an array of
/// structures, as [`set_up_struct_variable`] does a structure's: its
/// data-region offset into its slot, and the header of every element's copy
/// of each STRING field.
pub(crate) fn set_up_struct_array_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    info: &StructArrayVarInfo,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let offset_const = ctx.add_i32_constant(info.data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(info.var_index);
    initialize_element_strings(
        emitter,
        ctx,
        info.data_offset,
        info.scratch_var_index,
        &info.element_strings,
        span,
    )
}

/// Writes the header of each STRING the structure `fields`, from slot `base`
/// of the region at `data_offset`, holds where [`Region::value`] writes one
/// at `Position::Field`: a STRING field, each element of an ARRAY OF STRING
/// field, and those of a structure field, to any depth.
fn string_headers(
    emitter: &mut Emitter,
    fields: &[(String, SlotIndex, SemanticType)],
    base: u32,
    data_offset: u32,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    for (_, slot_offset, field_type) in fields {
        let slot = base + slot_offset.raw();
        match field_type {
            SemanticType::Structure { fields } => {
                let (infos, _) = build_struct_fields(fields, span)?;
                let fields: Vec<_> = infos
                    .into_iter()
                    .map(|f| (f.name, f.slot_offset, f.field_type))
                    .collect();
                string_headers(emitter, &fields, slot, data_offset, span)?;
            }
            SemanticType::String {
                max_len,
                char_width,
            } => {
                let max_length = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
                let at = byte_offset(data_offset, slot, span)?;
                emitter.emit_str_init(at, max_length, *char_width);
            }
            SemanticType::Array {
                element_type,
                dimensions,
            } => {
                let SemanticType::String {
                    max_len,
                    char_width,
                } = element_type.as_ref()
                else {
                    continue;
                };
                let max_length = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
                let total_elements = dimensions
                    .iter()
                    .fold(1u32, |acc, d| acc * (d.upper - d.lower + 1) as u32);
                let stride = string_region_size(max_length, *char_width);
                let at = byte_offset(data_offset, slot, span)?;
                for index in 0..total_elements {
                    emitter.emit_str_init(at + index * stride, max_length, *char_width);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Writes the header of every element's copy of each STRING field of an
/// array of structures in a variable's data region.
///
/// One `STR_INIT_ARRAY` per field covers every element, through the strided
/// descriptor registered for the field (ADR-0054). The opcode reads the base
/// address from a variable, so it is first computed into the scratch
/// variable. Without this, the headers stay zeroed, and a zero `char_width`
/// traps on first access.
fn initialize_element_strings(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    data_offset: u32,
    scratch_var_index: Option<VarIndex>,
    fields: &[ElementStringField],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if fields.is_empty() {
        return Ok(());
    }
    let scratch = scratch_var_index.ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "STRING fields of array-of-struct elements have no scratch variable",
        ))
    })?;
    for field in fields {
        let byte_offset = byte_offset(data_offset, field.slot_offset, span)?;
        let offset_const = ctx.add_i32_constant(byte_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(scratch);
        emitter.emit_str_init_array(scratch, field.desc_index);
    }
    Ok(())
}

/// The fields of a structure variable as `(name, slot offset, type)`.
fn struct_field_types(
    fields: &[super::compile_struct::StructFieldInfo],
) -> Vec<(String, SlotIndex, SemanticType)> {
    fields
        .iter()
        .map(|f| (f.name.clone(), f.slot_offset, f.field_type.clone()))
        .collect()
}

/// The byte offset in the data region of slot `slot` of the region that
/// starts at `data_offset`.
fn byte_offset(data_offset: u32, slot: u32, span: &SourceSpan) -> Result<u32, Diagnostic> {
    slot.checked_mul(8)
        .and_then(|offset| offset.checked_add(data_offset))
        .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow")))
}
