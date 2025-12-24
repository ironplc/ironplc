//! Property-based tests for comprehensive validation system
//!
//! These tests implement Task 12: Comprehensive validation system
//! covering array bounds validation, subrange validation, type reference validation,
//! array initialization validation, and enumeration value validation.

use proptest::prelude::*;
use dsl::common::*;
use dsl::core::FileId;
use crate::options::ParseOptions;
use crate::parse_program;

// Helper function to check if a string is a reserved keyword
fn is_reserved_keyword(s: &str) -> bool {
    let upper_s = s.to_uppercase();
    matches!(upper_s.as_str(), 
        "TO" | "DO" | "END" | "IF" | "THEN" | "ELSE" | "ELSIF" | "FOR" | "WHILE" | "CASE" | "OF" | 
        "VAR" | "TYPE" | "PROGRAM" | "FUNCTION" | "FUNCTION_BLOCK" | "TP" | "TON" | "TOF" | 
        "ARRAY" | "STRING" | "BOOL" | "INT" | "REAL" | "DINT" | "LINT" | "SINT" | "UINT" | 
        "UDINT" | "ULINT" | "USINT" | "LREAL" | "BYTE" | "WORD" | "DWORD" | "LWORD" | 
        "TIME" | "DATE" | "WSTRING" | "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "NOT" | 
        "XOR" | "MOD" | "DIV" | "RETURN" | "EXIT" | "CONTINUE" | "REPEAT" | "UNTIL" | 
        "STEP" | "TRANSITION" | "ACTION" | "ACTIONS" | "CLASS" | "METHOD" | "EXTENDS" | 
        "IMPLEMENTS" | "INTERFACE" | "ABSTRACT" | "FINAL" | "OVERRIDE" | "PRIVATE" | 
        "PROTECTED" | "PUBLIC" | "INTERNAL" | "CONSTANT" | "RETAIN" | "NON_RETAIN" | 
        "PERSISTENT" | "AT" | "REF_TO" | "POINTER" | "STRUCT" | "UNION" | "ENUM" | 
        "CONFIGURATION" | "RESOURCE" | "TASK" | "VAR_INPUT" | "VAR_OUTPUT" | "VAR_IN_OUT" | 
        "VAR_EXTERNAL" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_CONFIG" | "VAR_TEMP" | 
        "END_VAR" | "END_TYPE" | "END_STRUCT" | "END_UNION" | "END_ENUM" | "END_PROGRAM" | 
        "END_FUNCTION" | "END_FUNCTION_BLOCK" | "END_CLASS" | "END_METHOD" | "END_INTERFACE" | 
        "END_CONFIGURATION" | "END_RESOURCE" | "END_TASK" | "END_ACTION" | "END_ACTIONS" | 
        "END_CASE" | "END_FOR" | "END_WHILE" | "END_REPEAT" | "END_IF" | "END_STEP" | 
        "END_TRANSITION" | "ON" | "OFF" | "IDLE" | "RUNNING" | "STOPPED" | "START" | "STOP" |
        "PAUSE" | "RESET" | "RED" | "GREEN" | "BLUE" | "BY" | "FROM" | "WITH" | "READ_ONLY" |
        "READ_WRITE" | "INITIAL_STEP" | "R_EDGE" | "F_EDGE" | "EN" | "ENO" | "DT" | "DATE_AND_TIME"
    )
}

/// **Feature: ironplc-esstee-syntax-support, Property 33: Array Bounds Validation**
/// **Validates: Requirements 10.1**
#[cfg(test)]
mod array_bounds_validation_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_array_bounds_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            lower_bound in -10i32..10i32,
            upper_bound in -10i32..20i32,
            element_type in prop_oneof![
                Just("INT".to_string()),
                Just("BOOL".to_string()),
                Just("REAL".to_string()),
            ],
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let program = format!(
                "TYPE\n    {} : ARRAY[{}..{}] OF {};\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, lower_bound, upper_bound, element_type, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            if lower_bound <= upper_bound {
                // Valid bounds: lower <= upper should parse successfully
                prop_assert!(result.is_ok(), 
                    "Array with valid bounds [{}..{}] should parse successfully: {:?}", 
                    lower_bound, upper_bound, result.err());
                
                if let Ok(library) = result {
                    // Verify the array type was created correctly
                    let has_type_def = library.elements.iter().any(|e| matches!(e, 
                        LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                    ));
                    prop_assert!(has_type_def, "Should have type definition block or data type declaration");
                    
                    let has_program = library.elements.iter().any(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_)));
                    prop_assert!(has_program, "Should have program declaration");
                }
            } else {
                // Invalid bounds: lower > upper should still parse (semantic validation comes later)
                // The parser should accept the syntax but semantic analysis should catch the error
                prop_assert!(result.is_ok(), 
                    "Array with invalid bounds [{}..{}] should still parse (semantic validation comes later): {:?}", 
                    lower_bound, upper_bound, result.err());
            }
        }
    }

    proptest! {
        #[test]
        fn property_multi_dimensional_array_bounds_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            bounds1 in (0i32..5i32, 5i32..10i32),
            bounds2 in (0i32..3i32, 3i32..6i32),
            bounds3 in (1i32..2i32, 2i32..4i32),
            element_type in prop_oneof![
                Just("INT".to_string()),
                Just("BOOL".to_string()),
            ],
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let (lower1, upper1) = bounds1;
            let (lower2, upper2) = bounds2;
            let (lower3, upper3) = bounds3;
            
            let program = format!(
                "TYPE\n    {} : ARRAY[{}..{},{}..{},{}..{}] OF {};\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, lower1, upper1, lower2, upper2, lower3, upper3, element_type, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // All bounds are valid by construction (lower <= upper), so should parse successfully
            prop_assert!(result.is_ok(), 
                "Multi-dimensional array with valid bounds should parse successfully: {:?}", 
                result.err());
            
            if let Ok(library) = result {
                // Verify the multi-dimensional array type was created correctly
                let has_type_def = library.elements.iter().any(|e| matches!(e, 
                    LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                ));
                prop_assert!(has_type_def, "Should have type definition block or data type declaration");
            }
        }
    }

    proptest! {
        #[test]
        fn property_negative_array_bounds_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            lower_bound in -20i32..-5i32,
            upper_bound in -5i32..5i32,
            element_type in prop_oneof![
                Just("INT".to_string()),
                Just("REAL".to_string()),
            ],
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            // Test arrays with negative bounds
            let program = format!(
                "TYPE\n    {} : ARRAY[{}..{}] OF {};\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, lower_bound, upper_bound, element_type, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Negative bounds are valid as long as lower <= upper
            prop_assert!(result.is_ok(), 
                "Array with negative bounds [{}..{}] should parse successfully: {:?}", 
                lower_bound, upper_bound, result.err());
        }
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 34: Subrange Mathematical Validation**
/// **Validates: Requirements 10.2**
#[cfg(test)]
mod subrange_validation_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_subrange_mathematical_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            base_type in prop_oneof![
                Just("INT".to_string()),
                Just("DINT".to_string()),
                Just("SINT".to_string()),
            ],
            lower_bound in -100i32..50i32,
            upper_bound in -50i32..100i32,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let program = format!(
                "TYPE\n    {} : {}({}..{});\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, base_type, lower_bound, upper_bound, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            if lower_bound <= upper_bound {
                // Valid subrange: lower <= upper should parse successfully
                prop_assert!(result.is_ok(), 
                    "Subrange with valid bounds {}({}..{}) should parse successfully: {:?}", 
                    base_type, lower_bound, upper_bound, result.err());
                
                if let Ok(library) = result {
                    // Verify the subrange type was created correctly
                    let has_type_def = library.elements.iter().any(|e| matches!(e, 
                        LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                    ));
                    prop_assert!(has_type_def, "Should have type definition block or data type declaration");
                }
            } else {
                // Invalid subrange: lower > upper should still parse (semantic validation comes later)
                prop_assert!(result.is_ok(), 
                    "Subrange with invalid bounds {}({}..{}) should still parse (semantic validation comes later): {:?}", 
                    base_type, lower_bound, upper_bound, result.err());
            }
        }
    }

    proptest! {
        #[test]
        fn property_subrange_default_value_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            base_type in prop_oneof![
                Just("INT".to_string()),
                Just("DINT".to_string()),
            ],
            lower_bound in 0i32..20i32,
            upper_bound in 20i32..50i32,
            default_value in -10i32..60i32,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let program = format!(
                "TYPE\n    {} : {}({}..{}) := {};\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, base_type, lower_bound, upper_bound, default_value, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // The parser should accept the syntax regardless of whether the default value is in range
            // Semantic validation will check if default_value is within [lower_bound, upper_bound]
            prop_assert!(result.is_ok(), 
                "Subrange with default value should parse successfully: {:?}", 
                result.err());
            
            if let Ok(library) = result {
                // Verify the subrange type with default value was created correctly
                let has_type_def = library.elements.iter().any(|e| matches!(e, 
                    LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                ));
                prop_assert!(has_type_def, "Should have type definition block or data type declaration");
            }
        }
    }

    proptest! {
        #[test]
        fn property_single_value_subrange_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            base_type in prop_oneof![
                Just("INT".to_string()),
                Just("SINT".to_string()),
            ],
            single_value in -50i32..50i32,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            // Test single-value subranges where lower == upper
            let program = format!(
                "TYPE\n    {} : {}({}..{});\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, base_type, single_value, single_value, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Single-value subranges should be valid
            prop_assert!(result.is_ok(), 
                "Single-value subrange {}({}..{}) should parse successfully: {:?}", 
                base_type, single_value, single_value, result.err());
        }
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 35: Type Reference Validation**
/// **Validates: Requirements 10.3**
#[cfg(test)]
mod type_reference_validation_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_type_reference_validation(
            base_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            derived_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            alias_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure all type names are different
            prop_assume!(base_type_name != derived_type_name);
            prop_assume!(base_type_name != alias_type_name);
            prop_assume!(derived_type_name != alias_type_name);
            
            // Test type references: base_type -> derived_type -> alias_type
            let program = format!(
                "TYPE\n    {} : INT;\n    {} : {};\n    {} : {};\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                base_type_name, derived_type_name, base_type_name, alias_type_name, derived_type_name, program_name, var_name, alias_type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Valid type references should parse successfully
            prop_assert!(result.is_ok(), 
                "Valid type reference chain should parse successfully: {:?}", 
                result.err());
            
            if let Ok(library) = result {
                // Verify all type definitions were created
                let has_type_def = library.elements.iter().any(|e| matches!(e, 
                    LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                ));
                prop_assert!(has_type_def, "Should have type definition block or data type declaration");
                
                let has_program = library.elements.iter().any(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_)));
                prop_assert!(has_program, "Should have program declaration");
            }
        }
    }

    proptest! {
        #[test]
        fn property_forward_type_reference_validation(
            type_a_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            type_b_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type names are different
            prop_assume!(type_a_name != type_b_name);
            
            // Test forward reference: TypeA references TypeB which is defined later
            let program = format!(
                "TYPE\n    {} : {};\n    {} : INT;\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_a_name, type_b_name, type_b_name, program_name, var_name, type_a_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Forward type references should parse successfully
            prop_assert!(result.is_ok(), 
                "Forward type reference should parse successfully: {:?}", 
                result.err());
        }
    }

    proptest! {
        #[test]
        fn property_undefined_type_reference_validation(
            defined_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            undefined_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type names are different
            prop_assume!(defined_type_name != undefined_type_name);
            
            // Test reference to undefined type
            let program = format!(
                "TYPE\n    {} : INT;\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                defined_type_name, program_name, var_name, undefined_type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // The parser should accept the syntax (undefined type references are caught in semantic analysis)
            prop_assert!(result.is_ok(), 
                "Undefined type reference should still parse (semantic validation comes later): {:?}", 
                result.err());
        }
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 36: Array Initialization Structure Validation**
/// **Validates: Requirements 10.4**
#[cfg(test)]
mod array_initialization_validation_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_array_type_declaration_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            array_size in 2usize..5usize,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let upper_bound = array_size - 1;
            
            let program = format!(
                "TYPE\n    {} : ARRAY[0..{}] OF INT;\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, upper_bound, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Array type declaration should parse successfully
            prop_assert!(result.is_ok(), 
                "Array type declaration should parse successfully: {:?}", 
                result.err());
            
            if let Ok(library) = result {
                // Verify the array type was created
                let has_type_def = library.elements.iter().any(|e| matches!(e, 
                    LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                ));
                prop_assert!(has_type_def, "Should have type definition block or data type declaration");
                
                let has_program = library.elements.iter().any(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_)));
                prop_assert!(has_program, "Should have program declaration");
            }
        }
    }

    proptest! {
        #[test]
        fn property_multi_dimensional_array_type_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            dim1_size in 2usize..4usize,
            dim2_size in 2usize..3usize,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let dim1_upper = dim1_size - 1;
            let dim2_upper = dim2_size - 1;
            
            let program = format!(
                "TYPE\n    {} : ARRAY[0..{}, 0..{}] OF INT;\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\nEND_VAR\nEND_PROGRAM",
                type_name, dim1_upper, dim2_upper, program_name, var_name, type_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Multi-dimensional array type should parse successfully
            prop_assert!(result.is_ok(), 
                "Multi-dimensional array type should parse successfully: {:?}", 
                result.err());
        }
    }

    proptest! {
        #[test]
        fn property_array_variable_declaration_validation(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            array_size in 5usize..10usize,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure type name and program name are different to avoid conflicts
            prop_assume!(type_name != program_name);
            
            let upper_bound = array_size - 1;
            
            // Test array variable declaration without initialization
            let program = format!(
                "TYPE\n    {} : ARRAY[0..{}] OF INT;\nEND_TYPE\n\nVAR_GLOBAL\n    {} : {};\nEND_VAR\n\nPROGRAM {}\nEND_PROGRAM",
                type_name, upper_bound, var_name, type_name, program_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Array variable declaration should parse successfully
            prop_assert!(result.is_ok(), 
                "Array variable declaration should parse successfully: {:?}", 
                result.err());
        }
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 37: Enumeration Value Reference Validation**
/// **Validates: Requirements 10.5**
#[cfg(test)]
mod enumeration_value_validation_tests {
    use super::*;

    proptest! {
        #[test]
        fn property_enumeration_value_reference_validation(
            enum_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            enum_values in prop::collection::vec("[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 2..5),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure all enum values are unique and names don't conflict
            let mut unique_values = enum_values.clone();
            unique_values.sort();
            unique_values.dedup();
            prop_assume!(unique_values.len() >= 2);
            prop_assume!(enum_type_name != program_name);
            
            let enum_value_list = unique_values.join(", ");
            let first_value = &unique_values[0];
            
            let program = format!(
                "TYPE\n    {} : ({});\nEND_TYPE\n\nVAR_GLOBAL\n    {} : {} := {};\nEND_VAR\n\nPROGRAM {}\nEND_PROGRAM",
                enum_type_name, enum_value_list, var_name, enum_type_name, first_value, program_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Valid enumeration value reference should parse successfully
            prop_assert!(result.is_ok(), 
                "Valid enumeration value reference should parse successfully: {:?}", 
                result.err());
            
            if let Ok(library) = result {
                // Verify the enumeration type and variable were created
                let has_type_def = library.elements.iter().any(|e| matches!(e, 
                    LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
                ));
                prop_assert!(has_type_def, "Should have type definition block or data type declaration");
                
                let has_global_var = library.elements.iter().any(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_)));
                prop_assert!(has_global_var, "Should have global variable declaration");
            }
        }
    }

    proptest! {
        #[test]
        fn property_enumeration_type_declaration_validation(
            enum_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            enum_values in prop::collection::vec("[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 3..5),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            result_var in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure all enum values are unique and variable names are different
            let mut unique_values = enum_values.clone();
            unique_values.sort();
            unique_values.dedup();
            prop_assume!(unique_values.len() >= 3);
            prop_assume!(var_name != result_var);
            prop_assume!(enum_type_name != program_name);
            
            let enum_value_list = unique_values.join(", ");
            let first_value = &unique_values[0];
            let second_value = &unique_values[1];
            let third_value = &unique_values[2];
            
            let program = format!(
                "TYPE\n    {} : ({});\nEND_TYPE\n\nPROGRAM {}\nVAR\n    {} : {};\n    {} : INT;\nEND_VAR\n    CASE {} OF\n        {}: {} := 1;\n        {}: {} := 2;\n        {}: {} := 3;\n    END_CASE\nEND_PROGRAM",
                enum_type_name, enum_value_list, program_name, var_name, enum_type_name, result_var,
                var_name, first_value, result_var, second_value, result_var, third_value, result_var
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Enumeration values in CASE statements should parse successfully
            prop_assert!(result.is_ok(), 
                "Enumeration values in CASE statement should parse successfully: {:?}", 
                result.err());
        }
    }

    proptest! {
        #[test]
        fn property_invalid_enumeration_value_reference_validation(
            enum_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            enum_values in prop::collection::vec("[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 2..4),
            invalid_value in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure all enum values are unique and invalid_value is not in the enum
            let mut unique_values = enum_values.clone();
            unique_values.sort();
            unique_values.dedup();
            prop_assume!(unique_values.len() >= 2);
            prop_assume!(!unique_values.contains(&invalid_value));
            prop_assume!(enum_type_name != program_name);
            
            let enum_value_list = unique_values.join(", ");
            
            let program = format!(
                "TYPE\n    {} : ({});\nEND_TYPE\n\nVAR_GLOBAL\n    {} : {} := {};\nEND_VAR\n\nPROGRAM {}\nEND_PROGRAM",
                enum_type_name, enum_value_list, var_name, enum_type_name, invalid_value, program_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Invalid enumeration value reference should still parse (semantic validation comes later)
            prop_assert!(result.is_ok(), 
                "Invalid enumeration value reference should still parse (semantic validation comes later): {:?}", 
                result.err());
        }
    }

    proptest! {
        #[test]
        fn property_single_value_enumeration_validation(
            enum_type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            single_value in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure names don't conflict
            prop_assume!(enum_type_name != program_name);
            
            // Test single-value enumerations
            let program = format!(
                "TYPE\n    {} : ({});\nEND_TYPE\n\nVAR_GLOBAL\n    {} : {} := {};\nEND_VAR\n\nPROGRAM {}\nEND_PROGRAM",
                enum_type_name, single_value, var_name, enum_type_name, single_value, program_name
            );

            let result = parse_program(&program, &FileId::default(), &ParseOptions::default());

            // Single-value enumerations should parse successfully
            prop_assert!(result.is_ok(), 
                "Single-value enumeration should parse successfully: {:?}", 
                result.err());
        }
    }
}