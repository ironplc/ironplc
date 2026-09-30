//! Direct addresses (`%IX0.0`, `%MW10`, ...): the lexer takes the whole
//! address as one token, and the parser builds its fields.

use super::common::*;
use crate::token::{Token, TokenType};
use dsl::common::{LocationPrefix, SizePrefix};

fn tokens_of(source: &str) -> Vec<Token> {
    let (tokens, diagnostics) = crate::tokenize_program(
        source,
        &FileId::default(),
        &CompilerOptions::default(),
        0,
        0,
    );
    assert!(diagnostics.is_empty(), "diagnostics = {diagnostics:?}");
    tokens
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace)
        .collect()
}

#[rstest]
#[case("%MW10")]
#[case("%IX12.7")]
#[case("%QD100")]
#[case("%IX1.2.3.4")]
#[case("%QL4294967295")]
#[case("%I0")]
#[case("%mw10")]
#[case("%MW1_000")]
fn tokenize_when_direct_address_has_multi_digit_fields_then_one_token(#[case] address: &str) {
    let tokens = tokens_of(address);

    assert_eq!(tokens.len(), 1, "tokens = {tokens:?}");
    assert_eq!(tokens[0].token_type, TokenType::DirectAddress);
    assert_eq!(tokens[0].text, address);
}

#[rstest]
#[case("%MW10:INT", "%MW10", TokenType::Colon)]
#[case("%IX1.2.a", "%IX1.2", TokenType::Period)]
#[case("%QW12;", "%QW12", TokenType::Semicolon)]
#[case("%MW10_", "%MW10", TokenType::Identifier)]
fn tokenize_when_direct_address_is_followed_by_token_then_does_not_swallow_it(
    #[case] source: &str,
    #[case] address: &str,
    #[case] next: TokenType,
) {
    let tokens = tokens_of(source);

    assert_eq!(tokens[0].token_type, TokenType::DirectAddress);
    assert_eq!(tokens[0].text, address);
    assert_eq!(tokens[1].token_type, next);
}

#[test]
fn parse_when_located_var_has_multi_digit_address_then_address_fields() {
    let lib = parse_text(
        "PROGRAM main
VAR
    speed_setpoint AT %MW10 : INT;
    limit AT %IX12.7 : BOOL;
END_VAR
END_PROGRAM",
    );

    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let speed = cast!(&prog.variables[0].identifier, VariableIdentifier::Direct);
    assert_eq!(speed.address_assignment.location, LocationPrefix::M);
    assert_eq!(speed.address_assignment.size, SizePrefix::W);
    assert_eq!(speed.address_assignment.address, vec![10]);
    let limit = cast!(&prog.variables[1].identifier, VariableIdentifier::Direct);
    assert_eq!(limit.address_assignment.address, vec![12, 7]);
}

#[test]
fn parse_when_direct_address_field_overflows_then_syntax_error() {
    let result = parse_program(
        "PROGRAM main
VAR
    x AT %MW4294967296 : INT;
END_VAR
END_PROGRAM",
        &FileId::default(),
        &CompilerOptions::default(),
    );

    assert!(result.is_err());
}
