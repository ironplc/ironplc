//! Structure initialization code generation.
//!
//! Emits the init-function code that gives each field of a structure
//! variable its initial value: an explicit initializer when the declaration
//! has one, otherwise the type's default. Separated from `compile_struct.rs`
//! to keep module sizes within the 1000-line guideline.

use std::collections::HashMap;

use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::intermediate_type::IntermediateType;
use ironplc_container::{SlotIndex, VarIndex};
use ironplc_dsl::common::{StructInitialValueAssignmentKind, StructureElementInit};

use super::compile::{CompileContext, OpType, OpWidth, DEFAULT_STRING_MAX_LENGTH};
use super::compile_expr::compile_constant;
use super::compile_setup::emit_zero_const;
use super::compile_struct::{build_struct_fields, emit_truncation_for_field};
use crate::emit::Emitter;

/// Emits a constant load for the type-appropriate default value of a struct field.
///
/// For subrange types, emits the subrange's lower bound (min_value) as an i32/i64
/// constant, since IEC 61131-3 §2.4.3.1 specifies the default is the "leftmost
/// value" of the subrange. For all other types, emits zero via `emit_zero_const`.
fn emit_default_for_field(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    field_type: &IntermediateType,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    if let IntermediateType::Subrange { min_value, .. } = field_type {
        match op_type.0 {
            OpWidth::W32 => {
                let pool_index = ctx.add_i32_constant(*min_value as i32);
                emitter.emit_load_const_i32(pool_index);
            }
            OpWidth::W64 => {
                let pool_index = ctx.add_i64_constant(*min_value as i64);
                emitter.emit_load_const_i64(pool_index);
            }
            _ => {
                emit_zero_const(emitter, ctx, op_type);
            }
        }
    } else {
        emit_zero_const(emitter, ctx, op_type);
    }
    Ok(())
}

/// Compiles an explicit initial value for a structure field.
///
/// Handles constant expressions (integer/real/boolean literals) and
/// enumerated values from `StructInitialValueAssignmentKind`.
///
/// Note the `Array`/`Structure` arm returns `Ok(())` without pushing a value.
/// For a well-typed program that arm is unreachable -- `op_type` is `None` for
/// a struct- or array-typed field, so those go through the recursion in
/// `initialize_struct_fields` instead. It is reached only when the initializer
/// does not match the field's type, where pushing nothing leaves the caller's
/// unconditional store unbalanced.
fn compile_struct_field_init(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    init: &StructInitialValueAssignmentKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match init {
        StructInitialValueAssignmentKind::Constant(constant) => {
            compile_constant(emitter, ctx, constant, op_type)
        }
        StructInitialValueAssignmentKind::EnumeratedValue(ev) => {
            // REQ-EN-codegen-050: Resolve enum value to ordinal and push as i32 constant.
            let ordinal = crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, ev)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            Ok(())
        }
        StructInitialValueAssignmentKind::Array(_)
        | StructInitialValueAssignmentKind::Structure(_) => {
            // Unreachable for a well-typed program: see the note on this
            // function. Nested structures are handled by the recursion in
            // `initialize_struct_fields`, not here.
            Ok(())
        }
        StructInitialValueAssignmentKind::Expression(expr) => {
            // A general (possibly non-constant) expression, e.g.
            // `pDevice^.Delta` -- `ironplcc check` fully supports this;
            // codegen does not yet implement evaluating it at instance
            // construction time.
            Err(Diagnostic::not_implemented(Label::span(
                expr.span(),
                "Expression-valued struct/FB-instance field initializer",
            )))
        }
        StructInitialValueAssignmentKind::LateBound(late_bound) => {
            // `xform_resolve_late_bound_expr_kind` replaces every one of
            // these with an enumerated value or an expression, so reaching
            // codegen with one means that pass did not run.
            Err(Diagnostic::internal_error_at(Label::span(
                late_bound.value.span(),
                "Unresolved struct/FB-instance field initializer",
            )))
        }
    }
}

/// Pre-extracted field info for initialization, avoiding borrow conflicts.
///
/// Created by extracting data from `StructFieldInfo` before passing `ctx`
/// mutably to `initialize_struct_fields`.
pub(crate) struct FieldInitInfo {
    pub name: String,
    pub slot_offset: SlotIndex,
    pub field_type: IntermediateType,
    pub op_type: Option<OpType>,
    /// For STRING fields, the maximum character length. `None` for non-STRING fields.
    pub string_max_length: Option<u16>,
}

/// Initializes fields of a structure variable.
///
/// Emits constant-load + STORE_ARRAY for each leaf field. Uses explicit
/// initial values from `element_inits` when available, otherwise emits
/// type-appropriate defaults (zero or subrange lower bound).
///
/// `span` locates the variable declaration being initialized; a nested field
/// type the compiler cannot lay out is reported there.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_struct_fields(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    var_index: VarIndex,
    desc_index: u16,
    struct_data_offset: u32,
    fields: &[FieldInitInfo],
    element_inits: &[StructureElementInit],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    // Build a map of explicit initializers
    let init_map: HashMap<String, &StructInitialValueAssignmentKind> = element_inits
        .iter()
        .map(|e| (e.name.to_string().to_lowercase(), &e.init))
        .collect();

    // Iterate over fields in declaration order (Vec guarantees deterministic order)
    for field_info in fields {
        let slot_idx = field_info.slot_offset;

        if let Some(op_type) = field_info.op_type {
            // Leaf field (primitive/enum)
            if let Some(init_value) = init_map.get(&field_info.name) {
                // Emit explicit initial value
                compile_struct_field_init(emitter, ctx, init_value, op_type)?;
            } else {
                // Emit type-appropriate default value
                emit_default_for_field(emitter, ctx, &field_info.field_type, op_type)?;
            }

            // Truncate narrow types (e.g., SINT stored in W32 slot)
            emit_truncation_for_field(emitter, &field_info.field_type);

            // Store to field slot
            let idx_const = ctx.add_i32_constant(slot_idx.raw() as i32);
            emitter.emit_load_const_i32(idx_const);
            emitter.emit_store_array(var_index, desc_index);
        } else if let IntermediateType::Structure { fields } = &field_info.field_type {
            // Nested structure field — recursively initialize inner fields.
            // Extract nested initializers from the init map for this field.
            let nested_inits: Vec<StructureElementInit> =
                if let Some(StructInitialValueAssignmentKind::Structure(nested)) =
                    init_map.get(&field_info.name)
                {
                    nested.to_vec()
                } else {
                    // No explicit init — inner fields will be default-initialized.
                    vec![]
                };

            // Build inner field metadata with offsets adjusted to the parent's base.
            let (inner_fields, _) = build_struct_fields(fields, span)?;
            let inner_field_infos: Vec<FieldInitInfo> = inner_fields
                .iter()
                .map(|f| FieldInitInfo {
                    name: f.name.clone(),
                    slot_offset: SlotIndex::new(slot_idx.raw() + f.slot_offset.raw()),
                    field_type: f.field_type.clone(),
                    op_type: f.op_type,
                    string_max_length: f.string_max_length,
                })
                .collect();

            initialize_struct_fields(
                emitter,
                ctx,
                var_index,
                desc_index,
                struct_data_offset,
                &inner_field_infos,
                &nested_inits,
                span,
            )?;
        } else if let IntermediateType::String { char_width, .. } = &field_info.field_type {
            // STRING field — initialize the header in the data region.
            if let Some(max_length) = field_info.string_max_length {
                let byte_offset = struct_data_offset + slot_idx.raw() * 8;
                emitter.emit_str_init(byte_offset, max_length, *char_width);
            }
        } else if let IntermediateType::Array {
            element_type,
            dimensions: array_dims,
        } = &field_info.field_type
        {
            if let IntermediateType::String {
                max_len,
                char_width,
            } = element_type.as_ref()
            {
                // STRING/WSTRING array field — initialize headers for each string element.
                let max_length = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
                let total_elements = array_dims
                    .iter()
                    .fold(1u32, |acc, d| acc * (d.upper - d.lower + 1) as u32);
                let stride = super::compile::string_region_size(max_length, *char_width);
                let field_byte_offset = struct_data_offset + slot_idx.raw() * 8;
                for i in 0..total_elements {
                    let elem_byte_offset = field_byte_offset + i * stride;
                    emitter.emit_str_init(elem_byte_offset, max_length, *char_width);
                }
            }
        }
    }
    Ok(())
}
