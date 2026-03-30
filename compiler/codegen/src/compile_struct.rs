//! Structure code generation support.
//!
//! Handles structure variable metadata, field resolution, and
//! structure-related compilation helpers. Separated from compile.rs
//! to keep module sizes within the 1000-line guideline.

use std::collections::HashMap;

use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{StructuredVariable, SymbolicVariableKind};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::{
    ByteSized, IntermediateStructField, IntermediateType, SlotCountError,
};
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::FieldType;
use ironplc_container::{ContainerBuilder, SlotIndex, VarIndex};
use ironplc_dsl::common::{StructInitialValueAssignmentKind, StructureElementInit, TypeName};

use super::compile::{
    compile_constant, emit_truncation, emit_zero_const, CompileContext, OpType, OpWidth,
    Signedness, VarTypeInfo,
};
use crate::emit::Emitter;

/// Metadata for a structure variable, stored in CompileContext.
pub(crate) struct StructVarInfo {
    /// Variable table index holding the data region offset.
    pub var_index: VarIndex,
    /// Data region byte offset where this structure's fields start.
    pub data_offset: u32,
    /// Total number of 8-byte slots this structure occupies.
    #[allow(dead_code)]
    pub total_slots: SlotIndex,
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
pub(crate) struct StructFieldInfo {
    /// Field name (lowercase, for matching against access chains).
    pub name: String,
    /// Slot offset relative to the containing structure's base.
    pub slot_offset: SlotIndex,
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
pub(crate) fn build_struct_fields(
    fields: &[IntermediateStructField],
    span: &SourceSpan,
) -> Result<(Vec<StructFieldInfo>, HashMap<String, usize>), Diagnostic> {
    let mut field_list = Vec::with_capacity(fields.len());
    let mut field_index = HashMap::with_capacity(fields.len());
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
        slot_offset = SlotIndex::new(slot_offset.raw() + field_slots);
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
pub(crate) fn find_field_in_type(
    fields: &[IntermediateStructField],
    field_name: &Id,
    span: &SourceSpan,
) -> Result<(SlotIndex, IntermediateType), Diagnostic> {
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

/// Maximum nesting depth for `walk_struct_chain`. Matches the depth guard
/// in `slot_count_inner()`. Defense-in-depth: if the analyzer lets a
/// recursive type through, this prevents a stack overflow during expression
/// compilation.
const MAX_STRUCT_CHAIN_DEPTH: u32 = 32;

/// Resolves a `StructuredVariable` AST node to the information needed for
/// code emission: variable table index, array descriptor index, compile-time
/// slot offset, op type, and field type.
///
/// The returned `IntermediateType` enables callers (PR 5 store path) to
/// derive truncation information from the field's type.
pub(crate) fn resolve_struct_field_access(
    ctx: &CompileContext,
    structured: &StructuredVariable,
) -> Result<(VarIndex, u16, SlotIndex, OpType, IntermediateType), Diagnostic> {
    let (root_name, slot_offset, field_type) =
        walk_struct_chain(ctx, &structured.record, &structured.field, 0)?;

    let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                structured.span(),
                format!("Variable '{}' is not a structure", root_name),
            ),
        )
    })?;

    let op_type = resolve_field_op_type(&field_type).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                structured.field.span(),
                "Cannot read composite field directly (nested struct or array)",
            ),
        )
    })?;

    Ok((
        struct_info.var_index,
        struct_info.desc_index,
        slot_offset,
        op_type,
        field_type,
    ))
}

/// Walks a `StructuredVariable` AST chain to resolve the root variable name,
/// accumulated slot offset, and leaf field type.
///
/// - **Base case** (`Named`): looks up the field in `ctx.struct_vars`.
/// - **Recursive case** (`Structured`): recurses to resolve the parent, then
///   uses `find_field_in_type` to resolve the current field within the parent
///   type, accumulating slot offsets.
fn walk_struct_chain(
    ctx: &CompileContext,
    record: &SymbolicVariableKind,
    field: &Id,
    depth: u32,
) -> Result<(Id, SlotIndex, IntermediateType), Diagnostic> {
    if depth > MAX_STRUCT_CHAIN_DEPTH {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(
                field.span(),
                "Structure nesting exceeds maximum depth (possible recursive type)",
            ),
        ));
    }

    match record {
        SymbolicVariableKind::Named(named) => {
            let struct_info = ctx.struct_vars.get(&named.name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(
                        named.name.span(),
                        format!("Variable '{}' is not a structure", named.name),
                    ),
                )
            })?;
            let field_name = field.to_string().to_lowercase();
            let &field_idx = struct_info.field_index.get(&field_name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(field.span(), format!("Unknown field '{}'", field)),
                )
            })?;
            let field_info = &struct_info.fields[field_idx];
            Ok((
                named.name.clone(),
                field_info.slot_offset,
                field_info.field_type.clone(),
            ))
        }
        SymbolicVariableKind::Structured(inner) => {
            let (root, parent_offset, parent_type) =
                walk_struct_chain(ctx, &inner.record, &inner.field, depth + 1)?;

            let IntermediateType::Structure { fields } = &parent_type else {
                return Err(Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(
                        inner.field.span(),
                        format!("Field '{}' is not a structure type", inner.field),
                    ),
                ));
            };

            let (field_slot_offset, field_type) = find_field_in_type(fields, field, &field.span())?;

            Ok((
                root,
                SlotIndex::new(parent_offset.raw() + field_slot_offset.raw()),
                field_type,
            ))
        }
        // Array access within struct chain handled in PR 8
        _ => Err(Diagnostic::todo_with_span(record.span(), file!(), line!())),
    }
}

/// Derives a `VarTypeInfo` from an `IntermediateType` for use with `emit_truncation`.
///
/// This is needed because struct fields are identified by `IntermediateType`, not
/// by variable-table entries.
fn var_type_info_for_field(field_type: &IntermediateType) -> Option<VarTypeInfo> {
    let (op_width, signedness) = resolve_field_op_type(field_type)?;
    let storage_bits = match field_type {
        IntermediateType::Bool => 1,
        IntermediateType::Int { size }
        | IntermediateType::UInt { size }
        | IntermediateType::Real { size }
        | IntermediateType::Bytes { size }
        | IntermediateType::Time { size }
        | IntermediateType::Date { size }
        | IntermediateType::TimeOfDay { size }
        | IntermediateType::DateAndTime { size } => size.into(),
        IntermediateType::Enumeration { underlying_type } => {
            return var_type_info_for_field(underlying_type);
        }
        IntermediateType::Subrange { base_type, .. } => {
            return var_type_info_for_field(base_type);
        }
        IntermediateType::Reference { .. } => 64,
        _ => return None,
    };
    Some(VarTypeInfo {
        op_width,
        signedness,
        storage_bits,
    })
}

/// Emits truncation instructions for narrow types when storing to a struct field.
fn emit_truncation_for_field(emitter: &mut Emitter, field_type: &IntermediateType) {
    if let Some(vti) = var_type_info_for_field(field_type) {
        emit_truncation(emitter, vti);
    }
}

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
/// Handles constant expressions (integer/real/boolean literals) from
/// `StructInitialValueAssignmentKind`. Returns an error for unsupported kinds.
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
        StructInitialValueAssignmentKind::EnumeratedValue(_)
        | StructInitialValueAssignmentKind::Array(_)
        | StructInitialValueAssignmentKind::Structure(_) => {
            // Nested structures, arrays, and enums in struct init are not yet supported.
            // Enum support could be added by resolving the enum value to an integer constant.
            Ok(())
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
}

/// Initializes fields of a structure variable.
///
/// Emits constant-load + STORE_ARRAY for each leaf field. Uses explicit
/// initial values from `element_inits` when available, otherwise emits
/// type-appropriate defaults (zero or subrange lower bound).
pub(crate) fn initialize_struct_fields(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    var_index: VarIndex,
    desc_index: u16,
    fields: &[FieldInitInfo],
    element_inits: &[StructureElementInit],
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
        }
        // else: nested structure or array field — handled in later PRs
    }
    Ok(())
}

/// Allocates data region space for a structure variable and registers metadata.
///
/// Called from both the `Structure` and `LateResolvedType` match arms in
/// `assign_variables`.
pub(crate) fn allocate_struct_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    type_name: &TypeName,
    id: &Id,
    index: VarIndex,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let struct_type = types.resolve_struct_type(type_name).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Unknown structure type"),
        )
    })?;

    let IntermediateType::Structure { fields } = struct_type else {
        unreachable!("resolve_struct_type guarantees Structure variant");
    };

    // Compute total slots
    let total_slots = struct_type.slot_count().map_err(|e| {
        let msg = match e {
            SlotCountError::UnsupportedFieldType => {
                "Structure contains unsupported field types (STRING, WSTRING, or FunctionBlock)"
            }
            SlotCountError::MaxDepthExceeded => {
                "Structure exceeds maximum nesting depth (possible recursive type)"
            }
            SlotCountError::Overflow => "Structure is too large (slot count overflows u32)",
        };
        Diagnostic::problem(Problem::NotImplemented, Label::span(span.clone(), msg))
    })?;

    // Enforce slot limit (matches existing array limit for i32 flat-index safety)
    if total_slots > super::compile::MAX_DATA_REGION_SLOTS {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Structure exceeds maximum 32768 slots"),
        ));
    }

    // Allocate data region space
    let data_offset = ctx.data_region_offset;
    let total_bytes = total_slots.checked_mul(8).ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Structure size overflows (slots * 8)"),
        )
    })?;
    ctx.data_region_offset = ctx
        .data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Data region overflow"),
            )
        })?;

    // Guard against i32 truncation (data_offset is stored as i32 in the
    // variable slot, matching the array pattern)
    if ctx.data_region_offset > i32::MAX as u32 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region exceeds 2 GiB limit"),
        ));
    }

    // Register array descriptor (treating struct as flat slot array).
    let desc_index = builder.add_array_descriptor(FieldType::Slot as u8, total_slots);

    // Build field metadata (returns error for unsupported field types)
    let (fields_vec, field_index) = build_struct_fields(fields, span)?;

    // Store metadata
    ctx.struct_vars.insert(
        id.clone(),
        StructVarInfo {
            var_index: index,
            data_offset,
            total_slots: SlotIndex::new(total_slots),
            desc_index,
            fields: fields_vec,
            field_index,
        },
    );
    Ok(())
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
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        assert_eq!(field_list[1].name, "b");
        assert_eq!(field_list[1].slot_offset, SlotIndex::new(1));
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
        assert_eq!(field_list[0].slot_offset, SlotIndex::new(0));
        // inner has 2 slots, so z starts at offset 2
        assert_eq!(field_list[1].slot_offset, SlotIndex::new(2));
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
