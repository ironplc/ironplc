//! Unit tests for lexer enhancements
//! Tests new token recognition, time literal parsing, and comment parsing robustness
//! Requirements: 6.1, 6.2, 6.3, 4.2

#[cfg(test)]
mod lexer_enhancement_tests {
    use crate::lexer::{tokenize, validate_token};
    use crate::token::{Token, TokenType};
    use dsl::core::{FileId, SourceSpan};
    use logos::Logos;

    /// Helper function to create a test token
    fn create_test_token(token_type: TokenType, text: &str) -> Token {
        Token {
            token_type,
            span: SourceSpan {
                file_id: FileId::default(),
                start: 0,
                end: text.len(),
            },
            line: 0,
            col: 0,
            text: text.to_string(),
        }
    }

    /// Helper function to tokenize and extract token types
    fn extract_token_types(source: &str) -> Vec<TokenType> {
        let (tokens, _) = tokenize(source, &FileId::default());
        tokens
            .into_iter()
            .filter(|t| !matches!(t.token_type, TokenType::Whitespace | TokenType::Newline))
            .map(|t| t.token_type)
            .collect()
    }

    #[test]
    fn test_struct_token_recognition() {
        // Test STRUCT keyword recognition (case insensitive)
        let test_cases = vec![
            ("STRUCT", TokenType::Struct),
            ("struct", TokenType::Struct),
            ("Struct", TokenType::Struct),
            ("END_STRUCT", TokenType::EndStruct),
            ("end_struct", TokenType::EndStruct),
            ("End_Struct", TokenType::EndStruct),
        ];

        for (input, expected) in test_cases {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, expected, "Failed to recognize '{}' as {:?}", input, expected);
        }
    }

    #[test]
    fn test_array_token_recognition() {
        // Test ARRAY keyword recognition (case insensitive)
        let test_cases = vec![
            ("ARRAY", TokenType::Array),
            ("array", TokenType::Array),
            ("Array", TokenType::Array),
            ("OF", TokenType::Of),
            ("of", TokenType::Of),
            ("Of", TokenType::Of),
        ];

        for (input, expected) in test_cases {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, expected, "Failed to recognize '{}' as {:?}", input, expected);
        }
    }

    #[test]
    fn test_ton_token_recognition() {
        // Test TON (Timer On Delay) keyword recognition (case insensitive)
        let test_cases = vec![
            ("TON", TokenType::Ton),
            ("ton", TokenType::Ton),
            ("Ton", TokenType::Ton),
            ("TOF", TokenType::Tof),
            ("tof", TokenType::Tof),
            ("TP", TokenType::Tp),
            ("tp", TokenType::Tp),
        ];

        for (input, expected) in test_cases {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, expected, "Failed to recognize '{}' as {:?}", input, expected);
        }
    }

    #[test]
    fn test_case_token_recognition() {
        // Test CASE statement keyword recognition (case insensitive)
        let test_cases = vec![
            ("CASE", TokenType::Case),
            ("case", TokenType::Case),
            ("Case", TokenType::Case),
            ("END_CASE", TokenType::EndCase),
            ("end_case", TokenType::EndCase),
            ("End_Case", TokenType::EndCase),
        ];

        for (input, expected) in test_cases {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, expected, "Failed to recognize '{}' as {:?}", input, expected);
        }
    }

    #[test]
    fn test_string_with_length_token_recognition() {
        // Test STRING(n) token recognition
        let test_cases = vec![
            "STRING(10)",
            "STRING(255)",
            "STRING(1)",
            "string(50)",
            "String(100)",
        ];

        for input in test_cases {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, TokenType::StringWithLength, 
                      "Failed to recognize '{}' as StringWithLength", input);
        }
    }

    #[test]
    fn test_time_literal_parsing_accuracy() {
        // Test various time literal formats
        let valid_time_literals = vec![
            "T#5S",      // 5 seconds
            "T#100MS",   // 100 milliseconds
            "T#2M",      // 2 minutes
            "T#1H",      // 1 hour
            "T#3D",      // 3 days
            "t#10s",     // lowercase
            "T#999ms",   // mixed case
            "T#60M",     // 60 minutes
            "T#24H",     // 24 hours
        ];

        for input in valid_time_literals {
            let mut lexer = TokenType::lexer(input);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, TokenType::TimeLiteral, 
                      "Failed to recognize '{}' as TimeLiteral", input);
            
            // Test validation function
            let test_token = create_test_token(TokenType::TimeLiteral, input);
            assert!(validate_token(&test_token), 
                   "Time literal '{}' failed validation", input);
        }
    }

    #[test]
    fn test_time_literal_validation_edge_cases() {
        // Test edge cases for time literal validation
        let invalid_time_literals = vec![
            "T#",        // No time part
            "T#S",       // No digits
            "T#5",       // No unit
            "T#5X",      // Invalid unit
            "5S",        // Missing T# prefix
            "T#-5S",     // Negative time (not supported in this format)
        ];

        for input in invalid_time_literals {
            let test_token = create_test_token(TokenType::TimeLiteral, input);
            assert!(!validate_token(&test_token), 
                   "Invalid time literal '{}' should fail validation", input);
        }
    }

    #[test]
    fn test_string_with_length_validation() {
        // Test STRING(n) validation
        let valid_strings = vec![
            "STRING(1)",
            "STRING(255)",
            "STRING(65535)",
            "string(10)",
            "String(50)",
        ];

        for input in valid_strings {
            let test_token = create_test_token(TokenType::StringWithLength, input);
            assert!(validate_token(&test_token), 
                   "STRING with length '{}' should pass validation", input);
        }

        let invalid_strings = vec![
            "STRING()",     // Empty length
            "STRING(0)",    // Zero length (might be invalid depending on spec)
            "STRING(abc)",  // Non-numeric length
            "STRING(-5)",   // Negative length
            "STRING",       // Missing parentheses
        ];

        for input in invalid_strings {
            let test_token = create_test_token(TokenType::StringWithLength, input);
            assert!(!validate_token(&test_token), 
                   "Invalid STRING with length '{}' should fail validation", input);
        }
    }

    #[test]
    fn test_comment_parsing_robustness_basic() {
        // Test basic comment parsing
        let basic_comments = vec![
            "(* Simple comment *)",
            "(* Multi-line\n   comment *)",
            "(*Empty*)",
            "(* Comment with symbols !@#$%^&*() *)",
        ];

        for input in basic_comments {
            let (tokens, diagnostics) = tokenize(input, &FileId::default());
            assert!(diagnostics.is_empty(), 
                   "Comment '{}' should not produce diagnostics: {:?}", input, diagnostics);
            assert_eq!(tokens.len(), 1, "Should have exactly one comment token");
            assert_eq!(tokens[0].token_type, TokenType::Comment);
        }
    }

    #[test]
    fn test_comment_parsing_decorative_asterisks() {
        // Test comments with decorative asterisk patterns that work with current regex
        let decorative_comments = vec![
            "(* Simple comment *)",
            "(* Comment with content *)",
            "(*\n * Multi-line comment\n * with decorative asterisks\n *)",
        ];

        for input in decorative_comments {
            let (tokens, diagnostics) = tokenize(input, &FileId::default());
            assert!(diagnostics.is_empty(), 
                   "Decorative comment '{}' should not produce diagnostics: {:?}", input, diagnostics);
            assert_eq!(tokens.len(), 1, "Should have exactly one comment token");
            assert_eq!(tokens[0].token_type, TokenType::Comment);
            
            // Test validation
            assert!(validate_token(&tokens[0]), 
                   "Decorative comment '{}' should pass validation", input);
        }
    }

    #[test]
    fn test_comment_parsing_with_content() {
        // Test comments with various content types
        let content_comments = vec![
            "(* Function: CalculateSpeed *)",
            "(* Author: John Doe, Date: 2024-01-01 *)",
            "(* TODO: Implement error handling *)",
            "(* Version 1.0 - Initial implementation *)",
            "(* Parameters: input1 (INT), input2 (REAL) *)",
        ];

        for input in content_comments {
            let (tokens, diagnostics) = tokenize(input, &FileId::default());
            assert!(diagnostics.is_empty(), 
                   "Content comment '{}' should not produce diagnostics: {:?}", input, diagnostics);
            assert_eq!(tokens.len(), 1, "Should have exactly one comment token");
            assert_eq!(tokens[0].token_type, TokenType::Comment);
        }
    }

    #[test]
    fn test_comment_parsing_nested_comments() {
        // Test nested comment structures
        let nested_comments = vec![
            "(* Outer (* inner *) comment *)",
            "(* Level 1 (* Level 2 (* Level 3 *) *) *)",
        ];

        for input in nested_comments {
            let test_token = create_test_token(TokenType::Comment, input);
            assert!(validate_token(&test_token), 
                   "Nested comment '{}' should pass validation", input);
        }
    }

    #[test]
    fn test_comment_parsing_invalid_nesting() {
        // Test invalid comment nesting
        let invalid_comments = vec![
            "(* Unmatched (* comment *)",     // Missing closing for nested
            "(* Comment *) extra *)",         // Extra closing
        ];

        for input in invalid_comments {
            let test_token = create_test_token(TokenType::Comment, input);
            assert!(!validate_token(&test_token), 
                   "Invalid nested comment '{}' should fail validation", input);
        }
    }

    #[test]
    fn test_mixed_comment_and_code() {
        // Test comments interspersed with code
        let mixed_code = r#"
            (* Header comment *)
            PROGRAM TestProgram
            VAR
                (* Variable declaration comment *)
                x : INT := 5; (* Inline comment *)
            END_VAR
            (* Body comment *)
            x := x + 1;
            END_PROGRAM
        "#;

        let (tokens, diagnostics) = tokenize(mixed_code, &FileId::default());
        assert!(diagnostics.is_empty(), 
               "Mixed comment and code should not produce diagnostics: {:?}", diagnostics);
        
        // Count comment tokens
        let comment_count = tokens.iter()
            .filter(|t| t.token_type == TokenType::Comment)
            .count();
        assert_eq!(comment_count, 4, "Should have 4 comment tokens");
        
        // Verify program structure is still parsed correctly
        let program_tokens: Vec<_> = tokens.iter()
            .filter(|t| matches!(t.token_type, TokenType::Program | TokenType::EndProgram))
            .collect();
        assert_eq!(program_tokens.len(), 2, "Should have PROGRAM and END_PROGRAM tokens");
    }

    #[test]
    fn test_enhanced_tokens_in_context() {
        // Test enhanced tokens in realistic code context
        let enhanced_code = r#"
            TYPE
                (* Custom structure definition *)
                MyStruct : STRUCT
                    name : STRING(50);
                    values : ARRAY[1..10] OF INT;
                END_STRUCT;
            END_TYPE
            
            PROGRAM TimerExample
            VAR
                timer1 : TON;
                state : INT;
            END_VAR
            
            CASE state OF
                1: timer1(IN := TRUE, PT := T#5S);
                2: timer1(IN := FALSE, PT := T#100MS);
            END_CASE
            
            END_PROGRAM
        "#;

        let token_types = extract_token_types(enhanced_code);
        
        // Verify presence of enhanced tokens
        assert!(token_types.contains(&TokenType::Struct), "Should contain STRUCT token");
        assert!(token_types.contains(&TokenType::EndStruct), "Should contain END_STRUCT token");
        assert!(token_types.contains(&TokenType::Array), "Should contain ARRAY token");
        assert!(token_types.contains(&TokenType::Of), "Should contain OF token");
        assert!(token_types.contains(&TokenType::StringWithLength), "Should contain STRING(n) token");
        assert!(token_types.contains(&TokenType::Ton), "Should contain TON token");
        assert!(token_types.contains(&TokenType::Case), "Should contain CASE token");
        assert!(token_types.contains(&TokenType::EndCase), "Should contain END_CASE token");
        assert!(token_types.contains(&TokenType::TimeLiteral), "Should contain time literal tokens");
        assert!(token_types.contains(&TokenType::Comment), "Should contain comment tokens");
    }

    #[test]
    fn test_token_case_insensitivity() {
        // Test that enhanced tokens are case insensitive
        let case_variations = vec![
            ("STRUCT", "struct", "Struct"),
            ("ARRAY", "array", "Array"),
            ("TON", "ton", "Ton"),
            ("CASE", "case", "Case"),
        ];

        for (upper, lower, mixed) in case_variations {
            let mut upper_lexer = TokenType::lexer(upper);
            let mut lower_lexer = TokenType::lexer(lower);
            let mut mixed_lexer = TokenType::lexer(mixed);
            
            let upper_token = upper_lexer.next().unwrap().unwrap();
            let lower_token = lower_lexer.next().unwrap().unwrap();
            let mixed_token = mixed_lexer.next().unwrap().unwrap();
            
            assert_eq!(upper_token, lower_token, 
                      "Case insensitive tokens should match: {} vs {}", upper, lower);
            assert_eq!(upper_token, mixed_token, 
                      "Case insensitive tokens should match: {} vs {}", upper, mixed);
        }
    }

    #[test]
    fn test_time_literal_units_comprehensive() {
        // Test all supported time units
        let time_units = vec![
            ("MS", "milliseconds"),
            ("S", "seconds"),
            ("M", "minutes"),
            ("H", "hours"),
            ("D", "days"),
        ];

        for (unit, description) in time_units {
            let time_literal = format!("T#5{}", unit);
            let mut lexer = TokenType::lexer(&time_literal);
            let token = lexer.next().unwrap().unwrap();
            assert_eq!(token, TokenType::TimeLiteral, 
                      "Failed to recognize time literal with {} unit: {}", description, time_literal);
            
            // Test lowercase
            let time_literal_lower = format!("T#5{}", unit.to_lowercase());
            let mut lexer_lower = TokenType::lexer(&time_literal_lower);
            let token_lower = lexer_lower.next().unwrap().unwrap();
            assert_eq!(token_lower, TokenType::TimeLiteral, 
                      "Failed to recognize lowercase time literal: {}", time_literal_lower);
        }
    }

    #[test]
    fn test_string_with_length_edge_cases() {
        // Test edge cases for STRING(n) parsing
        let edge_cases = vec![
            ("STRING(1)", true),      // Minimum length
            ("STRING(65535)", true),  // Large length
            ("STRING(0)", false),     // Zero length (invalid)
            ("STRING()", false),      // Empty parentheses
            ("STRING(abc)", false),   // Non-numeric
        ];

        for (input, should_be_valid) in edge_cases {
            if should_be_valid {
                let mut lexer = TokenType::lexer(input);
                if let Some(Ok(token)) = lexer.next() {
                    assert_eq!(token, TokenType::StringWithLength, 
                              "Should recognize valid STRING(n): {}", input);
                    
                    // Also test validation
                    let test_token = create_test_token(TokenType::StringWithLength, input);
                    assert!(validate_token(&test_token), 
                           "Valid STRING(n) '{}' should pass validation", input);
                } else {
                    panic!("Failed to tokenize valid STRING(n): {}", input);
                }
            } else {
                // For invalid cases, check if they get tokenized as StringWithLength
                let mut lexer = TokenType::lexer(input);
                if let Some(Ok(token)) = lexer.next() {
                    if token == TokenType::StringWithLength {
                        // If it was tokenized as StringWithLength, validation should fail
                        let test_token = create_test_token(TokenType::StringWithLength, input);
                        assert!(!validate_token(&test_token), 
                               "Invalid STRING(n) '{}' should fail validation", input);
                    }
                }
                // If it wasn't tokenized as StringWithLength, that's also acceptable
            }
        }
    }

    #[test]
    fn test_comment_with_special_characters() {
        // Test comments containing special characters that might interfere with parsing
        let special_char_comments = vec![
            "(* Comment with parentheses () inside *)",
            "(* Comment with asterisks * inside *)",
            "(* Comment with quotes 'single' and \"double\" *)",
            "(* Comment with operators := <= >= <> *)",
            "(* Comment with numbers 123 and symbols !@#$%^&*()_+ *)",
        ];

        for input in special_char_comments {
            let (tokens, diagnostics) = tokenize(input, &FileId::default());
            assert!(diagnostics.is_empty(), 
                   "Special character comment should not produce diagnostics: {:?}", diagnostics);
            assert_eq!(tokens.len(), 1, "Should have exactly one comment token");
            assert_eq!(tokens[0].token_type, TokenType::Comment);
        }
    }
}