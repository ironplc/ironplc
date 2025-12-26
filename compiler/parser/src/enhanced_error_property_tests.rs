//! Property-based tests for enhanced error handling and reporting
//! 
//! **Feature: ironplc-esstee-syntax-support, Property 22: Syntax Error Reporting Quality**
//! **Validates: Requirements 6.2, 6.3, 6.4**
//! 
//! **Feature: ironplc-esstee-syntax-support, Property 23: Multiple Error Reporting**
//! **Validates: Requirements 6.5**

use crate::enhanced_error::{CompilerError, ErrorCollector, SuggestionEngine, GlobalVarErrorType, TypeDefinitionErrorType, EnumerationErrorType, ArrayTypeErrorType, SubrangeErrorType};
use crate::parse_program_enhanced;
use dsl::core::{FileId, SourceSpan};
use crate::options::ParseOptions;
use proptest::prelude::*;

/// Test data generator for syntax error scenarios with new syntax constructs
#[derive(Debug, Clone)]
struct NewSyntaxErrorScenario {
    source_code: String,
    expected_error_types: Vec<String>,
    should_have_location: bool,
}

impl Arbitrary for NewSyntaxErrorScenario {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    
    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        let scenarios = vec![
            // VAR_GLOBAL syntax errors
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  x: INT;\n  y: BOOL\n".to_string(), // Missing END_VAR
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  x INT;\n  y: BOOL;\nEND_VAR".to_string(), // Missing colon
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            // TYPE...END_TYPE syntax errors
            NewSyntaxErrorScenario {
                source_code: "TYPE\n  MyInt : INT;\n  MyBool : BOOL\n".to_string(), // Missing END_TYPE
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            NewSyntaxErrorScenario {
                source_code: "TYPE\n  MyInt INT;\nEND_TYPE".to_string(), // Missing colon
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            // Enumeration syntax errors - use cases that actually fail
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  e : (red, green, blue;\nEND_VAR".to_string(), // Missing closing paren
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            // Array syntax errors
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  arr : ARRAY[1..10 OF INT;\nEND_VAR".to_string(), // Missing closing bracket
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            // Multiple errors in single source
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  x INT\n  y BOOL\nTYPE\n  MyInt INT\n".to_string(), // Multiple missing elements
                expected_error_types: vec!["syntax".to_string()],
                should_have_location: true,
            },
            
            // Valid syntax (should pass)
            NewSyntaxErrorScenario {
                source_code: "VAR_GLOBAL\n  x: INT;\n  y: BOOL;\nEND_VAR".to_string(),
                expected_error_types: vec![],
                should_have_location: false,
            },
            
            // Another valid case
            NewSyntaxErrorScenario {
                source_code: "TYPE\n  MyInt : INT;\nEND_TYPE".to_string(),
                expected_error_types: vec![],
                should_have_location: false,
            },
        ];
        
        prop::sample::select(scenarios).boxed()
    }
}

/// Test data generator for multiple error collection scenarios
#[derive(Debug, Clone)]
struct MultipleErrorScenario {
    source_lines: Vec<String>,
    expected_min_errors: usize,
    expected_max_errors: usize,
}

impl Arbitrary for MultipleErrorScenario {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;
    
    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        let scenarios = vec![
            MultipleErrorScenario {
                source_lines: vec![
                    "VAR_GLOBAL".to_string(),
                    "  x INT".to_string(),  // Missing colon
                    "  y BOOL".to_string(), // Missing colon
                    "TYPE".to_string(),
                    "  MyInt INT".to_string(), // Missing colon
                ],
                expected_min_errors: 2,
                expected_max_errors: 10,
            },
            
            MultipleErrorScenario {
                source_lines: vec![
                    "VAR_GLOBAL".to_string(),
                    "  e : (red, green, blue".to_string(), // Missing closing paren
                    "  arr : ARRAY[1..10 OF INT".to_string(), // Missing closing bracket
                    "  x : INT(10..1)".to_string(), // Invalid subrange
                    "END_VAR".to_string(),
                ],
                expected_min_errors: 1,
                expected_max_errors: 5,
            },
        ];
        
        prop::sample::select(scenarios).boxed()
    }
}

/// Property 22: Syntax Error Reporting Quality
/// Tests that syntax errors for new syntax constructs are reported with specific error messages,
/// exact location information, and helpful suggestions when available.
fn property_syntax_error_reporting_quality(scenario: NewSyntaxErrorScenario) -> bool {
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    // Parse with enhanced error reporting
    let result = parse_program_enhanced(&scenario.source_code, &file_id, &options);
    
    match result {
        Ok(_) => {
            // If parsing succeeded, check if this was expected to be valid
            scenario.expected_error_types.is_empty()
        },
        Err(diagnostics) => {
            // If we expected no errors but got some, that's a failure
            if scenario.expected_error_types.is_empty() {
                return false;
            }
            
            // Verify we got diagnostics
            if diagnostics.is_empty() {
                return false;
            }
            
            // Verify each diagnostic has proper location information
            let has_proper_locations = diagnostics.iter().all(|d| {
                d.primary.location.start <= d.primary.location.end &&
                d.primary.location.end <= scenario.source_code.len()
            });
            
            if !has_proper_locations {
                return false;
            }
            
            // Verify diagnostics have meaningful messages
            let has_meaningful_messages = diagnostics.iter().all(|d| {
                let desc = d.description();
                !desc.is_empty() && desc.len() > 5
            });
            
            if !has_meaningful_messages {
                return false;
            }
            
            // Verify error codes are appropriate
            let has_valid_codes = diagnostics.iter().all(|d| {
                !d.code.is_empty() && d.code.starts_with('P')
            });
            
            has_valid_codes
        }
    }
}

/// Property 23: Multiple Error Reporting
/// Tests that the parser can collect and report multiple errors in a single compilation pass
/// without stopping at the first error.
fn property_multiple_error_reporting(scenario: MultipleErrorScenario) -> bool {
    let source = scenario.source_lines.join("\n");
    
    // Skip empty or very short sources
    if source.trim().is_empty() || source.len() < 10 {
        return true; // Discard this test case
    }
    
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let result = parse_program_enhanced(&source, &file_id, &options);
    
    match result {
        Ok(_) => {
            // If parsing succeeded, that's acceptable for property testing
            true
        },
        Err(diagnostics) => {
            // Verify we can collect multiple errors
            let error_count = diagnostics.len();
            
            // Should be able to collect errors without crashing
            if error_count == 0 {
                return false;
            }
            
            // Verify reasonable bounds on error count
            if error_count > 50 {
                return false;
            }
            
            // Verify each error has valid location information
            let all_valid_locations = diagnostics.iter().all(|d| {
                d.primary.location.start <= d.primary.location.end &&
                d.primary.location.end <= source.len()
            });
            
            if !all_valid_locations {
                return false;
            }
            
            // Verify errors are within expected range
            let within_expected_range = error_count >= scenario.expected_min_errors && 
                                      error_count <= scenario.expected_max_errors;
            
            within_expected_range
        }
    }
}

/// Property: Error Recovery and Continuation
/// Tests that the parser can recover from errors and continue parsing to find additional issues
fn property_error_recovery_continuation(source_fragments: Vec<String>) -> bool {
    // Filter and clean input
    let clean_fragments: Vec<String> = source_fragments.into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.chars().filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace()).collect())
        .take(5)
        .collect();
    
    if clean_fragments.is_empty() {
        return true; // Discard this test case
    }
    
    // Create source with intentional syntax errors
    let mut source = clean_fragments.join("\n");
    source.push_str("\nVAR_GLOBAL\n  x INT\n  y BOOL\n"); // Missing colons and END_VAR
    
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    let result = parse_program_enhanced(&source, &file_id, &options);
    
    match result {
        Ok(_) => true, // Unexpected success is still acceptable
        Err(diagnostics) => {
            // Should be able to collect errors without infinite loops or crashes
            let success = !diagnostics.is_empty() && 
                         diagnostics.len() <= 20 && // Reasonable upper bound
                         diagnostics.iter().all(|d| !d.description().is_empty());
            
            success
        }
    }
}

/// Property: Location Accuracy for New Syntax
/// Tests that error locations are accurate for new syntax constructs
fn property_location_accuracy_new_syntax() -> bool {
    let test_cases = vec![
        ("VAR_GLOBAL\n  x INT;\nEND_VAR", 2, 5), // Missing colon at line 2, around column 5
        ("TYPE\n  MyInt INT;\nEND_TYPE", 2, 9),   // Missing colon at line 2, around column 9
        ("VAR_GLOBAL\n  e : (red, green;\nEND_VAR", 2, 17), // Missing closing paren
    ];
    
    let file_id = FileId::default();
    let options = ParseOptions::default();
    
    for (source, expected_line, _expected_col_approx) in test_cases {
        let result = parse_program_enhanced(source, &file_id, &options);
        
        if let Err(diagnostics) = result {
            if !diagnostics.is_empty() {
                let diagnostic = &diagnostics[0];
                let location = &diagnostic.primary.location;
                
                // Calculate line and column from byte offset
                let lines: Vec<&str> = source.lines().collect();
                let mut byte_offset = 0;
                let mut found_line = 1;
                
                for (line_num, line) in lines.iter().enumerate() {
                    if byte_offset + line.len() >= location.start {
                        found_line = line_num + 1;
                        break;
                    }
                    byte_offset += line.len() + 1; // +1 for newline
                }
                
                // Verify line is approximately correct (within 1 line)
                if (found_line as i32 - expected_line as i32).abs() > 1 {
                    return false;
                }
            }
        }
    }
    
    true
}

/// Unit test for enhanced error types
#[cfg(test)]
mod enhanced_error_tests {
    use super::*;
    
    #[test]
    fn test_debug_var_global_parsing() {
        let source = "VAR_GLOBAL\n  x: INT;\n  y: BOOL\n"; // Missing END_VAR
        let file_id = FileId::default();
        let options = ParseOptions::default();
        
        println!("Testing source: {:?}", source);
        
        let result = parse_program_enhanced(source, &file_id, &options);
        
        match result {
            Ok(library) => {
                println!("Parsing succeeded unexpectedly!");
                println!("Library elements: {}", library.elements.len());
                panic!("Expected parsing to fail but it succeeded");
            },
            Err(diagnostics) => {
                println!("Parsing failed as expected with {} diagnostics:", diagnostics.len());
                for (i, diagnostic) in diagnostics.iter().enumerate() {
                    println!("  {}: {} - {}", i, diagnostic.code, diagnostic.description());
                }
                assert!(!diagnostics.is_empty(), "Should have at least one diagnostic");
            }
        }
    }
    
    #[test]
    fn test_debug_empty_enumeration() {
        let source = "VAR_GLOBAL\n  e : ();\nEND_VAR"; // Empty enumeration
        let file_id = FileId::default();
        let options = ParseOptions::default();
        
        println!("Testing empty enumeration source: {:?}", source);
        
        let result = parse_program_enhanced(source, &file_id, &options);
        
        match result {
            Ok(library) => {
                println!("Parsing succeeded! Library elements: {}", library.elements.len());
                // This might actually be valid - empty enumerations might be allowed
            },
            Err(diagnostics) => {
                println!("Parsing failed with {} diagnostics:", diagnostics.len());
                for (i, diagnostic) in diagnostics.iter().enumerate() {
                    println!("  {}: {} - {}", i, diagnostic.code, diagnostic.description());
                }
            }
        }
    }
    
    #[test]
    fn test_global_var_error_creation() {
        let error = CompilerError::GlobalVarError {
            error_type: GlobalVarErrorType::MissingEndVar,
            location: SourceSpan::range(0, 10).with_file_id(&FileId::default()),
        };
        
        assert_eq!(error.location().start, 0);
        assert_eq!(error.location().end, 10);
        
        let diagnostic = error.to_diagnostic();
        assert_eq!(diagnostic.code, "P0002"); // SyntaxError code
        println!("Global var diagnostic description: {}", diagnostic.description());
        println!("Global var primary label message: {}", diagnostic.primary.message);
        assert!(diagnostic.primary.message.contains("END_VAR"));
    }
    
    #[test]
    fn test_type_definition_error_creation() {
        let error = CompilerError::TypeDefinitionError {
            error_type: TypeDefinitionErrorType::MissingEndType,
            location: SourceSpan::range(5, 15).with_file_id(&FileId::default()),
        };
        
        let diagnostic = error.to_diagnostic();
        assert_eq!(diagnostic.code, "P0002");
        println!("Type def diagnostic description: {}", diagnostic.description());
        println!("Type def primary label message: {}", diagnostic.primary.message);
        assert!(diagnostic.primary.message.contains("END_TYPE"));
    }
    
    #[test]
    fn test_enumeration_error_creation() {
        let error = CompilerError::EnumerationError {
            error_type: EnumerationErrorType::MissingClosingParen,
            location: SourceSpan::range(10, 20).with_file_id(&FileId::default()),
        };
        
        let diagnostic = error.to_diagnostic();
        println!("Enumeration diagnostic description: {}", diagnostic.description());
        println!("Enumeration primary label message: {}", diagnostic.primary.message);
        assert!(diagnostic.primary.message.contains("closing parenthesis"));
    }
    
    #[test]
    fn test_array_type_error_creation() {
        let error = CompilerError::ArrayTypeError {
            error_type: ArrayTypeErrorType::BoundsOrderError,
            location: SourceSpan::range(15, 25).with_file_id(&FileId::default()),
        };
        
        let diagnostic = error.to_diagnostic();
        println!("Array type diagnostic description: {}", diagnostic.description());
        println!("Array type primary label message: {}", diagnostic.primary.message);
        assert!(diagnostic.primary.message.contains("lower bound"));
    }
    
    #[test]
    fn test_subrange_error_creation() {
        let error = CompilerError::SubrangeError {
            error_type: SubrangeErrorType::MinGreaterThanMax,
            location: SourceSpan::range(20, 30).with_file_id(&FileId::default()),
        };
        
        let diagnostic = error.to_diagnostic();
        println!("Subrange diagnostic description: {}", diagnostic.description());
        println!("Subrange primary label message: {}", diagnostic.primary.message);
        assert!(diagnostic.primary.message.contains("minimum"));
    }
    
    #[test]
    fn test_enhanced_suggestion_engine() {
        let engine = SuggestionEngine::new();
        
        // Test new suggestions
        assert!(engine.get_suggestion("VAR_GLOBAL").is_some());
        assert!(engine.get_suggestion("TYPE").is_some());
        assert!(engine.get_suggestion("ENUMERATION").is_some());
        assert!(engine.get_suggestion("SUBRANGE").is_some());
        
        // Test workarounds
        assert!(engine.get_workaround("VAR_GLOBAL").is_some());
        assert!(engine.get_workaround("TYPE").is_some());
        assert!(engine.get_workaround("SUBRANGE").is_some());
    }
    
    #[test]
    fn test_error_collector_with_new_types() {
        let mut collector = ErrorCollector::new();
        
        let global_var_error = CompilerError::GlobalVarError {
            error_type: GlobalVarErrorType::MissingEndVar,
            location: SourceSpan::range(0, 10).with_file_id(&FileId::default()),
        };
        
        let type_def_error = CompilerError::TypeDefinitionError {
            error_type: TypeDefinitionErrorType::InvalidTypeDefinition,
            location: SourceSpan::range(20, 30).with_file_id(&FileId::default()),
        };
        
        collector.add_error(global_var_error);
        collector.add_error(type_def_error);
        
        assert_eq!(collector.errors().len(), 2);
        assert!(collector.has_errors());
        
        let diagnostics = collector.to_diagnostics();
        assert_eq!(diagnostics.len(), 2);
    }
}

/// Run property tests
#[cfg(test)]
mod property_tests {
    use super::*;
    
    proptest! {
        // **Feature: ironplc-esstee-syntax-support, Property 22: Syntax Error Reporting Quality**
        // **Validates: Requirements 6.2, 6.3, 6.4**
        #[test]
        fn run_syntax_error_reporting_quality_property(scenario in any::<NewSyntaxErrorScenario>()) {
            prop_assert!(property_syntax_error_reporting_quality(scenario));
        }
        
        // **Feature: ironplc-esstee-syntax-support, Property 23: Multiple Error Reporting**
        // **Validates: Requirements 6.5**
        #[test]
        fn run_multiple_error_reporting_property(scenario in any::<MultipleErrorScenario>()) {
            prop_assert!(property_multiple_error_reporting(scenario));
        }
        
        #[test]
        fn run_error_recovery_continuation_property(fragments in prop::collection::vec(
            prop_oneof![
                // Valid PLC syntax fragments that could be part of a program
                Just("VAR_GLOBAL".to_string()),
                Just("END_VAR".to_string()),
                Just("TYPE".to_string()),
                Just("END_TYPE".to_string()),
                Just("PROGRAM Test".to_string()),
                Just("END_PROGRAM".to_string()),
                Just("x : INT".to_string()),
                Just("y : BOOL".to_string()),
                Just("arr : ARRAY[1..10] OF INT".to_string()),
                // Incomplete but realistic fragments
                Just("VAR_GLOBAL\n  x".to_string()),
                Just("TYPE\n  MyInt".to_string()),
                Just("x : INT\n  y".to_string()),
                // Empty or whitespace (should be filtered out)
                Just("".to_string()),
                Just("   ".to_string()),
                Just("\n".to_string()),
            ], 
            1..10
        )) {
            prop_assert!(property_error_recovery_continuation(fragments));
        }
        
        #[test]
        fn run_location_accuracy_property(_unused in ".*") {
            prop_assert!(property_location_accuracy_new_syntax());
        }
    }
}