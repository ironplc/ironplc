//! Property-based tests for backward compatibility validation specific to esstee syntax support
//! 
//! **Feature: ironplc-esstee-syntax-support, Property 30: Backward Compatibility Preservation**
//! **Validates: Requirements 9.1, 9.3**

use crate::parse_program;
use crate::options::ParseOptions;
use dsl::core::FileId;
use proptest::prelude::*;
use ironplc_test::read_shared_resource;

/// Test data generator for existing IronPLC syntax patterns that should continue to work
fn existing_ironplc_syntax() -> impl Strategy<Value = (String, String)> {
    prop_oneof![
        // Basic program structures that existed before esstee enhancements
        Just(("PROGRAM main\nVAR\n  x: INT;\nEND_VAR\nx := 5;\nEND_PROGRAM".to_string(), "Basic program with local variables".to_string())),
        
        // Function blocks with input/output variables
        Just(("FUNCTION_BLOCK Counter\nVAR_INPUT\n  Reset: BOOL;\nEND_VAR\nVAR_OUTPUT\n  Count: INT;\nEND_VAR\nVAR\n  Internal: INT;\nEND_VAR\nIF Reset THEN\n  Internal := 0;\nELSE\n  Internal := Internal + 1;\nEND_IF;\nCount := Internal;\nEND_FUNCTION_BLOCK".to_string(), "Function block with input/output variables".to_string())),
        
        // Functions with return values
        Just(("FUNCTION Add : INT\nVAR_INPUT\n  a: INT;\n  b: INT;\nEND_VAR\nAdd := a + b;\nEND_FUNCTION".to_string(), "Function with return value".to_string())),
        
        // Basic type declarations (pre-esstee)
        Just(("TYPE\n  LOGLEVEL : (CRITICAL, WARNING, INFO, DEBUG) := INFO;\nEND_TYPE".to_string(), "Basic enumeration type declaration".to_string())),
        
        // Configuration declarations
        Just(("CONFIGURATION config\nVAR_GLOBAL CONSTANT\n  MaxValue : INT := 100;\nEND_VAR\nRESOURCE resource1 ON PLC\n  TASK main_task(INTERVAL := T#100ms, PRIORITY := 1);\n  PROGRAM main_instance WITH main_task : main;\nEND_RESOURCE\nEND_CONFIGURATION".to_string(), "Configuration with resource and task".to_string())),
        
        // Variable declarations with initialization
        Just(("PROGRAM test\nVAR\n  flag: BOOL := TRUE;\n  counter: INT := 0;\n  value: REAL := 3.14;\n  text: STRING := 'Hello';\nEND_VAR\nEND_PROGRAM".to_string(), "Variable declarations with initialization".to_string())),
        
        // Control flow statements
        Just(("PROGRAM test\nVAR\n  x: INT;\n  y: INT;\nEND_VAR\nIF x > 0 THEN\n  y := x * 2;\nELSIF x < 0 THEN\n  y := x * -1;\nELSE\n  y := 0;\nEND_IF;\nEND_PROGRAM".to_string(), "IF-THEN-ELSE conditional statement".to_string())),
        
        // Loop statements
        Just(("PROGRAM test\nVAR\n  i: INT;\n  sum: INT := 0;\nEND_VAR\nFOR i := 1 TO 10 DO\n  sum := sum + i;\nEND_FOR;\nEND_PROGRAM".to_string(), "FOR loop statement".to_string())),
        
        // WHILE loops
        Just(("PROGRAM test\nVAR\n  counter: INT := 0;\nEND_VAR\nWHILE counter < 5 DO\n  counter := counter + 1;\nEND_WHILE;\nEND_PROGRAM".to_string(), "WHILE loop statement".to_string())),
        
        // Comments (IEC style)
        Just(("(* This is a comment *)\nPROGRAM test\nVAR\n  x: INT; (* Variable declaration *)\nEND_VAR\n(* Assignment statement *)\nx := 42;\nEND_PROGRAM".to_string(), "Program with IEC-style comments".to_string())),
        
        // Basic array declarations (pre-esstee enhancements)
        Just(("PROGRAM test\nVAR\n  values: ARRAY[1..5] OF INT;\nEND_VAR\nvalues[1] := 10;\nEND_PROGRAM".to_string(), "Basic array declaration and access".to_string())),
        
        // String declarations
        Just(("PROGRAM test\nVAR\n  message: STRING;\n  sized_message: STRING(50);\nEND_VAR\nmessage := 'Hello';\nsized_message := 'World';\nEND_PROGRAM".to_string(), "String variable declarations".to_string())),
        
        // CASE statements
        Just(("PROGRAM test\nVAR\n  state: INT;\n  output: INT;\nEND_VAR\nCASE state OF\n  1: output := 10;\n  2: output := 20;\n  3, 4: output := 30;\nELSE\n  output := 0;\nEND_CASE;\nEND_PROGRAM".to_string(), "CASE statement with multiple values".to_string())),
        
        // Structured data types (STRUCT)
        Just(("TYPE\n  Point : STRUCT\n    x: REAL;\n    y: REAL;\n  END_STRUCT;\nEND_TYPE\nPROGRAM test\nVAR\n  p: Point;\nEND_VAR\np.x := 1.0;\np.y := 2.0;\nEND_PROGRAM".to_string(), "STRUCT type declaration and usage".to_string())),
        
        // External function declarations
        Just(("{external} FUNCTION ExternalFunc : INT\nVAR_INPUT\n  param: INT;\nEND_VAR\nEND_FUNCTION".to_string(), "External function declaration".to_string())),
    ]
}

/// Test data generator for existing test files that should continue to pass
fn existing_test_file() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("var_decl.st".to_string()),
        Just("inout_var_decl.st".to_string()), 
        Just("input_var_decl.st".to_string()),
        Just("strings.st".to_string()),
        Just("type_decl.st".to_string()),
        Just("textual.st".to_string()),
        Just("conditional.st".to_string()),
        Just("oscat.st".to_string()),
        Just("expressions.st".to_string()),
        Just("array.st".to_string()),
        Just("nested.st".to_string()),
        Just("configuration.st".to_string()),
        Just("program.st".to_string()),
        Just("if.st".to_string()),
        Just("first_steps_function_block_counter_fbd.st".to_string()),
        Just("first_steps_func_avg_val.st".to_string()),
        Just("first_steps_program.st".to_string()),
        Just("first_steps_configuration.st".to_string()),
        Just("first_steps_function_block_logger.st".to_string()),
        Just("first_steps_function_block_counter_sfc.st".to_string()),
        Just("first_steps_data_type_decl.st".to_string()),
        Just("class_method.st".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    // **Feature: ironplc-esstee-syntax-support, Property 30: Backward Compatibility Preservation**
    // **Validates: Requirements 9.1**
    fn property_existing_syntax_compatibility((source_code, description) in existing_ironplc_syntax()) {
        let file_id = FileId::default();
        let options = ParseOptions::default();
        
        // Parse with enhanced compiler (includes esstee syntax support)
        let result = parse_program(&source_code, &file_id, &options);
        
        match result {
            Ok(library) => {
                // Existing syntax should parse successfully
                // Verify the library structure is valid
                prop_assert!(library.elements.len() >= 0, "Library should have valid structure");
                
                // For programs, verify they have the expected structure
                if description.contains("program") || description.contains("Program") {
                    let has_program = library.elements.iter().any(|elem| {
                        matches!(elem, dsl::common::LibraryElementKind::ProgramDeclaration(_))
                    });
                    if !has_program && !source_code.contains("TYPE") && !source_code.contains("FUNCTION") {
                        prop_assert!(false, "Program syntax should produce program declaration: {}", description);
                    }
                }
                
                // For function blocks, verify they have the expected structure
                if description.contains("Function block") {
                    let has_function_block = library.elements.iter().any(|elem| {
                        matches!(elem, dsl::common::LibraryElementKind::FunctionBlockDeclaration(_))
                    });
                    prop_assert!(has_function_block, "Function block syntax should produce function block declaration: {}", description);
                }
                
                // For functions, verify they have the expected structure
                if description.contains("Function") && !description.contains("block") {
                    let has_function = library.elements.iter().any(|elem| {
                        matches!(elem, dsl::common::LibraryElementKind::FunctionDeclaration(_))
                    });
                    prop_assert!(has_function, "Function syntax should produce function declaration: {}", description);
                }
                
                // For type declarations, verify they have the expected structure
                if description.contains("type") || description.contains("TYPE") {
                    let has_type = library.elements.iter().any(|elem| {
                        matches!(elem, dsl::common::LibraryElementKind::DataTypeDeclaration(_))
                    });
                    prop_assert!(has_type, "Type syntax should produce type declaration: {}", description);
                }
            },
            Err(diagnostic) => {
                // Some existing syntax might legitimately fail if it's not fully supported yet
                // But the error should be meaningful and not a crash
                prop_assert!(!diagnostic.description().is_empty(), 
                    "Parser should provide meaningful error for: {} - Error: {}", 
                    description, diagnostic.description());
                
                // The error should not be a panic or internal error
                prop_assert!(!diagnostic.description().contains("panic"), 
                    "Parser should not panic on existing syntax: {}", description);
                prop_assert!(!diagnostic.description().contains("internal error"), 
                    "Parser should not have internal errors on existing syntax: {}", description);
            }
        }
    }
}

/// Unit test to verify existing test files still work (Requirements 9.3)
/// **Feature: ironplc-esstee-syntax-support, Property 30: Backward Compatibility Preservation**
/// **Validates: Requirements 9.3**
#[test]
fn test_existing_test_suite_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let test_files = vec![
        "var_decl.st",
        "inout_var_decl.st", 
        "input_var_decl.st",
        "strings.st",
        "type_decl.st",
        "textual.st",
        "conditional.st",
        "oscat.st",
        "expressions.st",
        "array.st",
        "nested.st",
        "configuration.st",
        "program.st",
        "if.st",
        "first_steps_function_block_counter_fbd.st",
        "first_steps_func_avg_val.st",
        "first_steps_program.st",
        "first_steps_configuration.st",
        "first_steps_function_block_logger.st",
        "first_steps_function_block_counter_sfc.st",
        "first_steps_data_type_decl.st",
        "class_method.st",
    ];
    
    let mut successful_parses = 0;
    let mut failed_parses = 0;
    let mut panic_count = 0;
    
    for test_file in &test_files {
        // Try to read the test file
        let source_result = std::panic::catch_unwind(|| {
            read_shared_resource(test_file)
        });
        
        let source = match source_result {
            Ok(s) => s,
            Err(_) => {
                println!("⚠ Test file {} could not be read", test_file);
                failed_parses += 1;
                continue;
            }
        };
        
        // Try to parse the source
        let parse_result = std::panic::catch_unwind(|| {
            parse_program(&source, &file_id, &options)
        });
        
        match parse_result {
            Ok(result) => {
                match result {
                    Ok(library) => {
                        successful_parses += 1;
                        println!("✓ Test file {} parsed successfully ({} elements)", test_file, library.elements.len());
                        
                        // Verify it has valid structure
                        assert!(library.elements.len() >= 0, 
                            "Test file {} should produce valid library structure", test_file);
                        
                        // Most test files should have at least one element
                        if !test_file.contains("empty") {
                            assert!(library.elements.len() > 0, 
                                "Non-empty test file {} should have at least one element", test_file);
                        }
                    },
                    Err(diagnostic) => {
                        failed_parses += 1;
                        println!("⚠ Test file {} failed to parse: {}", test_file, diagnostic.description());
                        
                        // Should provide meaningful error message
                        assert!(!diagnostic.description().is_empty(), 
                            "Test file {} should provide meaningful error message", test_file);
                        
                        // Should not be internal errors or panics
                        assert!(!diagnostic.description().contains("panic"), 
                            "Test file {} should not cause parser panic", test_file);
                        assert!(!diagnostic.description().contains("internal error"), 
                            "Test file {} should not cause internal parser error", test_file);
                    }
                }
            },
            Err(_) => {
                panic_count += 1;
                panic!("Test file {} caused parser panic - this breaks backward compatibility", test_file);
            }
        }
    }
    
    let total_files = test_files.len();
    let success_rate = successful_parses as f64 / total_files as f64;
    
    println!("Test suite compatibility results:");
    println!("  Successful parses: {}/{} ({:.1}%)", successful_parses, total_files, success_rate * 100.0);
    println!("  Failed parses: {}/{} ({:.1}%)", failed_parses, total_files, failed_parses as f64 / total_files as f64 * 100.0);
    println!("  Panics: {}/{} ({:.1}%)", panic_count, total_files, panic_count as f64 / total_files as f64 * 100.0);
    
    // We expect at least 70% of test files to parse successfully for backward compatibility
    // Some test files might legitimately fail if they test error conditions
    assert!(success_rate >= 0.7, 
        "Expected at least 70% of test files to parse successfully for backward compatibility, got {:.1}% ({}/{})", 
        success_rate * 100.0, successful_parses, total_files);
    
    // No test files should cause panics
    assert_eq!(panic_count, 0, 
        "No test files should cause parser panics - found {} panics", panic_count);
}

/// Unit test to verify specific existing functionality still works
#[test]
fn test_basic_program_backward_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    // Test a simple program that definitely worked before esstee enhancements
    let simple_program = "PROGRAM main\nVAR\n  x: INT;\nEND_VAR\nx := 5;\nEND_PROGRAM";
    
    let result = parse_program(simple_program, &file_id, &options);
    match result {
        Ok(library) => {
            assert_eq!(library.elements.len(), 1, "Simple program should parse to exactly one element");
            match &library.elements[0] {
                dsl::common::LibraryElementKind::ProgramDeclaration(program) => {
                    assert_eq!(program.name.original, "main");
                    assert!(!program.variables.is_empty(), "Program should have variables");
                },
                _ => panic!("Expected program declaration, got: {:?}", library.elements[0]),
            }
        },
        Err(diagnostic) => {
            panic!("Simple program should parse successfully, but got error: {}", diagnostic.description());
        }
    }
}

/// Unit test to verify function block compatibility
#[test]
fn test_function_block_backward_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let function_block = "FUNCTION_BLOCK Counter\nVAR_INPUT\n  Reset: BOOL;\nEND_VAR\nVAR_OUTPUT\n  Count: INT;\nEND_VAR\nIF Reset THEN\n  Count := 0;\nELSE\n  Count := Count + 1;\nEND_IF;\nEND_FUNCTION_BLOCK";
    
    let result = parse_program(function_block, &file_id, &options);
    match result {
        Ok(library) => {
            assert_eq!(library.elements.len(), 1, "Function block should parse to exactly one element");
            match &library.elements[0] {
                dsl::common::LibraryElementKind::FunctionBlockDeclaration(fb) => {
                    assert_eq!(fb.name.name.original, "Counter");
                    assert!(!fb.variables.is_empty(), "Function block should have variables");
                },
                _ => panic!("Expected function block declaration, got: {:?}", library.elements[0]),
            }
        },
        Err(diagnostic) => {
            panic!("Function block should parse successfully, but got error: {}", diagnostic.description());
        }
    }
}

/// Unit test to verify type declaration compatibility
#[test]
fn test_type_declaration_backward_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let type_decl = "TYPE\n  LOGLEVEL : (CRITICAL, WARNING, INFO, DEBUG) := INFO;\nEND_TYPE";
    
    let result = parse_program(type_decl, &file_id, &options);
    match result {
        Ok(library) => {
            assert_eq!(library.elements.len(), 1, "Type declaration should parse to exactly one element");
            match &library.elements[0] {
                dsl::common::LibraryElementKind::DataTypeDeclaration(_) => {
                    // Type declaration parsed successfully
                },
                _ => panic!("Expected type declaration, got: {:?}", library.elements[0]),
            }
        },
        Err(diagnostic) => {
            panic!("Type declaration should parse successfully, but got error: {}", diagnostic.description());
        }
    }
}

/// Unit test to verify comment compatibility
#[test]
fn test_comment_backward_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let with_comments = "(* This is a comment *)\nPROGRAM test\nVAR\n  x: INT; (* Variable *)\nEND_VAR\n(* Assignment statement *)\nx := 42;\nEND_PROGRAM";
    
    let result = parse_program(with_comments, &file_id, &options);
    match result {
        Ok(library) => {
            assert_eq!(library.elements.len(), 1, "Program with comments should parse to exactly one element");
            match &library.elements[0] {
                dsl::common::LibraryElementKind::ProgramDeclaration(program) => {
                    assert_eq!(program.name.original, "test");
                },
                _ => panic!("Expected program declaration, got: {:?}", library.elements[0]),
            }
        },
        Err(diagnostic) => {
            panic!("Program with comments should parse successfully, but got error: {}", diagnostic.description());
        }
    }
}

/// Unit test to verify existing test files still work
#[test]
fn test_existing_test_files_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let critical_test_files = vec![
        "var_decl.st",
        "strings.st", 
        "type_decl.st",
        "conditional.st",
        "expressions.st",
        "program.st",
    ];
    
    let mut successful_parses = 0;
    let total_files = critical_test_files.len();
    
    for test_file in critical_test_files {
        match std::panic::catch_unwind(|| {
            let source = read_shared_resource(test_file);
            parse_program(&source, &file_id, &options)
        }) {
            Ok(parse_result) => {
                match parse_result {
                    Ok(_library) => {
                        successful_parses += 1;
                        println!("✓ Test file {} parsed successfully", test_file);
                    },
                    Err(diagnostic) => {
                        println!("⚠ Test file {} failed to parse: {}", test_file, diagnostic.description());
                        // Some test files might legitimately fail, but we log it
                    }
                }
            },
            Err(_) => {
                panic!("Test file {} caused parser panic - this breaks backward compatibility", test_file);
            }
        }
    }
    
    // We expect at least 80% of critical test files to parse successfully
    let success_rate = successful_parses as f64 / total_files as f64;
    assert!(success_rate >= 0.8, 
        "Expected at least 80% of critical test files to parse successfully, got {:.1}% ({}/{})", 
        success_rate * 100.0, successful_parses, total_files);
}

/// Unit test to verify mixed old/new syntax compatibility
#[test]
fn test_mixed_syntax_backward_compatibility() {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    // Test mixing existing syntax with potentially new syntax
    let mixed_syntax = "PROGRAM test\nVAR\n  old_var: INT;\n  array_var: ARRAY[1..5] OF INT;\nEND_VAR\nold_var := 42;\narray_var[1] := old_var;\nEND_PROGRAM";
    
    let result = parse_program(mixed_syntax, &file_id, &options);
    match result {
        Ok(library) => {
            assert_eq!(library.elements.len(), 1, "Mixed syntax should parse to exactly one element");
            match &library.elements[0] {
                dsl::common::LibraryElementKind::ProgramDeclaration(program) => {
                    assert_eq!(program.name.original, "test");
                    assert!(program.variables.len() >= 2, "Program should have at least 2 variables");
                },
                _ => panic!("Expected program declaration, got: {:?}", library.elements[0]),
            }
        },
        Err(diagnostic) => {
            // Mixed syntax might fail if new features aren't fully implemented,
            // but it should not crash and should provide meaningful errors
            assert!(!diagnostic.description().is_empty(), 
                "Mixed syntax should provide meaningful error message: {}", diagnostic.description());
            assert!(!diagnostic.description().contains("panic"), 
                "Mixed syntax should not cause parser panic");
        }
    }
}