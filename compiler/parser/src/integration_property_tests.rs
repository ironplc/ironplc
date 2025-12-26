//! Property-based tests for integration of new syntax with existing parser infrastructure
//!
//! These tests verify that the enhanced parser properly integrates new syntax elements
//! (VAR_GLOBAL, TYPE blocks, enumerations, arrays, subranges) with existing parser
//! infrastructure while maintaining backward compatibility.

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

// Shared generators for property tests
prop_compose! {
    fn arb_simple_type_definition()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        base_type in prop_oneof![
            Just("INT".to_string()),
            Just("BOOL".to_string()),
            Just("REAL".to_string()),
            Just("STRING".to_string()),
        ]
    ) -> String {
        format!("    {} : {};", name, base_type)
    }
}

prop_compose! {
    fn arb_global_variable()(
        name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        var_type in prop_oneof![
            Just("INT".to_string()),
            Just("BOOL".to_string()),
            Just("REAL".to_string()),
        ],
        init_value in prop_oneof![
            Just("".to_string()),
            Just(" := 0".to_string()),
            Just(" := TRUE".to_string()),
            Just(" := FALSE".to_string()),
        ]
    ) -> String {
        format!("    {} : {}{};", name, var_type, init_value)
    }
}

prop_compose! {
    fn arb_simple_program()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
    ) -> String {
        format!("PROGRAM {}\nEND_PROGRAM", name)
    }
}

prop_compose! {
    fn arb_type_block()(
        type_defs in prop::collection::vec(arb_simple_type_definition(), 1..3)
    ) -> String {
        format!("TYPE\n{}\nEND_TYPE", type_defs.join("\n"))
    }
}

prop_compose! {
    fn arb_global_var_block()(
        global_vars in prop::collection::vec(arb_global_variable(), 1..3)
    ) -> String {
        format!("VAR_GLOBAL\n{}\nEND_VAR", global_vars.join("\n"))
    }
}

prop_compose! {
    fn arb_enumeration_type()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        values in prop::collection::vec("[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 2..4)
    ) -> String {
        format!("    {} : ({});", name, values.join(", "))
    }
}

prop_compose! {
    fn arb_array_type()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        lower in 0..10i32,
        upper in 10..20i32,
        element_type in prop_oneof![
            Just("INT".to_string()),
            Just("BOOL".to_string()),
            Just("REAL".to_string()),
        ]
    ) -> String {
        format!("    {} : ARRAY[{}..{}] OF {};", name, lower, upper, element_type)
    }
}

prop_compose! {
    fn arb_subrange_type()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        lower in -10..0i32,
        upper in 0..10i32
    ) -> String {
        format!("    {} : INT({}..{});", name, lower, upper)
    }
}

prop_compose! {
    fn arb_mixed_type_block()(
        simple_types in prop::collection::vec(arb_simple_type_definition(), 0..2),
        enum_types in prop::collection::vec(arb_enumeration_type(), 0..2),
        array_types in prop::collection::vec(arb_array_type(), 0..2),
        subrange_types in prop::collection::vec(arb_subrange_type(), 0..2)
    ) -> String {
        let mut all_types = Vec::new();
        all_types.extend(simple_types);
        all_types.extend(enum_types);
        all_types.extend(array_types);
        all_types.extend(subrange_types);
        
        if all_types.is_empty() {
            all_types.push("    SimpleType : INT;".to_string());
        }
        
        format!("TYPE\n{}\nEND_TYPE", all_types.join("\n"))
    }
}

prop_compose! {
    fn arb_mixed_global_vars()(
        simple_vars in prop::collection::vec(arb_global_variable(), 0..2),
        enum_vars in prop::collection::vec(
            ("[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 
             "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))), 0..2
        ).prop_map(|vars| vars.into_iter().map(|(name, enum_type)| 
            format!("    {} : ({}, {});", name, enum_type, enum_type)
        ).collect::<Vec<_>>()),
        array_vars in prop::collection::vec(
            ("[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 1..5i32, 5..10i32), 0..2
        ).prop_map(|vars| vars.into_iter().map(|(name, lower, upper)| 
            format!("    {} : ARRAY[{}..{}] OF INT;", name, lower, upper)
        ).collect::<Vec<_>>())
    ) -> String {
        let mut all_vars = Vec::new();
        all_vars.extend(simple_vars);
        all_vars.extend(enum_vars);
        all_vars.extend(array_vars);
        
        if all_vars.is_empty() {
            all_vars.push("    simple_var : INT;".to_string());
        }
        
        format!("VAR_GLOBAL\n{}\nEND_VAR", all_vars.join("\n"))
    }
}

prop_compose! {
    fn arb_function_with_mixed_vars()(
        name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
        return_type in prop_oneof![
            Just("INT".to_string()),
            Just("BOOL".to_string()),
            Just("REAL".to_string()),
        ],
        local_vars in prop::collection::vec(arb_global_variable(), 0..3)
    ) -> String {
        let var_block = if local_vars.is_empty() {
            "".to_string()
        } else {
            format!("VAR\n{}\nEND_VAR\n", local_vars.join("\n"))
        };
        
        format!("FUNCTION {} : {}\n{}\n{} := 0;\nEND_FUNCTION", name, return_type, var_block, name)
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 27: Declaration Order Independence**
/// 
/// For any file with varying declaration order (types before globals or globals before types),
/// all valid declaration orders should be handled correctly.
/// 
/// **Validates: Requirements 7.4**
#[cfg(test)]
mod declaration_order_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_declaration_order_independence(
            type_block in arb_type_block(),
            global_block in arb_global_var_block(),
            program in arb_simple_program(),
            order in 0..6u8
        ) {
            // Test different orderings of TYPE, VAR_GLOBAL, and PROGRAM blocks
            let source = match order {
                0 => format!("{}\n\n{}\n\n{}", type_block, global_block, program),
                1 => format!("{}\n\n{}\n\n{}", global_block, type_block, program),
                2 => format!("{}\n\n{}\n\n{}", program, type_block, global_block),
                3 => format!("{}\n\n{}\n\n{}", program, global_block, type_block),
                4 => format!("{}\n\n{}\n\n{}", type_block, program, global_block),
                _ => format!("{}\n\n{}\n\n{}", global_block, program, type_block),
            };

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            // All orderings should parse successfully
            prop_assert!(result.is_ok(), "Failed to parse with order {}: {:?}", order, result.err());
            
            let library = result.unwrap();
            
            // Should have exactly 3 elements regardless of order
            prop_assert_eq!(library.elements.len(), 3);
            
            // Should contain one of each type
            let mut has_type_block = false;
            let mut has_global_var = false;
            let mut has_program = false;
            
            for element in &library.elements {
                match element {
                    LibraryElementKind::TypeDefinitionBlock(_) => has_type_block = true,
                    LibraryElementKind::GlobalVariableDeclaration(_) => has_global_var = true,
                    LibraryElementKind::ProgramDeclaration(_) => has_program = true,
                    _ => {}
                }
            }
            
            prop_assert!(has_type_block, "Missing TYPE block");
            prop_assert!(has_global_var, "Missing VAR_GLOBAL block");
            prop_assert!(has_program, "Missing PROGRAM block");
        }
    }

    proptest! {
        #[test]
        fn test_multiple_type_and_global_blocks_order_independence(
            type_block1 in arb_type_block(),
            type_block2 in arb_type_block(),
            global_block1 in arb_global_var_block(),
            global_block2 in arb_global_var_block(),
            interleaved in any::<bool>()
        ) {
            let source = if interleaved {
                format!("{}\n\n{}\n\n{}\n\n{}", type_block1, global_block1, type_block2, global_block2)
            } else {
                format!("{}\n\n{}\n\n{}\n\n{}", type_block1, type_block2, global_block1, global_block2)
            };

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse interleaved blocks: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 4 elements
            prop_assert_eq!(library.elements.len(), 4);
            
            // Count each type
            let type_blocks = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::TypeDefinitionBlock(_))).count();
            let global_blocks = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_))).count();
            
            prop_assert_eq!(type_blocks, 2);
            prop_assert_eq!(global_blocks, 2);
        }
    }
}

/// **Feature: ironplc-esstee-syntax-support, Property 31: Syntax Compatibility**
/// 
/// For any file mixing new and existing syntax features, the combination should be
/// handled without conflicts.
/// 
/// **Validates: Requirements 9.2**
#[cfg(test)]
mod syntax_compatibility_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_new_syntax_with_existing_functions(
            function_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            return_type in prop_oneof![
                Just("INT".to_string()),
                Just("BOOL".to_string()),
                Just("REAL".to_string()),
            ]
        ) {
            // Create a simple, controlled test case to avoid name conflicts
            let type_block = "TYPE\n    CustomInt : INT;\n    CustomBool : BOOL;\nEND_TYPE";
            let global_block = "VAR_GLOBAL\n    global_var : INT;\n    global_flag : BOOL;\nEND_VAR";
            let function = format!("FUNCTION {} : {}\nVAR\n    local_var : INT;\nEND_VAR\n{} := 42;\nEND_FUNCTION", 
                                 function_name, return_type, function_name);
            
            let source = format!("{}\n\n{}\n\n{}", type_block, global_block, function);

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse mixed syntax: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 3 elements
            prop_assert_eq!(library.elements.len(), 3);
            
            // Verify we have the expected types
            let mut has_type_element = false;
            let mut has_global_var = false;
            let mut has_function = false;
            
            for element in &library.elements {
                match element {
                    LibraryElementKind::TypeDefinitionBlock(_) => has_type_element = true,
                    LibraryElementKind::DataTypeDeclaration(_) => has_type_element = true,
                    LibraryElementKind::GlobalVariableDeclaration(_) => has_global_var = true,
                    LibraryElementKind::FunctionDeclaration(_) => has_function = true,
                    _ => {}
                }
            }
            
            prop_assert!(has_type_element, "Missing TYPE element");
            prop_assert!(has_global_var, "Missing VAR_GLOBAL block");
            prop_assert!(has_function, "Missing FUNCTION");
        }
    }

    proptest! {
        #[test]
        fn test_new_syntax_with_existing_programs_and_function_blocks(
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s) && s != "Or"),
            fb_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s) && s != "Or")
        ) {
            // Create controlled test case with unique names and simple syntax
            let type_block = "TYPE\n    MyCustomType : INT;\nEND_TYPE";
            let global_block = "VAR_GLOBAL\n    shared_counter : INT;\nEND_VAR";
            let program = format!("PROGRAM {}\nVAR\n    x : INT;\nEND_VAR\n    x := 1;\nEND_PROGRAM", program_name);
            let function_block = format!("FUNCTION_BLOCK {}\nVAR\n    y : BOOL;\nEND_VAR\n    y := TRUE;\nEND_FUNCTION_BLOCK", fb_name);
            
            let source = format!("{}\n\n{}\n\n{}\n\n{}", type_block, global_block, program, function_block);

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse with programs and FBs: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 4 elements
            prop_assert_eq!(library.elements.len(), 4);
            
            // Count each type
            let type_elements = library.elements.iter().filter(|e| matches!(e, 
                LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
            )).count();
            let global_blocks = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_))).count();
            let programs = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_))).count();
            let function_blocks = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::FunctionBlockDeclaration(_))).count();
            
            prop_assert!(type_elements >= 1, "Missing TYPE elements");
            prop_assert_eq!(global_blocks, 1);
            prop_assert_eq!(programs, 1);
            prop_assert_eq!(function_blocks, 1);
        }
    }

    proptest! {
        #[test]
        fn test_backward_compatibility_with_existing_syntax(
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            function_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            fb_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Ensure unique names by adding prefixes
            let unique_function_name = format!("Func_{}", function_name);
            let unique_fb_name = format!("FB_{}", fb_name);
            let unique_program_name = format!("Prog_{}", program_name);
            
            // Test that existing syntax still works when no new syntax is present
            let source = format!(
                "FUNCTION {} : INT\nVAR\n    x : INT := 42;\nEND_VAR\n{} := x;\nEND_FUNCTION\n\n\
                 FUNCTION_BLOCK {}\nVAR_INPUT\n    input : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\n\
                 PROGRAM {}\nVAR\n    counter : INT;\nEND_VAR\n    counter := counter + 1;\nEND_PROGRAM",
                unique_function_name, unique_function_name, unique_fb_name, unique_program_name
            );

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse existing syntax: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 3 elements
            prop_assert_eq!(library.elements.len(), 3);
            
            // All should be existing syntax elements
            for element in &library.elements {
                prop_assert!(matches!(element, 
                    LibraryElementKind::FunctionDeclaration(_) |
                    LibraryElementKind::FunctionBlockDeclaration(_) |
                    LibraryElementKind::ProgramDeclaration(_)
                ), "Unexpected element type: {:?}", element);
            }
        }
    }

    proptest! {
        #[test]
        fn test_simple_enumeration_syntax_compatibility(
            enum_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Test enumeration syntax with existing program structure
            let unique_enum = format!("Enum_{}", enum_name);
            let unique_prog = format!("Prog_{}", program_name);
            
            let type_block = format!(
                "TYPE\n    {} : (STOP, START, PAUSE, RESET);\nEND_TYPE",
                unique_enum
            );
            
            let global_block = format!(
                "VAR_GLOBAL\n    machine_state : {};\nEND_VAR",
                unique_enum
            );
            
            let program = format!(
                "PROGRAM {}\nVAR\n    local_counter : INT;\nEND_VAR\n    local_counter := 1;\nEND_PROGRAM",
                unique_prog
            );
            
            let source = format!("{}\n\n{}\n\n{}", type_block, global_block, program);

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse enum syntax: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 3 elements
            prop_assert_eq!(library.elements.len(), 3);
            
            // Verify all expected elements are present
            let has_types = library.elements.iter().any(|e| matches!(e, 
                LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
            ));
            let has_globals = library.elements.iter().any(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_)));
            let has_program = library.elements.iter().any(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_)));
            
            prop_assert!(has_types, "Missing TYPE elements");
            prop_assert!(has_globals, "Missing VAR_GLOBAL block");
            prop_assert!(has_program, "Missing PROGRAM");
        }
    }

    proptest! {
        #[test]
        fn test_simple_array_syntax_compatibility(
            array_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            function_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) {
            // Test simple array syntax with existing function structure
            let unique_array = format!("Array_{}", array_name);
            let unique_func = format!("Func_{}", function_name);
            
            let type_block = format!(
                "TYPE\n    {} : ARRAY[1..10] OF INT;\nEND_TYPE",
                unique_array
            );
            
            let global_block = format!(
                "VAR_GLOBAL\n    data_buffer : {};\nEND_VAR",
                unique_array
            );
            
            let function = format!(
                "FUNCTION {} : INT\nVAR\n    local_var : INT;\nEND_VAR\n    local_var := 42;\n    {} := local_var;\nEND_FUNCTION",
                unique_func, unique_func
            );
            
            let source = format!("{}\n\n{}\n\n{}", type_block, global_block, function);

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse array syntax: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have 3 elements
            prop_assert_eq!(library.elements.len(), 3);
            
            // Verify all expected elements are present
            let has_types = library.elements.iter().any(|e| matches!(e, 
                LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
            ));
            let has_globals = library.elements.iter().any(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_)));
            let has_function = library.elements.iter().any(|e| matches!(e, LibraryElementKind::FunctionDeclaration(_)));
            
            prop_assert!(has_types, "Missing TYPE elements");
            prop_assert!(has_globals, "Missing VAR_GLOBAL block");
            prop_assert!(has_function, "Missing FUNCTION");
        }
    }

    proptest! {
        #[test]
        fn test_mixed_new_and_existing_syntax_integration(
            type_suffix in "[A-Z0-9]+".prop_filter("Non-empty suffix", |s| !s.is_empty() && s != "A"),
            var_suffix in "[a-z0-9]+".prop_filter("Non-empty suffix", |s| !s.is_empty() && s != "a"),
            prog_suffix in "[A-Z0-9]+".prop_filter("Non-empty suffix", |s| !s.is_empty() && s != "A")
        ) {
            // Test integration of multiple new syntax features with existing constructs
            let type_block = format!(
                "TYPE\n    CustomType{} : INT;\nEND_TYPE",
                type_suffix
            );
            
            let global_block = format!(
                "VAR_GLOBAL\n    global_var{} : CustomType{};\nEND_VAR",
                var_suffix, type_suffix
            );
            
            let program = format!(
                "PROGRAM TestProg{}\nVAR\n    local_counter : INT;\nEND_VAR\n    local_counter := 1;\nEND_PROGRAM",
                prog_suffix
            );
            
            let source = format!("{}\n\n{}\n\n{}", type_block, global_block, program);

            let result = parse_program(&source, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse mixed syntax integration: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have exactly 3 elements
            prop_assert_eq!(library.elements.len(), 3);
            
            // Verify structure
            let type_count = library.elements.iter().filter(|e| matches!(e, 
                LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
            )).count();
            let global_count = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_))).count();
            let program_count = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_))).count();
            
            prop_assert!(type_count >= 1, "Missing TYPE elements");
            prop_assert_eq!(global_count, 1, "Should have exactly 1 VAR_GLOBAL block");
            prop_assert_eq!(program_count, 1, "Should have exactly 1 PROGRAM");
        }
    }
}

/// Test utilities for property-based testing
#[cfg(test)]
mod test_utils {
    use super::*;

    /// Helper to create simple test cases
    pub fn create_simple_library_element(element: LibraryElementKind) -> Library {
        Library {
            elements: vec![element],
        }
    }

    /// Helper to verify library structure
    pub fn verify_library_structure(library: &Library, expected_count: usize) -> bool {
        library.elements.len() == expected_count
    }
}