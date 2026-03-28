//! Structure code generation support.
//!
//! Handles structure variable metadata, field resolution, and
//! structure-related compilation helpers. Separated from compile.rs
//! to keep module sizes within the 1000-line guideline.

use std::collections::HashMap;

use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::{
    ByteSized, IntermediateStructField, IntermediateType, SlotCountError,
};

use super::compile::{OpType, OpWidth, Signedness};

/// Metadata for a structure variable, stored in CompileContext.
#[allow(dead_code)]
pub(crate) struct StructVarInfo {
    /// Variable table index holding the data region offset.
    pub var_index: u16,
    /// Data region byte offset where this structure's fields start.
    pub data_offset: u32,
    /// Total number of 8-byte slots this structure occupies.
    pub total_slots: u32,
    /// Array descriptor index for this structure (treats struct as flat slot array).
    pub desc_index: u16,
    /// Fields in declaration order. Preserving order ensures deterministic
    /// bytecode emission (reproducible builds) and predictable initialization
    /// sequences. Use `field_index` for O(1) lookup by name.
    pub fields: Vec<StructFieldInfo>,
    /// Maps field name (lowercase) to index in `fields` Vec for O(1) lookup.
    pub field_index: HashMap<String, usize>,
}

/// Metadata for a single structure field.
#[allow(dead_code)]
pub(crate) struct StructFieldInfo {
    /// Field name (lowercase, for matching against access chains).
    pub name: String,
    /// Slot offset relative to the containing structure's base.
    pub slot_offset: u32,
    /// The field's intermediate type (for nested resolution).
    pub field_type: IntermediateType,
    /// Op type for leaf (primitive/enum) fields. `None` for structure/array
    /// fields (which are accessed via further resolution).
    pub op_type: Option<OpType>,
}

/// Maps an IntermediateType to its OpType for leaf fields.
///
/// Returns `Some((OpWidth, Signedness))` for primitive, enum, and subrange types.
/// Returns `None` for structure, array, and other composite types (which are
/// accessed via further resolution, not loaded/stored directly as single values).
#[allow(dead_code)]
pub(crate) fn resolve_field_op_type(field_type: &IntermediateType) -> Option<OpType> {
    match field_type {
        IntermediateType::Bool => Some((OpWidth::W32, Signedness::Signed)),
        IntermediateType::Int { size } | IntermediateType::Time { size } => match size {
            ByteSized::B8 | ByteSized::B16 | ByteSized::B32 => {
                Some((OpWidth::W32, Signedness::Signed))
            }
            ByteSized::B64 => Some((OpWidth::W64, Signedness::Signed)),
        },
        IntermediateType::UInt { size }
        | IntermediateType::Bytes { size }
        | IntermediateType::Date { size }
        | IntermediateType::TimeOfDay { size }
        | IntermediateType::DateAndTime { size } => match size {
            ByteSized::B8 | ByteSized::B16 | ByteSized::B32 => {
                Some((OpWidth::W32, Signedness::Unsigned))
            }
            ByteSized::B64 => Some((OpWidth::W64, Signedness::Unsigned)),
        },
        IntermediateType::Real { size } => match size {
            ByteSized::B32 => Some((OpWidth::F32, Signedness::Signed)),
            ByteSized::B64 => Some((OpWidth::F64, Signedness::Signed)),
            _ => Some((OpWidth::F32, Signedness::Signed)),
        },
        IntermediateType::Enumeration { underlying_type } => resolve_field_op_type(underlying_type),
        IntermediateType::Subrange { base_type, .. } => resolve_field_op_type(base_type),
        IntermediateType::Reference { .. } => Some((OpWidth::W64, Signedness::Unsigned)),
        // Composite types are not loaded/stored as single values
        IntermediateType::Structure { .. }
        | IntermediateType::Array { .. }
        | IntermediateType::String { .. }
        | IntermediateType::FunctionBlock { .. }
        | IntermediateType::Function { .. } => None,
    }
}

/// Builds field metadata (ordered Vec + lookup HashMap) from a structure's
/// intermediate type.
///
/// Returns `Err` if any field has an unsupported type (STRING, WSTRING,
/// FunctionBlock). Nested structures are NOT flattened — each level is a
/// separate field list.
#[allow(dead_code)]
pub(crate) fn build_struct_fields(
    fields: &[IntermediateStructField],
    span: &SourceSpan,
) -> Result<(Vec<StructFieldInfo>, HashMap<String, usize>), Diagnostic> {
    let mut field_list = Vec::with_capacity(fields.len());
    let mut field_index = HashMap::with_capacity(fields.len());
    let mut slot_offset = 0u32;
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
            Diagnostic::problem(Problem::NotImplemented, Label::span(span.clone(), msg))
        })?;
        let name = field.name.to_string().to_lowercase();
        let op_type = resolve_field_op_type(&field.field_type);

        // Defense-in-depth: detect duplicate field names (case-insensitive).
        // The analyzer should reject these, but if one slips through, silently
        // overwriting the HashMap entry would make the first field inaccessible
        // by name while it still occupies slots in the layout.
        if field_index.contains_key(&name) {
            return Err(Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(
                    span.clone(),
                    format!(
                        "Structure has duplicate field name '{}' (case-insensitive)",
                        name
                    ),
                ),
            ));
        }
        field_index.insert(name.clone(), field_list.len());
        field_list.push(StructFieldInfo {
            name,
            slot_offset,
            field_type: field.field_type.clone(),
            op_type,
        });
        slot_offset += field_slots;
    }
    Ok((field_list, field_index))
}

/// Looks up a field by name within an IntermediateType::Structure's field list.
///
/// Delegates to `build_struct_fields` to compute slot offsets, ensuring a single
/// source of truth for offset computation. This prevents divergence between
/// the offsets computed during variable allocation and those computed during
/// expression compilation. Both paths use `build_struct_fields` → `slot_count()`
/// for offset accumulation.
///
/// Returns `(slot_offset, field_type)` for the named field, or an error if the
/// field is not found.
#[allow(dead_code)]
pub(crate) fn find_field_in_type(
    fields: &[IntermediateStructField],
    field_name: &Id,
    span: &SourceSpan,
) -> Result<(u32, IntermediateType), Diagnostic> {
    // Reuse build_struct_fields to compute offsets — do NOT duplicate the
    // slot-offset accumulation logic. The cost of building the full field
    // list per lookup is acceptable at compile time (structures are small).
    let (field_list, field_index) = build_struct_fields(fields, span)?;
    let name_lower = field_name.to_string().to_lowercase();
    let &idx = field_index.get(&name_lower).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), format!("Unknown field '{}'", field_name)),
        )
    })?;
    let info = &field_list[idx];
    Ok((info.slot_offset, info.field_type.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_analyzer::intermediate_type::IntermediateStructField;
    use ironplc_dsl::core::Id;

    fn make_field(name: &str, field_type: IntermediateType) -> IntermediateStructField {
        IntermediateStructField {
            name: Id::from(name),
            field_type,
            offset: 0,
            var_type: None,
            has_default: false,
        }
    }

    #[test]
    fn build_struct_fields_when_two_fields_then_sequential_offsets() {
        let fields = vec![
            make_field(
                "a",
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
            make_field("b", IntermediateType::Bool),
        ];
        let (field_list, field_index) =
            build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list.len(), 2);
        assert_eq!(field_list[0].name, "a");
        assert_eq!(field_list[0].slot_offset, 0);
        assert_eq!(field_list[1].name, "b");
        assert_eq!(field_list[1].slot_offset, 1);
        assert_eq!(field_index["a"], 0);
        assert_eq!(field_index["b"], 1);
    }

    #[test]
    fn build_struct_fields_when_nested_struct_then_inner_occupies_multiple_slots() {
        let inner = IntermediateType::Structure {
            fields: vec![
                make_field(
                    "x",
                    IntermediateType::Int {
                        size: ByteSized::B32,
                    },
                ),
                make_field(
                    "y",
                    IntermediateType::Int {
                        size: ByteSized::B32,
                    },
                ),
            ],
        };
        let fields = vec![
            make_field("inner", inner),
            make_field(
                "z",
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
        ];
        let (field_list, _) = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        assert_eq!(field_list[0].slot_offset, 0);
        // inner has 2 slots, so z starts at offset 2
        assert_eq!(field_list[1].slot_offset, 2);
    }

    #[test]
    fn build_struct_fields_when_string_field_then_returns_error() {
        let fields = vec![make_field(
            "s",
            IntermediateType::String { max_len: Some(255) },
        )];
        let result = build_struct_fields(&fields, &SourceSpan::default());
        assert!(result.is_err());
    }

    #[test]
    fn build_struct_fields_when_fb_field_then_returns_error() {
        let fields = vec![make_field(
            "fb",
            IntermediateType::FunctionBlock {
                name: "MyFB".to_string(),
                fields: vec![],
            },
        )];
        let result = build_struct_fields(&fields, &SourceSpan::default());
        assert!(result.is_err());
    }

    #[test]
    fn build_struct_fields_when_iterated_then_declaration_order_preserved() {
        let fields = vec![
            make_field("first", IntermediateType::Bool),
            make_field(
                "second",
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
            make_field(
                "third",
                IntermediateType::Real {
                    size: ByteSized::B64,
                },
            ),
        ];
        let (field_list, _) = build_struct_fields(&fields, &SourceSpan::default()).unwrap();
        let names: Vec<&str> = field_list.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["first", "second", "third"]);
    }

    #[test]
    fn build_struct_fields_when_duplicate_field_names_then_returns_error() {
        let fields = vec![
            make_field("x", IntermediateType::Bool),
            make_field("X", IntermediateType::Bool),
        ];
        let result = build_struct_fields(&fields, &SourceSpan::default());
        assert!(result.is_err());
    }

    #[test]
    fn find_field_in_type_when_valid_field_then_matches_build_struct_fields_offset() {
        let fields = vec![
            make_field(
                "a",
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
            make_field("b", IntermediateType::Bool),
            make_field(
                "c",
                IntermediateType::Real {
                    size: ByteSized::B64,
                },
            ),
        ];
        let span = SourceSpan::default();

        // Cross-check: find_field_in_type returns the same offsets as
        // iterating build_struct_fields directly.
        let (field_list, _) = build_struct_fields(&fields, &span).unwrap();

        let (offset_b, _) = find_field_in_type(&fields, &Id::from("b"), &span).unwrap();
        assert_eq!(offset_b, field_list[1].slot_offset);

        let (offset_c, _) = find_field_in_type(&fields, &Id::from("c"), &span).unwrap();
        assert_eq!(offset_c, field_list[2].slot_offset);

        // Unknown field returns error
        let result = find_field_in_type(&fields, &Id::from("unknown"), &span);
        assert!(result.is_err());
    }
}
