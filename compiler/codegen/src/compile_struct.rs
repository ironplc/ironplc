//! Structure code generation support.
//!
//! Handles structure variable metadata, field resolution, and
//! structure-related compilation helpers. Separated from compile.rs
//! to keep module sizes within the 1000-line guideline.

use std::collections::{HashMap, HashSet};

use crate::access::{Access, Field};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::semantic_type::{SemanticStructField, SemanticType, SlotCountError};
use ironplc_container::FieldType;
use ironplc_container::{ContainerBuilder, SlotIndex, VarIndex};
use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};

use super::compile::{CompileContext, VarTypeInfo, DEFAULT_STRING_MAX_LENGTH};
use crate::lowered::slot_type;

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
    /// Per-field STRING array descriptor info. Maps field name (lowercase) to
    /// `(desc_index, total_elements, max_string_len)`.
    pub string_array_descs: HashMap<String, (u16, u32, u16)>,
    /// The STRING fields of the elements of the arrays of structures this
    /// structure holds, at any depth of nested structure fields.
    pub element_strings: Vec<crate::compile_array_struct::ElementStringField>,
}

/// A field of a structure, as the walk of the AST declarations lays it out.
#[derive(Clone)]
pub(crate) struct StructFieldInfo {
    /// Slot offset relative to the containing structure's base.
    pub slot_offset: SlotIndex,
    /// The field's semantic type (for nested resolution).
    pub field_type: SemanticType,
    /// For STRING fields, the maximum character length. `None` for non-STRING fields.
    pub string_max_length: Option<u16>,
}

/// Builds field metadata, in declaration order, from a structure's semantic
/// type.
///
/// Returns `Err` if any field has an unsupported type (STRING, WSTRING,
/// FunctionBlock). Nested structures are NOT flattened — each level is a
/// separate field list.
pub(crate) fn build_struct_fields(
    fields: &[SemanticStructField],
    span: &SourceSpan,
) -> Result<Vec<StructFieldInfo>, Diagnostic> {
    let mut field_list = Vec::with_capacity(fields.len());
    let mut names = HashSet::with_capacity(fields.len());
    let mut slot_offset = SlotIndex::new(0);
    for field in fields {
        let field_slots = field.field_type.slot_count().map_err(|e| {
            let msg = match e {
                SlotCountError::UnsupportedFieldType => format!(
                    "Structure field '{}' has unsupported type (STRING, WSTRING, or FunctionBlock)",
                    field.name
                ),
                SlotCountError::MaxDepthExceeded => format!(
                    "Structure field '{}' exceeds maximum nesting depth (possible recursive type)",
                    field.name
                ),
                SlotCountError::Overflow => format!(
                    "Structure field '{}' is too large (slot count overflows)",
                    field.name
                ),
            };
            Diagnostic::not_implemented(Label::span(span.clone(), msg))
        })?;
        let name = field.name.to_string().to_lowercase();
        let string_max_length = match &field.field_type {
            SemanticType::String { max_len, .. } => {
                Some(max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16)
            }
            _ => None,
        };

        // Defense-in-depth: detect duplicate field names (case-insensitive).
        // The analyzer should reject these, but if one slips through, silently
        // laying out both would leave the first field inaccessible by name
        // while it still occupies slots in the layout.
        if !names.insert(name.clone()) {
            return Err(Diagnostic::internal_error_at(Label::span(
                span.clone(),
                format!(
                    "Structure has duplicate field name '{}' (case-insensitive)",
                    name
                ),
            )));
        }
        field_list.push(StructFieldInfo {
            slot_offset,
            field_type: field.field_type.clone(),
            string_max_length,
        });
        slot_offset = SlotIndex::new(slot_offset.raw() + field_slots);
    }
    Ok(field_list)
}

/// The slot of the field `field` of a structure of the type `record`, from
/// the start of the structure, and the field's type.
pub(crate) fn field_in(
    types: &TypeTable,
    record: TypeId,
    field: &Id,
) -> Result<(SlotIndex, TypeId), Diagnostic> {
    let not_a_structure = || {
        Diagnostic::not_implemented(Label::span(
            field.span(),
            format!("Cannot select field '{field}' -- it is not a field of a structure"),
        ))
    };
    let Some(TypeKind::Structure { fields }) = types.kind(record) else {
        return Err(not_a_structure());
    };
    let name = field.to_string().to_lowercase();
    let mut slot = 0_u32;
    for candidate in fields {
        if candidate.name.to_string().to_lowercase() == name {
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
    let (root_name, slot_offset, field_type) =
        walk_struct_chain(types, ctx, &structured.record, &structured.field, 0)?;

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
    record: &Access<S>,
    field: &Id,
    depth: u32,
) -> Result<(Id, SlotIndex, TypeId), Diagnostic> {
    if depth > MAX_STRUCT_CHAIN_DEPTH {
        return Err(Diagnostic::not_implemented(Label::span(
            field.span(),
            "Structure nesting exceeds maximum depth (possible recursive type)",
        )));
    }
    match record {
        Access::Named(name) => {
            let struct_info = ctx.struct_var(name).ok_or_else(|| {
                Diagnostic::not_implemented(Label::span(
                    name.span(),
                    format!("Variable '{}' is not a structure", name),
                ))
            })?;
            let (slot_offset, field_type) = field_in(types, struct_info.ty, field)?;
            Ok((name.clone(), slot_offset, field_type))
        }
        Access::Field(inner) => {
            let (root, parent_offset, parent_type) =
                walk_struct_chain(types, ctx, &inner.record, &inner.field, depth + 1)?;

            if !matches!(types.kind(parent_type), Some(TypeKind::Structure { .. })) {
                return Err(Diagnostic::not_implemented(Label::span(
                    inner.field.span(),
                    format!("Field '{}' is not a structure type", inner.field),
                )));
            }

            let (field_slot_offset, field_type) = field_in(types, parent_type, field)?;

            Ok((
                root,
                SlotIndex::new(parent_offset.raw() + field_slot_offset.raw()),
                field_type,
            ))
        }
        // An `Index` record (`arr[i].field`, `s.items[i].field`) is resolved
        // by `compile_array_struct` before the callers reach this walk, and
        // a dereference owns no field of a structure the layout reaches.
        Access::Index(_) | Access::Deref(_) => Err(Diagnostic::todo_with_span(record.span())),
    }
}

/// Reserves, in the walk of the AST declarations, the data region space, the
/// array descriptors and the scratch slot of a variable of the structure type
/// `struct_type`, and returns the descriptor of the structure as a flat array
/// of slots. The layout built from the lowered declarations records how the
/// variable is reached.
pub(crate) fn allocate_struct_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    struct_type: &SemanticType,
    id: &Id,
    span: &SourceSpan,
) -> Result<u16, Diagnostic> {
    let SemanticType::Structure { fields } = struct_type else {
        return Err(Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "Structure type did not resolve to a structure",
        )));
    };

    // Compute total slots.
    //
    // Overflowing the slot count is a fixed limit of the bytecode format, so
    // it reports P9997 (NotSupported). The other two failures are not limits:
    // an unsupported field type is a capability the compiler does not offer
    // *yet*, and exceeding the nesting depth means a recursive type reached
    // codegen that the analyzer's toposort should have rejected. Both report
    // P9999 (NotImplemented), which is the code that invites the program in.
    let total_slots = struct_type.slot_count().map_err(|e| match e {
        SlotCountError::UnsupportedFieldType => Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Structure contains unsupported field types (STRING, WSTRING, or FunctionBlock)",
        )),
        SlotCountError::MaxDepthExceeded => Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Structure exceeds maximum nesting depth (possible recursive type)",
        )),
        SlotCountError::Overflow => Diagnostic::not_supported(Label::span(
            span.clone(),
            "Structure is too large (slot count overflows u32)",
        )),
    })?;

    // Enforce slot limit (matches existing array limit for i32 flat-index safety)
    if total_slots > super::compile::MAX_DATA_REGION_SLOTS {
        return Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Structure exceeds maximum 32768 slots",
        )));
    }

    // Allocate data region space
    let total_bytes = total_slots.checked_mul(8).ok_or_else(|| {
        Diagnostic::not_supported(Label::span(
            span.clone(),
            "Structure size overflows (slots * 8)",
        ))
    })?;
    crate::data_region::reserve(ctx, total_bytes, span)?;

    // Register array descriptor (treating struct as flat slot array).
    let desc_index = builder.add_array_descriptor(FieldType::Slot as u8, total_slots, 0);

    // Build field metadata (returns error for unsupported field types)
    let fields_vec = build_struct_fields(fields, span)?;

    // Track max STRING capacity for temp buffer sizing.
    for f in &fields_vec {
        if let Some(max_len) = f.string_max_length {
            if max_len > ctx.max_string_capacity {
                ctx.max_string_capacity = max_len;
            }
        }
    }

    // Register STRING array descriptors and scratch variable for fields
    // that are ARRAY OF STRING.
    let mut scratch_var_index: Option<VarIndex> = None;
    for f in &fields_vec {
        if let SemanticType::Array {
            element_type,
            dimensions: array_dims,
        } = &f.field_type
        {
            if let SemanticType::String {
                max_len,
                char_width,
            } = element_type.as_ref()
            {
                let max_str_len = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128) as u16;
                let total_elements = array_dims
                    .iter()
                    .try_fold(1u32, |acc, d| {
                        let size = (d.upper as i64 - d.lower as i64 + 1).max(0) as u32;
                        acc.checked_mul(size)
                    })
                    .ok_or_else(|| {
                        Diagnostic::not_supported(Label::span(span.clone(), "Array too large"))
                    })?;
                // Allocate scratch variable once for all STRING/WSTRING array fields.
                if scratch_var_index.is_none() {
                    let scratch_idx = ctx.allocate_scratch_variable(&id.to_string(), span)?;
                    scratch_var_index = Some(scratch_idx);
                }
                let element_field_type = FieldType::string(*char_width);
                builder.add_array_descriptor(element_field_type as u8, total_elements, max_str_len);
                // Track max string capacity for temp buffer sizing.
                if max_str_len > ctx.max_string_capacity {
                    ctx.max_string_capacity = max_str_len;
                }
            }
        }
    }

    // Register a strided STRING descriptor for each STRING field of the
    // elements of the arrays of structures this structure holds.
    let mut element_strings = Vec::new();
    crate::compile_array_struct::register_struct_element_strings(
        ctx,
        builder,
        fields,
        0,
        span,
        &mut element_strings,
    )?;
    if !element_strings.is_empty() && scratch_var_index.is_none() {
        ctx.allocate_scratch_variable(&id.to_string(), span)?;
    }
    Ok(desc_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_analyzer::semantic_type::{ByteSized, SemanticStructField};
    use ironplc_container::CharWidth;
    use ironplc_dsl::core::Id;

    fn make_field(name: &str, field_type: SemanticType) -> SemanticStructField {
        SemanticStructField {
            name: Id::from(name),
            field_type,
            has_default: false,
        }
    }

    #[test]
    fn build_struct_fields_when_two_fields_then_sequential_offsets() {
        let fields = vec![
            make_field(
                "a",
                SemanticType::Int {
                    size: ByteSized::B32,
                },
            ),
            make_field("b", SemanticType::Bool),
        ];
        let field_list = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list.len(), 2);
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        assert_eq!(field_list[1].slot_offset, SlotIndex::new(1));
    }

    #[test]
    fn build_struct_fields_when_nested_struct_then_inner_occupies_multiple_slots() {
        let inner = SemanticType::Structure {
            fields: vec![
                make_field(
                    "x",
                    SemanticType::Int {
                        size: ByteSized::B32,
                    },
                ),
                make_field(
                    "y",
                    SemanticType::Int {
                        size: ByteSized::B32,
                    },
                ),
            ],
        };
        let fields = vec![
            make_field("inner", inner),
            make_field(
                "z",
                SemanticType::Int {
                    size: ByteSized::B32,
                },
            ),
        ];
        let field_list = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        // inner has 2 slots, so z starts at offset 2
        assert_eq!(field_list[1].slot_offset, SlotIndex::new(2));
    }

    #[test]
    fn build_struct_fields_when_string_field_then_computes_slots() {
        // STRING[255] needs ceil((4 + 255) / 8) = 33 slots
        let fields = vec![make_field(
            "s",
            SemanticType::String {
                max_len: Some(255),
                char_width: CharWidth::Narrow,
            },
        )];
        let field_list = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list.len(), 1);
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        assert_eq!(field_list[0].string_max_length, Some(255));
    }

    #[test]
    fn build_struct_fields_when_string_and_int_then_correct_offsets() {
        // STRING[30] needs ceil((4 + 30) / 8) = 5 slots, then INT at offset 5
        let fields = vec![
            make_field(
                "s",
                SemanticType::String {
                    max_len: Some(30),
                    char_width: CharWidth::Narrow,
                },
            ),
            make_field(
                "n",
                SemanticType::Int {
                    size: ByteSized::B32,
                },
            ),
        ];
        let field_list = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        assert_eq!(field_list[1].slot_offset, SlotIndex::new(5));
    }

    #[test]
    fn build_struct_fields_when_fb_field_then_returns_error() {
        let fields = vec![make_field(
            "fb",
            SemanticType::FunctionBlock {
                name: "MyFB".to_string(),
            },
        )];
        let result = build_struct_fields(&fields, &SourceSpan::default());
        assert!(result.is_err());
    }

    #[test]
    fn build_struct_fields_when_iterated_then_declaration_order_preserved() {
        let fields = vec![
            make_field("first", SemanticType::Bool),
            make_field(
                "second",
                SemanticType::Int {
                    size: ByteSized::B32,
                },
            ),
            make_field(
                "third",
                SemanticType::Real {
                    size: ByteSized::B64,
                },
            ),
        ];
        let field_list = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        let types: Vec<&SemanticType> = field_list.iter().map(|f| &f.field_type).collect();
        assert_eq!(
            types,
            fields.iter().map(|f| &f.field_type).collect::<Vec<_>>()
        );
    }

    #[test]
    fn build_struct_fields_when_duplicate_field_names_then_returns_error() {
        let fields = vec![
            make_field("x", SemanticType::Bool),
            make_field("X", SemanticType::Bool),
        ];
        let result = build_struct_fields(&fields, &SourceSpan::default());
        assert!(result.is_err());
    }
}
