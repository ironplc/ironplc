//! Partial-access (`%X`, `%B`, `%W`, `%D`, `%L`) syntax parsing.

use super::common::*;
use spec_test_macro::spec_test;

/// REQ-PAB-parser-001: The lexer tokenizes `%X<digits>` as `PartialAccessBit`.
#[spec_test(REQ_PAB_parser_001)]
fn lexer_spec_req_pab_001_percent_x_digits_tokenizes_as_partial_access_bit() {
    use crate::token::TokenType;
    let (tokens, _) = crate::tokenize_program(
        "PROGRAM p VAR b : BYTE; END_VAR b.%X3; END_PROGRAM",
        &FileId::default(),
        &opts_with_partial_access(),
        0,
        0,
    );
    let uppercase_hit = tokens
        .iter()
        .any(|t| t.token_type == TokenType::PartialAccessBit && t.text == "%X3");
    assert!(uppercase_hit, "tokens = {:?}", tokens);

    // Case insensitivity.
    let (tokens_lower, _) = crate::tokenize_program(
        "PROGRAM p VAR b : BYTE; END_VAR b.%x3; END_PROGRAM",
        &FileId::default(),
        &opts_with_partial_access(),
        0,
        0,
    );
    assert!(tokens_lower
        .iter()
        .any(|t| t.token_type == TokenType::PartialAccessBit && t.text == "%x3"));
}

/// REQ-PAB-parser-002: `%X0` does not cannibalize `DirectAddress` tokens like `%IX0.0`.
#[spec_test(REQ_PAB_parser_002)]
fn lexer_spec_req_pab_002_direct_address_still_takes_precedence() {
    use crate::token::TokenType;
    let (tokens, _) = crate::tokenize_program(
        "PROGRAM p VAR x AT %IX0.0 : BOOL; END_VAR END_PROGRAM",
        &FileId::default(),
        &opts_with_partial_access(),
        0,
        0,
    );
    assert!(tokens
        .iter()
        .any(|t| t.token_type == TokenType::DirectAddress && t.text == "%IX0.0"));
    assert!(!tokens
        .iter()
        .any(|t| t.token_type == TokenType::PartialAccessBit));
}

/// REQ-PAB-parser-100: The lexer tokenizes `%B`, `%W`, `%D` and `%L` followed by
/// digits as the byte, word, dword and lword partial-access selectors,
/// case-insensitive.
#[spec_test(REQ_PAB_parser_100)]
fn lexer_spec_req_pab_100_percent_b_w_d_l_digits_tokenize_as_partial_access_selectors() {
    use crate::token::TokenType;
    let cases = [
        ("%B1", TokenType::PartialAccessByte),
        ("%w1", TokenType::PartialAccessWord),
        ("%D1", TokenType::PartialAccessDWord),
        ("%l0", TokenType::PartialAccessLWord),
    ];
    for (selector, expected) in cases {
        let source = format!("PROGRAM p VAR l : LWORD; END_VAR l.{selector}; END_PROGRAM");
        let (tokens, _) = crate::tokenize_program(
            &source,
            &FileId::default(),
            &opts_with_partial_access(),
            0,
            0,
        );
        assert!(
            tokens
                .iter()
                .any(|t| t.token_type == expected && t.text == selector),
            "{selector}: tokens = {:?}",
            tokens
        );
    }
}

/// REQ-PAB-parser-101: the wider selectors are accepted wherever `.%Xn` is:
/// after a simple variable, an array subscript and a structure field.
#[spec_test(REQ_PAB_parser_101)]
#[rstest]
#[case::simple_var("r := b.%B0;")]
#[case::array_element("r := arr[0].%W0;")]
#[case::struct_field("r := s.f.%D0;")]
fn parser_spec_req_pab_101_wider_selectors_accepted_in_every_position(#[case] body: &str) {
    let src = wrap_program(body);
    let result = parse_program(&src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-parser-110: a wider selector lowers to `PartialAccessVariable`
/// carrying its size and index, a node distinct from `BitAccessVariable`.
#[spec_test(REQ_PAB_parser_110)]
#[test]
fn parser_spec_req_pab_110_wider_selector_lowers_to_partial_access_variable() {
    let src = wrap_program("r := b.%W1;");
    let library = parse_program(&src, &FileId::default(), &opts_with_partial_access()).unwrap();
    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    let statements = cast!(&program.body, FunctionBlockBodyKind::Statements);
    let assignment = cast!(&statements.body[0], StmtKind::Assignment);
    let variable = cast!(&assignment.value.kind, ExprKind::Variable);
    let symbolic = cast!(variable, Variable::Symbolic);
    let partial = cast!(symbolic, SymbolicVariableKind::PartialAccess);
    assert_eq!(partial.size, PartialAccessSize::Word);
    assert_eq!(partial.index.value, 1);
}

/// REQ-PAB-parser-010: `.%Xn` is accepted on a simple variable.
#[spec_test(REQ_PAB_parser_010)]
fn parser_spec_req_pab_010_dot_percent_x_accepted_on_simple_var() {
    let src = wrap_program("r := b.%X0;");
    let result = parse_program(&src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-parser-011: `.%Xn` is accepted after an array subscript — the user's case.
#[spec_test(REQ_PAB_parser_011)]
fn parser_spec_req_pab_011_dot_percent_x_accepted_after_array_subscript() {
    let src = wrap_program("r := arr[0].%X0;");
    let result = parse_program(&src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-parser-012: `.%Xn` is accepted after a struct field access.
#[spec_test(REQ_PAB_parser_012)]
fn parser_spec_req_pab_012_dot_percent_x_accepted_after_struct_field() {
    // Define MY_STRUCT with a BYTE field so the program type-checks later,
    // though this test only exercises the parser surface.
    let src = "
TYPE MY_STRUCT : STRUCT f : BYTE; END_STRUCT; END_TYPE
PROGRAM main
VAR
  s : MY_STRUCT;
  r : BOOL;
END_VAR
  r := s.f.%X0;
END_PROGRAM
";
    let result = parse_program(src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-parser-020: `x.%Xn` and `x.n` produce equal AST subtrees.
#[spec_test(REQ_PAB_parser_020)]
fn parser_spec_req_pab_020_dot_percent_x_and_dot_n_produce_equal_ast() {
    let long = wrap_program("r := b.%X3;");
    let short = wrap_program("r := b.3;");
    let long_lib = parse_program(&long, &FileId::default(), &opts_with_partial_access()).unwrap();
    let short_lib = parse_program(&short, &FileId::default(), &CompilerOptions::default()).unwrap();

    // Dig out the single assignment's RHS symbolic variable from each AST
    // and compare structurally.
    let long_prog = cast!(
        &long_lib.elements[0],
        LibraryElementKind::ProgramDeclaration
    );
    let short_prog = cast!(
        &short_lib.elements[0],
        LibraryElementKind::ProgramDeclaration
    );
    assert_eq!(long_prog.body, short_prog.body);
}

/// REQ-PAB-parser-050: When the flag is off, `.%Xn` produces
/// `PartialAccessSyntaxDisabled` (P4033) — not a lexer-level P0003.
#[spec_test(REQ_PAB_parser_050)]
fn parser_spec_req_pab_050_disabled_flag_produces_partial_access_syntax_disabled() {
    let src = wrap_program("r := b.%X0;");
    let result = parse_program(&src, &FileId::default(), &CompilerOptions::default());
    match result {
        Ok(_) => panic!("expected error, got Ok"),
        Err(d) => {
            assert_eq!(
                d.code,
                "P4033",
                "expected P4033 PartialAccessSyntaxDisabled, got {}: {}",
                d.code,
                d.description(),
            );
        }
    }
}
