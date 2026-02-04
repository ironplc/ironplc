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

        // Determine if this field has a default value
        let has_default = field_has_default(&element.init);

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
            var_type: None, // Structure fields don't have input/output distinction
            has_default,
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

/// Determines whether a structure field has a default value based on its initializer.
///
/// A field has a default if it has an explicit initial value in its type specification.
/// For nested types (structures, arrays), we consider them to have defaults if they
/// have initializers or if all their constituent parts have defaults.
fn field_has_default(init: &InitialValueAssignmentKind) -> bool {
    match init {
        InitialValueAssignmentKind::None(_) => false,
        InitialValueAssignmentKind::Simple(simple_init) => simple_init.initial_value.is_some(),
        InitialValueAssignmentKind::String(string_init) => string_init.initial_value.is_some(),
        InitialValueAssignmentKind::EnumeratedValues(enum_init) => {
            enum_init.initial_value.is_some()
        }
        InitialValueAssignmentKind::EnumeratedType(enum_type_init) => {
            enum_type_init.initial_value.is_some()
        }
        InitialValueAssignmentKind::FunctionBlock(_) => {
            // Function block fields can have their inputs initialized,
            // but function blocks themselves don't have "defaults" in the
            // same sense - they always need to be instantiated.
            // For const checking purposes, we treat them as not having defaults.
            false
        }
        InitialValueAssignmentKind::Subrange(_subrange_spec) => {
            // Subranges in structure fields don't currently preserve initial values
            // in the parser (see parser.rs line 573 - subrange.1 is discarded).
            // Conservatively treat them as not having defaults.
            false
        }
        InitialValueAssignmentKind::Structure(struct_init) => {
            // A structure field has a "default" if it has explicit initializers.
            // If elements_init is non-empty, those fields are explicitly initialized.
            // However, the complete determination of whether all fields are initialized
            // requires checking the type definition - which happens during const validation.
            // For this field-level check, we say it has a default if any explicit
            // initializers are provided.
            !struct_init.elements_init.is_empty()
        }
        InitialValueAssignmentKind::Array(array_init) => {
            // Array has a default if it has initial values (non-empty vec)
            !array_init.initial_values.is_empty()
        }
        InitialValueAssignmentKind::LateResolvedType(_) => {
            // Late resolved types don't carry initial value information
            false
        }
    }
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
        InitialValueAssignmentKind::LateResolvedType(type_name) => {
            // LateResolvedType may appear when the field references a user-defined type.
            // Since types are processed in topological order, the referenced type should
            // already be in the environment.
            let type_attrs = type_environment.get(type_name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::StructFieldTypeNotDeclared,
                    Label::span(type_name.span(), "Field type"),
                )
            })?;
            Ok(type_attrs.representation.clone())
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
        InitialValueAssignmentKind::Structure(structure_init) => {
            // Handle nested structure field types
            let type_attrs = type_environment
                .get(&structure_init.type_name)
                .ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::StructFieldTypeNotDeclared,
                        Label::span(structure_init.type_name.span(), "Structure type"),
                    )
                })?;
            Ok(type_attrs.representation.clone())
        }
        InitialValueAssignmentKind::Array(array_init) => {
            // Handle array field types
            let array_result = crate::intermediates::array::try_from(
                &TypeName::from("_field_array"),
                &array_init.spec,
                type_environment,
            )?;

            match array_result {
                crate::intermediates::array::IntermediateResult::Type(attrs) => {
                    Ok(attrs.representation)
                }
                crate::intermediates::array::IntermediateResult::Alias(base_name) => {
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
        assert!(result.is_ok(), "Expected Ok, got error: {:?}", result.err());
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

    // Nested structure tests

    #[test]
    fn parse_structure_with_nested_structure_then_creates_structure_with_nested_type() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT := 0;
    END_STRUCT;
    Rectangle : STRUCT
        topLeft : Point;
        bottomRight : Point;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let rect_type = env.get(&TypeName::from("Rectangle")).unwrap();
        assert!(rect_type.representation.is_structure());

        let fields = match &rect_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                rect_type.representation
            ),
        };

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, Id::from("topLeft"));
        assert_eq!(fields[1].name, Id::from("bottomRight"));
        assert!(fields[0].field_type.is_structure());
        assert!(fields[1].field_type.is_structure());
    }

    #[test]
    fn parse_structure_with_deeply_nested_structures_then_creates_correct_types() {
        let program = "
TYPE
    Inner : STRUCT
        value : INT := 0;
    END_STRUCT;
    Middle : STRUCT
        inner : Inner;
    END_STRUCT;
    Outer : STRUCT
        middle : Middle;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let outer_type = env.get(&TypeName::from("Outer")).unwrap();
        assert!(outer_type.representation.is_structure());

        let fields = match &outer_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                outer_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("middle"));
        assert!(fields[0].field_type.is_structure());
    }

    #[test]
    fn parse_structure_with_undeclared_nested_type_then_error() {
        let program = "
TYPE
    MyStruct : STRUCT
        nested : UndeclaredType;
    END_STRUCT;
END_TYPE
        ";
        let errors = parse_and_expect_error(program);

        assert!(!errors.is_empty());
        assert_eq!(Problem::StructFieldTypeNotDeclared.code(), errors[0].code);
    }

    #[test]
    fn parse_structure_with_nested_structure_then_calculates_correct_offsets() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT := 0;
    END_STRUCT;
    Line : STRUCT
        start : Point;
        endPt : Point;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let line_type = env.get(&TypeName::from("Line")).unwrap();
        let fields = match &line_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                line_type.representation
            ),
        };

        // Point is 4 bytes (2x INT with 2-byte alignment), so:
        // - start at offset 0
        // - endPt at offset 4
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 4);
    }

    #[test]
    fn parse_structure_with_mixed_nested_and_primitive_fields_then_correct_layout() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT := 0;
    END_STRUCT;
    Entity : STRUCT
        id : DINT := 0;
        position : Point;
        active : BOOL := FALSE;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let entity_type = env.get(&TypeName::from("Entity")).unwrap();
        let fields = match &entity_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                entity_type.representation
            ),
        };

        assert_eq!(fields.len(), 3);
        // id (DINT) at offset 0 (4 bytes)
        // position (Point) at offset 4 (4 bytes, 2-byte alignment)
        // active (BOOL) at offset 8 (1 byte)
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 4);
        assert_eq!(fields[2].offset, 8);
    }

    // Array field tests

    #[test]
    fn parse_structure_with_array_field_then_creates_structure_with_array() {
        let program = "
TYPE
    MyStruct : STRUCT
        values : ARRAY [1..10] OF INT;
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
        assert_eq!(fields[0].name, Id::from("values"));
        assert!(matches!(
            fields[0].field_type,
            IntermediateType::Array { .. }
        ));
    }

    #[test]
    fn parse_structure_with_multidimensional_array_field_then_creates_structure_with_array() {
        let program = "
TYPE
    Matrix : STRUCT
        data : ARRAY [1..3, 1..4] OF REAL;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("Matrix")).unwrap();
        assert!(struct_type.representation.is_structure());

        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("data"));
        if let IntermediateType::Array { size, .. } = &fields[0].field_type {
            assert_eq!(*size, Some(12)); // 3 * 4 = 12
        } else {
            panic!("Expected Array type");
        }
    }

    #[test]
    fn parse_structure_with_array_of_struct_field_then_creates_structure_with_nested_array() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT := 0;
    END_STRUCT;
    Polygon : STRUCT
        vertices : ARRAY [1..4] OF Point;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let polygon_type = env.get(&TypeName::from("Polygon")).unwrap();
        assert!(polygon_type.representation.is_structure());

        let fields = match &polygon_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                polygon_type.representation
            ),
        };

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, Id::from("vertices"));
        if let IntermediateType::Array { element_type, size } = &fields[0].field_type {
            assert!(element_type.is_structure());
            assert_eq!(*size, Some(4));
        } else {
            panic!("Expected Array type");
        }
    }

    #[test]
    fn parse_structure_with_mixed_array_and_primitive_fields_then_correct_types() {
        let program = "
TYPE
    DataRecord : STRUCT
        id : DINT := 0;
        values : ARRAY [1..5] OF INT;
        active : BOOL := FALSE;
    END_STRUCT;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let struct_type = env.get(&TypeName::from("DataRecord")).unwrap();
        let fields = match &struct_type.representation {
            IntermediateType::Structure { fields } => fields,
            _ => panic!(
                "Expected Structure type, got {:?}",
                struct_type.representation
            ),
        };

        assert_eq!(fields.len(), 3);
        assert!(matches!(fields[0].field_type, IntermediateType::Int { .. }));
        assert!(matches!(
            fields[1].field_type,
            IntermediateType::Array { .. }
        ));
        assert!(matches!(fields[2].field_type, IntermediateType::Bool));
    }
}
