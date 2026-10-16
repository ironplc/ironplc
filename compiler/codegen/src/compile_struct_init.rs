//! Structure initialization code generation.
//!
//! Stores the starting value the analyzer resolved for a structure variable,
//! or an array of structures, into its data region: every field and element,
//! however deeply nested. Separated from `compile_struct.rs` to keep module
//! sizes within the 1000-line guideline.

use crate::initial_value::{InitialValue, StringValue};
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::semantic_type::{ArrayDimension, SemanticType};
use ironplc_container::{SlotIndex, VarIndex};

use super::compile::{string_region_size, CompileContext, DEFAULT_STRING_MAX_LENGTH};
use super::compile_array_struct::{ElementStringField, StructArrayVarInfo};
use super::compile_initial_value::{emit_reference, emit_scalar, emit_string_store, is_cleared};
use super::compile_struct::{
    build_struct_fields, emit_truncation_for_field, resolve_field_op_type, StructVarInfo,
};
use crate::emit::Emitter;

/// When the stores of a region's initialization run.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Writes {
    /// Once, when the program starts, over a data region that starts
    /// cleared.
    Once,
    /// On every call of a function, over whatever the last call left.
    EveryCall,
}

/// Emits the initialization of a structure variable: stores its data-region
/// offset into the variable's slot, then `value` into every field.
pub(crate) fn initialize_struct_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    info: &StructVarInfo,
    value: &InitialValue,
    writes: Writes,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let offset_const = ctx.add_i32_constant(info.data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(info.var_index);

    let mut region = Region {
        var_index: info.var_index,
        desc_index: info.desc_index,
        data_offset: info.data_offset,
        writes,
        span: span.clone(),
        element_strings: Vec::new(),
    };
    let field_types = struct_field_types(&info.fields);
    region.structure(emitter, ctx, &field_types, value, 0, Position::Field)?;

    initialize_element_strings(
        emitter,
        ctx,
        info.data_offset,
        info.scratch_var_index,
        &info.element_strings,
        span,
    )?;
    region.store_element_strings(emitter, ctx);
    Ok(())
}

/// Emits the initialization of a variable that is an array of structures:
/// stores its data-region offset into the variable's slot, writes the header
/// of every element's STRING fields, then stores `value` into every element.
pub(crate) fn initialize_struct_array_variable(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    info: &StructArrayVarInfo,
    value: &InitialValue,
    writes: Writes,
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
    )?;

    let mut region = Region {
        var_index: info.var_index,
        desc_index: info.desc_index,
        data_offset: info.data_offset,
        writes,
        span: span.clone(),
        element_strings: Vec::new(),
    };
    region.array(
        emitter,
        ctx,
        &info.element_type,
        &info.dimensions,
        value,
        0,
        Position::Field,
    )?;
    region.store_element_strings(emitter, ctx);
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

/// Where in a region a value is stored, which decides how its STRING
/// headers are written.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    /// A field of the variable's structure, or of a structure field of it:
    /// a STRING header is written where the field is met.
    Field,
    /// An element of an array that is such a field, or the variable.
    Element,
    /// A field of a structure that is an element of an array: the header of
    /// a STRING field is written for every element at once, through the
    /// field's strided descriptor, after the fields are stored.
    ElementField,
    /// Deeper inside an array element, where no STRING header is written.
    Nested,
}

/// A data region being initialized, addressed as one flat array of slots.
struct Region {
    var_index: VarIndex,
    desc_index: u16,
    data_offset: u32,
    writes: Writes,
    span: SourceSpan,
    /// The STRING values of array elements, stored after the headers the
    /// element strings' descriptors write: `(byte offset, value)`.
    element_strings: Vec<(u32, StringValue)>,
}

impl Region {
    /// Stores the fields of the structure `value` at slot `base`. `fields`
    /// are the structure's fields with their slot offsets in it.
    fn structure(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        fields: &[(String, SlotIndex, SemanticType)],
        value: &InitialValue,
        base: u32,
        position: Position,
    ) -> Result<(), Diagnostic> {
        for (name, slot_offset, field_type) in fields {
            let field_value = field_value(value, name).ok_or_else(|| self.missing(name))?;
            let slot = base + slot_offset.raw();
            self.value(emitter, ctx, field_type, field_value, slot, position)?;
        }
        Ok(())
    }

    /// Stores `value`, of `value_type`, at slot `slot`.
    fn value(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        value_type: &SemanticType,
        value: &InitialValue,
        slot: u32,
        position: Position,
    ) -> Result<(), Diagnostic> {
        match value_type {
            SemanticType::Structure { fields } => {
                let (infos, _) = build_struct_fields(fields, &self.span)?;
                let fields: Vec<_> = infos
                    .into_iter()
                    .map(|f| (f.name, f.slot_offset, f.field_type))
                    .collect();
                let inner = match position {
                    Position::Field => Position::Field,
                    Position::Element => Position::ElementField,
                    Position::ElementField | Position::Nested => Position::Nested,
                };
                self.structure(emitter, ctx, &fields, value, slot, inner)
            }
            SemanticType::String {
                max_len,
                char_width,
            } => {
                let InitialValue::String(string) = value else {
                    return Err(self.mismatch());
                };
                let byte_offset = byte_offset(self.data_offset, slot, &self.span)?;
                match position {
                    Position::Field => {
                        let max_length =
                            max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
                        emitter.emit_str_init(byte_offset, max_length, *char_width);
                        emit_string_store(emitter, ctx, string, byte_offset);
                    }
                    Position::ElementField => {
                        if !string.chars.is_empty() {
                            self.element_strings.push((byte_offset, string.clone()));
                        }
                    }
                    Position::Element | Position::Nested => self.unreachable_string(string)?,
                }
                Ok(())
            }
            SemanticType::Array {
                element_type,
                dimensions,
            } => self.array(
                emitter,
                ctx,
                element_type,
                dimensions,
                value,
                slot,
                position,
            ),
            _ => self.leaf(emitter, ctx, value_type, value, slot, position),
        }
    }

    /// Stores the elements of the array `value` from slot `base`.
    #[allow(clippy::too_many_arguments)]
    fn array(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        element_type: &SemanticType,
        dimensions: &[ArrayDimension],
        value: &InitialValue,
        base: u32,
        position: Position,
    ) -> Result<(), Diagnostic> {
        let InitialValue::Array(elements) = value else {
            return Err(self.mismatch());
        };
        if let SemanticType::String {
            max_len,
            char_width,
        } = element_type
        {
            return self.string_array(
                emitter,
                ctx,
                elements,
                *max_len,
                *char_width,
                dimensions,
                base,
                position,
            );
        }
        let element_slots = element_type.slot_count().map_err(|_| {
            Diagnostic::not_implemented(Label::span(
                self.span.clone(),
                "Array element type is unsupported",
            ))
        })?;
        let inner = match position {
            Position::Field => Position::Element,
            Position::Element | Position::ElementField | Position::Nested => Position::Nested,
        };
        for (index, element) in elements.iter().enumerate() {
            let slot = base + index as u32 * element_slots;
            self.value(emitter, ctx, element_type, element, slot, inner)?;
        }
        Ok(())
    }

    /// Stores the elements of a STRING array field, which are packed one
    /// string region apart rather than in whole slots.
    #[allow(clippy::too_many_arguments)]
    fn string_array(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        elements: &[InitialValue],
        max_len: Option<u128>,
        char_width: ironplc_container::CharWidth,
        dimensions: &[ArrayDimension],
        base: u32,
        position: Position,
    ) -> Result<(), Diagnostic> {
        let strings = elements
            .iter()
            .map(|element| match element {
                InitialValue::String(string) => Ok(string),
                _ => Err(self.mismatch()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if position != Position::Field {
            // No header is written for a STRING array inside an array
            // element, so no value can be stored there either.
            for string in strings {
                self.unreachable_string(string)?;
            }
            return Ok(());
        }
        let max_length = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
        let total_elements = dimensions
            .iter()
            .fold(1u32, |acc, d| acc * (d.upper - d.lower + 1) as u32);
        let stride = string_region_size(max_length, char_width);
        let field_byte_offset = byte_offset(self.data_offset, base, &self.span)?;
        for index in 0..total_elements {
            emitter.emit_str_init(field_byte_offset + index * stride, max_length, char_width);
        }
        for (index, string) in strings.into_iter().enumerate() {
            emit_string_store(
                emitter,
                ctx,
                string,
                field_byte_offset + index as u32 * stride,
            );
        }
        Ok(())
    }

    /// Stores the scalar or reference `value`, of `value_type`, at slot
    /// `slot`. A cleared value inside an array is not stored when the region
    /// starts cleared.
    fn leaf(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        value_type: &SemanticType,
        value: &InitialValue,
        slot: u32,
        position: Position,
    ) -> Result<(), Diagnostic> {
        if self.writes == Writes::Once && position != Position::Field && is_cleared(value) {
            return Ok(());
        }
        let op_type = resolve_field_op_type(value_type).ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                self.span.clone(),
                "Structure field type is unsupported",
            ))
        })?;
        match value {
            InitialValue::Scalar(scalar) => emit_scalar(emitter, ctx, scalar, op_type, &self.span)?,
            InitialValue::Reference(reference) => emit_reference(emitter, ctx, reference)?,
            // A field initializer that is an expression (an extension) is
            // evaluated when the instance is created, which a structure
            // field does not support yet.
            InitialValue::Expression(_) => {
                return Err(Diagnostic::not_implemented(Label::span(
                    self.span.clone(),
                    "Expression-valued struct/FB-instance field initializer",
                )))
            }
            _ => return Err(self.mismatch()),
        }
        emit_truncation_for_field(emitter, value_type);
        let idx_const = ctx.add_i32_constant(slot as i32);
        emitter.emit_load_const_i32(idx_const);
        emitter.emit_store_array(self.var_index, self.desc_index);
        Ok(())
    }

    /// Stores the STRING values of array elements, whose headers have been
    /// written by now.
    fn store_element_strings(&mut self, emitter: &mut Emitter, ctx: &mut CompileContext) {
        for (byte_offset, string) in std::mem::take(&mut self.element_strings) {
            emit_string_store(emitter, ctx, &string, byte_offset);
        }
    }

    /// Refuses a STRING value where no header is written.
    fn unreachable_string(&self, string: &StringValue) -> Result<(), Diagnostic> {
        match string.chars.is_empty() {
            true => Ok(()),
            false => Err(Diagnostic::not_implemented(Label::span(
                self.span.clone(),
                "Initial value of a STRING nested inside an array element",
            ))),
        }
    }

    fn missing(&self, name: &str) -> Diagnostic {
        Diagnostic::internal_error_at(Label::span(
            self.span.clone(),
            format!("Initial value has no value for field '{name}'"),
        ))
    }

    fn mismatch(&self) -> Diagnostic {
        Diagnostic::internal_error_at(Label::span(
            self.span.clone(),
            "Initial value does not have the shape of its type",
        ))
    }
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

/// The value of the field `name` (lower case) of the structure `value`.
fn field_value<'a>(value: &'a InitialValue, name: &str) -> Option<&'a InitialValue> {
    match value {
        InitialValue::Structure(fields) => fields
            .iter()
            .find(|field| field.name.lower_case() == name)
            .map(|field| &field.value),
        _ => None,
    }
}

/// The byte offset in the data region of slot `slot` of the region that
/// starts at `data_offset`.
fn byte_offset(data_offset: u32, slot: u32, span: &SourceSpan) -> Result<u32, Diagnostic> {
    slot.checked_mul(8)
        .and_then(|offset| offset.checked_add(data_offset))
        .ok_or_else(|| Diagnostic::not_supported(Label::span(span.clone(), "Data region overflow")))
}
