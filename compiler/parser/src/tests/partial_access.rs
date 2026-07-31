//! Partial-access (`%X`) syntax parsing.

use super::common::*;

/// REQ-PAB-001: The lexer tokenizes `%X<digits>` as `PartialAccessBit`.
#[test]
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

/// REQ-PAB-002: `%X0` does not cannibalize `DirectAddress` tokens like `%IX0.0`.
#[test]
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

/// REQ-PAB-010: `.%Xn` is accepted on a simple variable.
#[test]
fn parser_spec_req_pab_010_dot_percent_x_accepted_on_simple_var() {
    let src = wrap_program("r := b.%X0;");
    let result = parse_program(&src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-011: `.%Xn` is accepted after an array subscript — the user's case.
#[test]
fn parser_spec_req_pab_011_dot_percent_x_accepted_after_array_subscript() {
    let src = wrap_program("r := arr[0].%X0;");
    let result = parse_program(&src, &FileId::default(), &opts_with_partial_access());
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

/// REQ-PAB-012: `.%Xn` is accepted after a struct field access.
#[test]
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

/// REQ-PAB-020: `x.%Xn` and `x.n` produce equal AST subtrees.
#[test]
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

/// REQ-PAB-050: When the flag is off, `.%Xn` produces
/// `PartialAccessSyntaxDisabled` (P4033) — not a lexer-level P0003.
#[test]
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
