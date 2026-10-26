//! Structure code generation support.
//!
//! Handles structure variable metadata, field resolution, and
//! structure-related compilation helpers. Separated from compile.rs
//! to keep module sizes within the 1000-line guideline.

use std::collections::HashMap;

use crate::access::{Access, Field, Named};
use ironplc_ir::Located;
use ironplc_ir::{Diagnostic, Label};

use ironplc_container::{SlotIndex, VarIndex};
use ironplc_ir::lowered::{FieldIdx, TypeId, TypeKind, TypeTable};

use super::compile::{CompileContext, VarTypeInfo};
use crate::body::slot_type;

/// Where a structure variable is, and how its fields are reached, as the
/// layout built from the lowered declarations records it.
#[derive(Clone)]
pub(crate) struct StructVarInfo {
    /// Variable table index holding the data region offset.
    pub var_index: VarIndex,
    /// Data region byte offset where this structure's fields start.
    pub data_offset: u32,
    /// Array descriptor index for this structure (treats struct as flat slot array).
    pub desc_index: u16,
    /// The structure's lowered type, whose fields the accesses select.
    pub ty: TypeId,
    /// Scratch variable for STRING array field access. Allocated lazily
    /// only when the struct has at least one STRING array field or one
    /// STRING field of an array-of-struct element. Holds
    /// `struct_data_offset + field_byte_offset` during STR_LOAD/STORE_ARRAY_ELEM.
    pub scratch_var_index: Option<VarIndex>,
    /// Per-field STRING array descriptor info. Maps each of the structure's
    /// own fields that is an array of strings to `(desc_index,
    /// total_elements, max_string_len)`.
    pub string_array_descs: HashMap<FieldIdx, (u16, u32, u16)>,
    /// The STRING fields of the elements of the arrays of structures this
    /// structure holds, at any depth of nested structure fields.
    pub element_strings: Vec<crate::compile_array_struct::ElementStringField>,
}

/// The slot of the field `selected` selects of a structure of the type
/// `record`, from the start of the structure, and the field's type.
pub(crate) fn field_in<S>(
    types: &TypeTable,
    record: TypeId,
    selected: &Field<S>,
) -> Result<(SlotIndex, TypeId), Diagnostic> {
    let field = &selected.field;
    let not_a_structure = || {
        Diagnostic::not_implemented(Label::span(
            field.span(),
            format!("Cannot select field '{field}' -- it is not a field of a structure"),
        ))
    };
    let Some(TypeKind::Structure { fields }) = types.kind(record) else {
        return Err(not_a_structure());
    };
    let mut slot = 0_u32;
    for (position, candidate) in fields.iter().enumerate() {
        if position == selected.idx.index() as usize {
            return Ok((SlotIndex::new(slot), candidate.ty));
        }
        slot = crate::layout::slots(types, candidate.ty)
            .and_then(|slots| slot.checked_add(slots))
            .ok_or_else(|| {
                Diagnostic::not_supported(Label::span(field.span(), "Structure too large"))
            })?;
    }
    Err(Diagnostic::not_implemented(Label::span(
        field.span(),
        format!("Unknown field '{field}'"),
    )))
}

/// Maximum nesting depth for `walk_struct_chain`. Matches the depth guard
/// in `slot_count_inner()`. Defense-in-depth: if the analyzer lets a
/// recursive type through, this prevents a stack overflow during expression
/// compilation.
const MAX_STRUCT_CHAIN_DEPTH: u32 = 32;

/// Resolves the field `structured` names to the information needed for
/// code emission: variable table index, array descriptor index, compile-time
/// slot offset, and the type the field is loaded and stored at.
pub(crate) fn resolve_struct_field_access<S>(
    types: &TypeTable,
    ctx: &CompileContext,
    structured: &Field<S>,
) -> Result<(VarIndex, u16, SlotIndex, VarTypeInfo), Diagnostic> {
    let (root_name, slot_offset, field_type) = walk_struct_chain(types, ctx, structured, 0)?;

    let struct_info = ctx.struct_var(&root_name).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            format!("Variable '{}' is not a structure", root_name),
        ))
    })?;

    let type_info = slot_type(types, field_type).ok_or_else(|| {
        Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            "Cannot read composite field directly (nested struct or array)",
        ))
    })?;

    Ok((
        struct_info.var_index,
        struct_info.desc_index,
        slot_offset,
        type_info,
    ))
}

/// Walks a chain of fields, `record.field`, to resolve the root variable
/// name, accumulated slot offset, and leaf field type, of the types
/// `types`.
///
/// - **Base case** (`Named`): looks up the structure in `ctx.struct_vars`,
///   and the field in its type.
/// - **Recursive case** (`Field`): recurses to resolve the parent, then
///   uses [`field_in`] to resolve the current field within the parent
///   type, accumulating slot offsets.
pub(crate) fn walk_struct_chain<S>(
    types: &TypeTable,
    ctx: &CompileContext,
    structured: &Field<S>,
    depth: u32,
) -> Result<(Named, SlotIndex, TypeId), Diagnostic> {
    if depth > MAX_STRUCT_CHAIN_DEPTH {
        return Err(Diagnostic::not_implemented(Label::span(
            structured.field.span(),
            "Structure nesting exceeds maximum depth (possible recursive type)",
        )));
    }
    match structured.record.as_ref() {
        Access::Named(name) => {
            let struct_info = ctx.struct_var(name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    name.span(),
                    format!("Variable '{}' is not a structure", name),
                ))
            })?;
            let (slot_offset, field_type) = field_in(types, struct_info.ty, structured)?;
            Ok((name.clone(), slot_offset, field_type))
        }
        Access::Field(inner) => {
            let (root, parent_offset, parent_type) =
                walk_struct_chain(types, ctx, inner, depth + 1)?;

            if !matches!(types.kind(parent_type), Some(TypeKind::Structure { .. })) {
                return Err(Diagnostic::not_implemented(Label::span(
                    inner.field.span(),
                    format!("Field '{}' is not a structure type", inner.field),
                )));
            }

            let (field_slot_offset, field_type) = field_in(types, parent_type, structured)?;

            Ok((
                root,
                SlotIndex::new(parent_offset.raw() + field_slot_offset.raw()),
                field_type,
            ))
        }
        // An `Index` record (`arr[i].field`, `s.items[i].field`) is resolved
        // by `compile_array_struct` before the callers reach this walk, and
        // a dereference owns no field of a structure the layout reaches.
        record @ (Access::Index(_) | Access::Deref(_)) => {
            Err(Diagnostic::todo_with_span(record.span()))
        }
    }
}
