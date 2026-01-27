//! Structure type processing
//!
//! This module handles creating structure types from structure declarations,
//! including field validation, offset calculation, and memory layout.

use crate::intermediate_type::{IntermediateStructField, IntermediateType};
use crate::intermediates::enumeration::try_from_values;
use crate::intermediates::subrange::IntermediateResult;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Try to create the intermediate type information from the structure specification.
pub fn try_from(
    node_name: &TypeName,
    spec: &StructureDeclaration,
    type_environment: &TypeEnvironment,
) -> Result<TypeAttributes, Diagnostic> {
    // Note: Field name uniqueness is validated by semantic rules, not here

    // Resolve field types and calculate offsets
    let mut fields = Vec::new();
    let mut current_offset = 0u32;

    for element in &spec.elements {
        // Resolve the field type from the initial value assignment
        let field_type = resolve_field_type(&element.init, type_environment)?;

        // Calculate field alignment and adjust offset if needed
        let field_alignment = field_type.alignment_bytes() as u32;
        let aligned_offset = align_offset(current_offset, field_alignment);
        // Use 0 for unknown sizes - the structure's overall size_in_bytes() will return None
        let field_size = field_type.size_in_bytes().unwrap_or(0) as u32;

        // Create the field
        let field = IntermediateStructField {
            name: element.name.clone(),
            field_type,
            offset: aligned_offset,
        };

        fields.push(field);

        // Update offset for next field
        current_offset = aligned_offset + field_size;
    }

    Ok(TypeAttributes::new(
        node_name.span(),
        IntermediateType::Structure { fields },
    ))
}

/// Aligns an offset to the specified alignment boundary
fn align_offset(offset: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return offset;
    }
    offset.div_ceil(alignment) * alignment
}

/// Resolves the field type from an initial value assignment
fn resolve_field_type(
    init: &InitialValueAssignmentKind,
    type_environment: &TypeEnvironment,
) -> Result<IntermediateType, Diagnostic> {
    match init {
        InitialValueAssignmentKind::Simple(simple_init) => {
            // Handle simple field types like BOOL, INT, etc.
            let type_attrs = type_environment
                .get(&simple_init.type_name)
                .ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::StructFieldTypeNotDeclared,
                        Label::span(simple_init.type_name.span(), "Field type"),
                    )
                })?;
            Ok(type_attrs.representation.clone())
        }
        InitialValueAssignmentKind::LateResolvedType(_type_name) => {
            // LateResolvedType should have been resolved by xform_resolve_late_bound_type_initializer
            // before structure processing runs. If we see this, it's a compiler bug.
            Err(Diagnostic::internal_error(file!(), line!()))
        }
        InitialValueAssignmentKind::Subrange(subrange_spec) => {
            // Handle subrange field types
            // TODO: Replace magic string with proper type-safe approach for anonymous subranges
            // Consider: Option<&TypeName> or dedicated enum for synthetic/anonymous type names
            // Current approach relies on underscore prefix convention which isn't type-safe
            let subrange_result = crate::intermediates::subrange::try_from(
                &TypeName::from("_field_subrange"),
                subrange_spec,
                type_environment,
            )?;

            match subrange_result {
                IntermediateResult::Type(attrs) => Ok(attrs.representation),
                IntermediateResult::Alias(base_name) => {
                    let base_attrs = type_environment.get(&base_name).ok_or_else(|| {
                        Diagnostic::problem(
                            Problem::StructFieldTypeNotDeclared,
                            Label::span(base_name.span(), "Base type"),
                        )
                    })?;
                    Ok(base_attrs.representation.clone())
                }
            }
        }
        InitialValueAssignmentKind::EnumeratedValues(values) => {
            // Handle enumerated field types with values
            let enum_attrs = try_from_values(values)?;
            Ok(enum_attrs.representation)
        }
        InitialValueAssignmentKind::EnumeratedType(enum_assignment) => {
            // Handle enumerated field types with type reference
            let type_attrs = type_environment
                .get(&enum_assignment.type_name)
                .ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::StructFieldTypeNotDeclared,
                        Label::span(enum_assignment.type_name.span(), "Enumeration type"),
                    )
                })?;
            Ok(type_attrs.representation.clone())
        }
        InitialValueAssignmentKind::Structure(_structure_init) => {
            // Nested structures are not yet supported
            Err(Diagnostic::problem(
                Problem::NestedStructuresNotSupported,
                Label::span(ironplc_dsl::core::SourceSpan::default(), "Nested structure"),
            ))
        }
        InitialValueAssignmentKind::Array(_array_init) => {
            // Array fields are not yet supported
            Err(Diagnostic::todo(file!(), line!()))
        }
        _other => {
            // Other types are not yet supported
            Err(Diagnostic::todo(file!(), line!()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::IntermediateType;
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};
    use crate::xform_resolve_type_decl_environment::apply;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::{FileId, Id};
    use ironplc_parser::options::ParseOptions;

    /// Helper function to parse a program and apply type resolution
    /// Returns the type environment with resolved types
    fn parse_and_apply(program: &str) -> TypeEnvironment {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);
        assert!(result.is_ok());
        env
    }

    /// Helper function to parse a program and expect an error
    /// Returns the error diagnostics
    fn parse_and_expect_error(program: &str) -> Vec<ironplc_dsl::diagnostic::Diagnostic> {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);
        assert!(result.is_err());
        result.unwrap_err()
    }

    #[test]
    fn align_offset_with_various_alignments_then_aligns_correctly() {
        // Test alignment to 1-byte boundary (no change)
        assert_eq!(align_offset(0, 1), 0);
        assert_eq!(align_offset(1, 1), 1);
        assert_eq!(align_offset(5, 1), 5);

        // Test alignment to 2-byte boundary
        assert_eq!(align_offset(0, 2), 0);
        assert_eq!(align_offset(1, 2), 2);
        assert_eq!(align_offset(2, 2), 2);
        assert_eq!(align_offset(3, 2), 4);

        // Test alignment to 4-byte boundary
        assert_eq!(align_offset(0, 4), 0);
        assert_eq!(align_offset(1, 4), 4);
        assert_eq!(align_offset(4, 4), 4);
        assert_eq!(align_offset(5, 4), 8);

        // Test alignment to 8-byte boundary
        assert_eq!(align_offset(0, 8), 0);
        assert_eq!(align_offset(1, 8), 8);
        assert_eq!(align_offset(8, 8), 8);
        assert_eq!(align_offset(9, 8), 16);
    }

    #[test]
    fn align_offset_with_zero_alignment_then_returns_original() {
        assert_eq!(align_offset(0, 0), 0);
        assert_eq!(align_offset(5, 0), 5);
        assert_eq!(align_offset(100, 0), 100);
    }

    // Parser-based tests (more readable and maintainable)

    #[test]
    fn parse_simple_structure_then_creates_structure_type() {
        let program = "
TYPE
    MyStruct : STRUCT
        x : INT := 0;
        y : BOOL := FALSE;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        // Check that the structure type was created
        let struct_type = env.get(&TypeName::from("MyStruct")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, Id::from("x"));
        assert_eq!(fields[1].name, Id::from("y"));

        // Check field types
        assert!(matches!(fields[0].field_type, IntermediateType::Int { .. }));
        assert!(matches!(fields[1].field_type, IntermediateType::Bool));

        // Check field offsets (INT at 0, BOOL at 2 due to alignment)
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 2);
    }

    #[test]
    fn parse_structure_with_subrange_field_then_creates_structure_with_subrange() {
        let program = "
TYPE
    MyStruct : STRUCT
        range_field : INT (1..100) := 50;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("MyStruct")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("range_field"));
        assert!(matches!(
            fields[0].field_type,
            IntermediateType::Subrange { .. }
        ));
    }

    #[test]
    fn parse_structure_with_enumerated_values_field_then_creates_structure_with_enum() {
        let program = "
TYPE
    MyStruct : STRUCT
        color_field : (RED, GREEN, BLUE) := RED;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("MyStruct")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("color_field"));
        assert!(matches!(
            fields[0].field_type,
            IntermediateType::Enumeration { .. }
        ));
    }

    #[test]
    fn parse_structure_with_enumerated_type_field_then_creates_structure_with_enum_ref() {
        let program = "
TYPE
    ColorEnum : (RED, GREEN, BLUE) := RED;
    MyStruct : STRUCT
        color_field : ColorEnum := RED;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("MyStruct")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("color_field"));
        assert!(matches!(
            fields[0].field_type,
            IntermediateType::Enumeration { .. }
        ));
    }

    #[test]
    fn parse_structure_with_custom_type_field_then_creates_structure_with_custom_type() {
        let program = "
TYPE
    CustomType : INT := 0;
    MyStruct : STRUCT
        custom_field : CustomType := 42;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("MyStruct")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("custom_field"));
        assert!(matches!(fields[0].field_type, IntermediateType::Int { .. }));
    }

    #[test]
    fn parse_structure_with_missing_field_type_then_error() {
        let program = "
TYPE
    MyStruct : STRUCT
        x : MISSING_TYPE := 0;
    END_STRUCT;
END_TYPE
        ";
        let errors = parse_and_expect_error(program);

        assert!(!errors.is_empty());
        assert_eq!(Problem::StructFieldTypeNotDeclared.code(), errors[0].code);
    }
}
