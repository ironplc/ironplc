//! The setup of a structure variable's, or an array of structures', data
//! region before `init` or a prologue stores its starting value: the
//! region's offset in the variable's slot, and the header of every STRING it
//! holds, however deeply nested. Separated from `compile_struct.rs` to keep
//! module sizes within the 1000-line guideline.

use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

use ironplc_container::VarIndex;
use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};

use super::compile::{string_region_size, CompileContext};
use super::compile_array_struct::{ElementStringField, StructArrayVarInfo};
use super::compile_struct::StructVarInfo;
use crate::emit::Emitter;
use crate::layout::{elements, slots, string_shape};

/// Sets up the storage the layout gives a structure variable, before `init`
/// stores its fields' values (REQ-LOW-codegen-200): its data-region offset
/// into its slot, the header of every STRING its fields hold, and of every
/// element's copy of each STRING field of the arrays of structures it holds.
/// Its fields are those of its type in `types`.
pub(crate) fn set_up_struct_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    types: &TypeTable,
    info: &StructVarInfo,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let offset_const = ctx.add_i32_constant(info.data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(info.var_index);
    string_headers(emitter, types, info.ty, 0, info.data_offset, span)?;
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

/// Writes the header of each STRING a structure of the type `ty`, from slot
/// `base` of the region at `data_offset`, holds where [`Region::value`]
/// writes one at `Position::Field`: a STRING field, each element of an ARRAY
/// OF STRING field, and those of a structure field, to any depth.
fn string_headers(
    emitter: &mut Emitter,
    types: &TypeTable,
    ty: TypeId,
    base: u32,
    data_offset: u32,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let unsupported =
        || Diagnostic::not_supported(Label::span(span.clone(), "Structure too large"));
    let fields = types.fields(ty).ok_or_else(unsupported)?;
    let mut slot = base;
    for field in fields {
        match types.kind(field.ty) {
            Some(TypeKind::Structure { .. }) => {
                string_headers(emitter, types, field.ty, slot, data_offset, span)?;
            }
            Some(TypeKind::String(shape)) => {
                let (max_length, char_width) = string_shape(*shape).ok_or_else(unsupported)?;
                let at = byte_offset(data_offset, slot, span)?;
                emitter.emit_str_init(at, max_length, char_width);
            }
            Some(TypeKind::Array { element, .. }) => {
                if let Some(TypeKind::String(shape)) = types.kind(*element) {
                    let (max_length, char_width) = string_shape(*shape).ok_or_else(unsupported)?;
                    let total_elements = elements(types, field.ty).ok_or_else(unsupported)?;
                    let stride = string_region_size(max_length, char_width);
                    let at = byte_offset(data_offset, slot, span)?;
                    for index in 0..total_elements {
                        emitter.emit_str_init(at + index * stride, max_length, char_width);
                    }
                }
            }
            Some(
                TypeKind::Bool
                | TypeKind::Integer { .. }
                | TypeKind::BitString { .. }
                | TypeKind::Real { .. }
                | TypeKind::Temporal { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Subrange { .. }
                | TypeKind::FunctionBlock { .. }
                | TypeKind::Program { .. }
                | TypeKind::Reference { .. },
            )
            | None => {}
        }
        slot = slots(types, field.ty)
            .and_then(|slots| slot.checked_add(slots))
            .ok_or_else(unsupported)?;
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

/// The byte offset in the data region of slot `slot` of the region that
/// starts at `data_offset`.
fn byte_offset(data_offset: u32, slot: u32, span: &SourceSpan) -> Result<u32, Diagnostic> {
    slot.checked_mul(8)
        .and_then(|offset| offset.checked_add(data_offset))
        .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow")))
}
