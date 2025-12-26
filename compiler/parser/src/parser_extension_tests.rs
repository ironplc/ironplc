//! Unit tests for parser extensions
//! Tests STRUCT, ARRAY, STRING(n), and CASE statement parsing
//! Requirements: 1.1, 2.1, 3.1, 5.1

#[cfg(test)]
mod parser_extension_tests {
    use crate::{parse_program, options::ParseOptions};
    use dsl::core::FileId;
    use dsl::common::{LibraryElementKind, DataTypeDeclarationKind};

    /// Helper function to parse and expect success
    fn parse_and_expect_success(source: &str) -> dsl::common::Library {
        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        result.unwrap()
    }

    /// Helper function to parse and expect failure
    fn parse_and_expect_failure(source: &str) -> dsl::diagnostic::Diagnostic {
        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_err(), "Parse should have failed but succeeded");
        result.err().unwrap()
    }

    #[test]
    fn test_struct_type_definition_parsing_basic() {
        // Test basic STRUCT type definition
        let source = r#"
            TYPE
                MyStruct : STRUCT
                    field1 : BOOL;
                    field2 : INT;
                END_STRUCT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(struct_decl)) = &library.elements[0] {
            assert_eq!(struct_decl.type_name.name.original, "MyStruct");
            assert_eq!(struct_decl.elements.len(), 2);
            
            // Check first field
            assert_eq!(struct_decl.elements[0].name.original, "field1");
            // Check second field
            assert_eq!(struct_decl.elements[1].name.original, "field2");
        } else {
            panic!("Expected STRUCT declaration, got: {:?}", library.elements[0]);
        }
    }

    #[test]
    fn test_struct_type_definition_parsing_complex() {
        // Test complex STRUCT with multiple field types
        let source = r#"
            TYPE
                ComplexStruct : STRUCT
                    flag : BOOL;
                    counter : INT;
                    temperature : REAL;
                    name : STRING;
                    data : ARRAY[1..5] OF DINT;
                END_STRUCT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(struct_decl)) = &library.elements[0] {
            assert_eq!(struct_decl.type_name.name.original, "ComplexStruct");
            assert_eq!(struct_decl.elements.len(), 5);
            
            // Verify field names
            let field_names: Vec<_> = struct_decl.elements.iter()
                .map(|e| e.name.original.as_str())
                .collect();
            assert_eq!(field_names, vec!["flag", "counter", "temperature", "name", "data"]);
        } else {
            panic!("Expected STRUCT declaration, got: {:?}", library.elements[0]);
        }
    }

    #[test]
    fn test_struct_type_definition_parsing_nested() {
        // Test nested STRUCT definitions
        let source = r#"
            TYPE
                InnerStruct : STRUCT
                    value : INT;
                END_STRUCT;
                
                OuterStruct : STRUCT
                    inner : InnerStruct;
                    flag : BOOL;
                END_STRUCT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 2);

        // Check first struct (InnerStruct)
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(inner_struct)) = &library.elements[0] {
            assert_eq!(inner_struct.type_name.name.original, "InnerStruct");
            assert_eq!(inner_struct.elements.len(), 1);
        } else {
            panic!("Expected first STRUCT declaration");
        }

        // Check second struct (OuterStruct)
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(outer_struct)) = &library.elements[1] {
            assert_eq!(outer_struct.type_name.name.original, "OuterStruct");
            assert_eq!(outer_struct.elements.len(), 2);
        } else {
            panic!("Expected second STRUCT declaration");
        }
    }

    #[test]
    fn test_struct_type_definition_parsing_empty() {
        // Test STRUCT with minimal content (single field)
        let source = r#"
            TYPE
                MinimalStruct : STRUCT
                    dummy : BOOL;
                END_STRUCT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(struct_decl)) = &library.elements[0] {
            assert_eq!(struct_decl.type_name.name.original, "MinimalStruct");
            assert_eq!(struct_decl.elements.len(), 1);
            assert_eq!(struct_decl.elements[0].name.original, "dummy");
        } else {
            panic!("Expected STRUCT declaration, got: {:?}", library.elements[0]);
        }
    }

    #[test]
    fn test_array_declaration_parsing_basic() {
        // Test basic ARRAY declaration
        let source = r#"
            TYPE
                IntArray : ARRAY[1..10] OF INT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(array_decl)) = &library.elements[0] {
            assert_eq!(array_decl.type_name.name.original, "IntArray");
            
            // Check array specification
            if let dsl::common::ArraySpecificationKind::Subranges(subranges) = &array_decl.spec {
                assert_eq!(subranges.ranges.len(), 1);
                
                // Check array bounds
                let range = &subranges.ranges[0];
                assert_eq!(range.start.value.value, 1);
                assert_eq!(range.end.value.value, 10);
            } else {
                panic!("Expected ArraySpecificationKind::Subranges");
            }
        } else {
            panic!("Expected ARRAY declaration, got: {:?}", library.elements[0]);
        }
    }

    #[test]
    fn test_array_declaration_parsing_various_bounds() {
        // Test ARRAY declarations with various bounds
        let test_cases = vec![
            ("ARRAY[0..9] OF BOOL", 0, 9),
            ("ARRAY[1..100] OF INT", 1, 100),
            ("ARRAY[-5..5] OF REAL", 5, 5), // Note: negative values stored as positive with is_neg flag
        ];

        for (array_spec, expected_start, expected_end) in test_cases {
            let source = format!(r#"
                TYPE
                    TestArray : {};
                END_TYPE
            "#, array_spec);

            let library = parse_and_expect_success(&source);
            assert_eq!(library.elements.len(), 1);

            if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(array_decl)) = &library.elements[0] {
                assert_eq!(array_decl.type_name.name.original, "TestArray");
                
                if let dsl::common::ArraySpecificationKind::Subranges(subranges) = &array_decl.spec {
                    assert_eq!(subranges.ranges.len(), 1);
                    
                    let range = &subranges.ranges[0];
                    assert_eq!(range.start.value.value, expected_start, 
                              "Start bound mismatch for {}", array_spec);
                    assert_eq!(range.end.value.value, expected_end, 
                              "End bound mismatch for {}", array_spec);
                } else {
                    panic!("Expected ArraySpecificationKind::Subranges for {}", array_spec);
                }
            } else {
                panic!("Expected ARRAY declaration for {}", array_spec);
            }
        }
    }

    #[test]
    fn test_array_declaration_parsing_multidimensional() {
        // Test multi-dimensional ARRAY declarations
        let source = r#"
            TYPE
                Matrix : ARRAY[1..5, 1..3] OF REAL;
                Cube : ARRAY[0..9, 0..9, 0..9] OF INT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 2);

        // Check 2D array (Matrix)
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(matrix_decl)) = &library.elements[0] {
            assert_eq!(matrix_decl.type_name.name.original, "Matrix");
            
            if let dsl::common::ArraySpecificationKind::Subranges(subranges) = &matrix_decl.spec {
                assert_eq!(subranges.ranges.len(), 2);
                
                // First dimension: 1..5
                assert_eq!(subranges.ranges[0].start.value.value, 1);
                assert_eq!(subranges.ranges[0].end.value.value, 5);
                
                // Second dimension: 1..3
                assert_eq!(subranges.ranges[1].start.value.value, 1);
                assert_eq!(subranges.ranges[1].end.value.value, 3);
            } else {
                panic!("Expected ArraySpecificationKind::Subranges for Matrix");
            }
        } else {
            panic!("Expected Matrix ARRAY declaration");
        }

        // Check 3D array (Cube)
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(cube_decl)) = &library.elements[1] {
            assert_eq!(cube_decl.type_name.name.original, "Cube");
            
            if let dsl::common::ArraySpecificationKind::Subranges(subranges) = &cube_decl.spec {
                assert_eq!(subranges.ranges.len(), 3);
                
                // All dimensions: 0..9
                for i in 0..3 {
                    assert_eq!(subranges.ranges[i].start.value.value, 0);
                    assert_eq!(subranges.ranges[i].end.value.value, 9);
                }
            } else {
                panic!("Expected ArraySpecificationKind::Subranges for Cube");
            }
        } else {
            panic!("Expected Cube ARRAY declaration");
        }
    }

    #[test]
    fn test_array_declaration_parsing_different_types() {
        // Test ARRAY declarations with different element types
        let element_types = vec![
            "BOOL", "INT", "DINT", "REAL", "STRING", "BYTE", "WORD", "DWORD"
        ];

        for element_type in element_types {
            let source = format!(r#"
                TYPE
                    TestArray : ARRAY[1..10] OF {};
                END_TYPE
            "#, element_type);

            let library = parse_and_expect_success(&source);
            assert_eq!(library.elements.len(), 1);

            if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(array_decl)) = &library.elements[0] {
                assert_eq!(array_decl.type_name.name.original, "TestArray");
                // The element type is stored in the array specification
                // We just verify the parsing succeeded
            } else {
                panic!("Expected ARRAY declaration for element type {}", element_type);
            }
        }
    }

    #[test]
    fn test_string_with_length_parsing() {
        // Test STRING(n) length specification parsing
        let source = r#"
            PROGRAM TestProgram
            VAR
                short_string : STRING(10);
                medium_string : STRING(50);
                long_string : STRING(255);
            END_VAR
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            assert_eq!(program_decl.variables.len(), 3);
            
            // Check variable names
            let var_names: Vec<_> = program_decl.variables.iter()
                .filter_map(|v| v.identifier.symbolic_id())
                .map(|id| id.original.as_str())
                .collect();
            assert_eq!(var_names, vec!["short_string", "medium_string", "long_string"]);
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_string_with_length_parsing_in_struct() {
        // Test STRING(n) in STRUCT context
        let source = r#"
            TYPE
                PersonInfo : STRUCT
                    first_name : STRING(20);
                    last_name : STRING(30);
                    address : STRING(100);
                END_STRUCT;
            END_TYPE
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(struct_decl)) = &library.elements[0] {
            assert_eq!(struct_decl.type_name.name.original, "PersonInfo");
            assert_eq!(struct_decl.elements.len(), 3);
            
            // Check field names
            let field_names: Vec<_> = struct_decl.elements.iter()
                .map(|e| e.name.original.as_str())
                .collect();
            assert_eq!(field_names, vec!["first_name", "last_name", "address"]);
        } else {
            panic!("Expected STRUCT declaration");
        }
    }

    #[test]
    fn test_case_statement_parsing_basic() {
        // Test basic CASE statement parsing
        let source = r#"
            PROGRAM TestProgram
            VAR
                state : INT := 1;
                result : INT;
            END_VAR
            
            CASE state OF
                1: result := 10;
                2: result := 20;
                3: result := 30;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            
            // Check that the program body contains statements (including CASE)
            if let dsl::common::FunctionBlockBodyKind::Statements(statements) = &program_decl.body {
                // We expect at least one statement (the CASE statement)
                assert!(!statements.body.is_empty(), "Program should have statements including CASE");
                
                // Look for CASE statement in the statements
                let has_case = statements.body.iter().any(|stmt| {
                    matches!(stmt, dsl::textual::StmtKind::Case(_))
                });
                assert!(has_case, "Should contain a CASE statement");
            } else {
                panic!("Expected statements in program body");
            }
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_case_statement_parsing_with_else() {
        // Test CASE statement with ELSE clause
        let source = r#"
            PROGRAM TestProgram
            VAR
                state : INT := 1;
                result : INT;
            END_VAR
            
            CASE state OF
                1: result := 10;
                2: result := 20;
                ELSE
                    result := 0;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            
            // Check that the program body contains statements
            if let dsl::common::FunctionBlockBodyKind::Statements(statements) = &program_decl.body {
                assert!(!statements.body.is_empty(), "Program should have statements");
                
                // Look for CASE statement
                let has_case = statements.body.iter().any(|stmt| {
                    matches!(stmt, dsl::textual::StmtKind::Case(_))
                });
                assert!(has_case, "Should contain a CASE statement with ELSE");
            } else {
                panic!("Expected statements in program body");
            }
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_case_statement_parsing_multiple_labels() {
        // Test CASE statement with multiple labels per case
        let source = r#"
            PROGRAM TestProgram
            VAR
                state : INT := 1;
                result : INT;
            END_VAR
            
            CASE state OF
                1, 2, 3: result := 10;
                4, 5: result := 20;
                ELSE
                    result := 0;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            
            // Check that parsing succeeded - detailed AST verification would require
            // more complex matching, but successful parsing indicates the syntax is handled
            if let dsl::common::FunctionBlockBodyKind::Statements(statements) = &program_decl.body {
                assert!(!statements.body.is_empty(), "Program should have statements");
            } else {
                panic!("Expected statements in program body");
            }
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_case_statement_parsing_nested() {
        // Test nested CASE statements
        let source = r#"
            PROGRAM TestProgram
            VAR
                outer_state : INT := 1;
                inner_state : INT := 1;
                result : INT;
            END_VAR
            
            CASE outer_state OF
                1: 
                    CASE inner_state OF
                        1: result := 11;
                        2: result := 12;
                    END_CASE
                2: result := 20;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            
            // Verify parsing succeeded for nested CASE
            if let dsl::common::FunctionBlockBodyKind::Statements(statements) = &program_decl.body {
                assert!(!statements.body.is_empty(), "Program should have statements");
            } else {
                panic!("Expected statements in program body");
            }
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_case_statement_parsing_with_complex_expressions() {
        // Test CASE statement with complex case expressions
        let source = r#"
            PROGRAM TestProgram
            VAR
                state : INT := 1;
                result : INT;
            END_VAR
            
            CASE state OF
                1: 
                    result := 10;
                    result := result + 5;
                2: 
                    result := 20;
                    IF result > 15 THEN
                        result := result * 2;
                    END_IF
                ELSE
                    result := 0;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 1);

        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[0] {
            assert_eq!(program_decl.name.original, "TestProgram");
            
            // Verify complex case bodies are parsed
            if let dsl::common::FunctionBlockBodyKind::Statements(statements) = &program_decl.body {
                assert!(!statements.body.is_empty(), "Program should have statements");
            } else {
                panic!("Expected statements in program body");
            }
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_mixed_enhanced_syntax() {
        // Test mixing all enhanced syntax features
        let source = r#"
            TYPE
                ControlData : STRUCT
                    name : STRING(50);
                    values : ARRAY[1..10] OF INT;
                    flags : ARRAY[0..7] OF BOOL;
                END_STRUCT;
            END_TYPE
            
            PROGRAM MixedSyntaxTest
            VAR
                data : ControlData;
                state : INT := 1;
                timer1 : TON;
            END_VAR
            
            (* Initialize data *)
            data.name := 'Test System';
            
            CASE state OF
                1: 
                    data.values[1] := 100;
                    timer1(IN := TRUE, PT := T#5S);
                2:
                    data.flags[0] := TRUE;
                    timer1(IN := FALSE, PT := T#1S);
                ELSE
                    state := 1;
            END_CASE
            
            END_PROGRAM
        "#;

        let library = parse_and_expect_success(source);
        assert_eq!(library.elements.len(), 2);

        // Check STRUCT declaration
        if let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(struct_decl)) = &library.elements[0] {
            assert_eq!(struct_decl.type_name.name.original, "ControlData");
            assert_eq!(struct_decl.elements.len(), 3);
        } else {
            panic!("Expected STRUCT declaration");
        }

        // Check PROGRAM declaration
        if let LibraryElementKind::ProgramDeclaration(program_decl) = &library.elements[1] {
            assert_eq!(program_decl.name.original, "MixedSyntaxTest");
            assert_eq!(program_decl.variables.len(), 3);
        } else {
            panic!("Expected PROGRAM declaration");
        }
    }

    #[test]
    fn test_invalid_struct_syntax() {
        // Test invalid STRUCT syntax
        let invalid_cases = vec![
            // Missing END_STRUCT
            r#"
                TYPE
                    BadStruct : STRUCT
                        field : INT;
                END_TYPE
            "#,
            // Missing field type
            r#"
                TYPE
                    BadStruct : STRUCT
                        field;
                    END_STRUCT;
                END_TYPE
            "#,
        ];

        for invalid_source in invalid_cases {
            let _error = parse_and_expect_failure(invalid_source);
            // We expect parsing to fail for invalid syntax
        }
    }

    #[test]
    fn test_invalid_array_syntax() {
        // Test invalid ARRAY syntax
        let invalid_cases = vec![
            // Missing bounds
            r#"
                TYPE
                    BadArray : ARRAY OF INT;
                END_TYPE
            "#,
            // Invalid bounds syntax
            r#"
                TYPE
                    BadArray : ARRAY[1,10] OF INT;
                END_TYPE
            "#,
        ];

        for invalid_source in invalid_cases {
            let _error = parse_and_expect_failure(invalid_source);
            // We expect parsing to fail for invalid syntax
        }
    }

    #[test]
    fn test_invalid_case_syntax() {
        // Test invalid CASE syntax
        let invalid_cases = vec![
            // Missing END_CASE
            r#"
                PROGRAM TestProgram
                VAR state : INT; END_VAR
                CASE state OF
                    1: state := 2;
                END_PROGRAM
            "#,
            // Missing OF keyword
            r#"
                PROGRAM TestProgram
                VAR state : INT; END_VAR
                CASE state
                    1: state := 2;
                END_CASE
                END_PROGRAM
            "#,
        ];

        for invalid_source in invalid_cases {
            let _error = parse_and_expect_failure(invalid_source);
            // We expect parsing to fail for invalid syntax
        }
    }
}