//! Direct addresses whose fields have more than one digit (`%MW10`,
//! `%IX12.7`): the lexer takes the whole address as one token, and the
//! parser reads each field's value.

use super::common::*;
use crate::token::{Token, TokenType};

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
#[case("%MW10", LocationPrefix::M, SizePrefix::W, &[10])]
#[case("%IX12.7", LocationPrefix::I, SizePrefix::X, &[12, 7])]
#[case("%QD100", LocationPrefix::Q, SizePrefix::D, &[100])]
#[case("%IX1.2.3.4", LocationPrefix::I, SizePrefix::X, &[1, 2, 3, 4])]
#[case("%I10", LocationPrefix::I, SizePrefix::Nil, &[10])]
#[case("%ix0.0", LocationPrefix::I, SizePrefix::X, &[0, 0])]
#[case("%MW1_000", LocationPrefix::M, SizePrefix::W, &[1000])]
fn parse_when_located_var_has_direct_address_then_address_fields(
    #[case] address: &str,
    #[case] location: LocationPrefix,
    #[case] size: SizePrefix,
    #[case] fields: &[u32],
) {
    let source = format!(
        "PROGRAM main
VAR
    x AT {address} : INT;
END_VAR
END_PROGRAM"
    );
    let lib = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();

    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let direct = cast!(&prog.variables[0].identifier, VariableIdentifier::Direct);
    assert_eq!(direct.address_assignment.location, location);
    assert_eq!(direct.address_assignment.size, size);
    assert_eq!(direct.address_assignment.address, fields);
    let span = &direct.address_assignment.position;
    assert_eq!(&source[span.start..span.end], address);
}

#[rstest]
#[case("%MW10:INT", "%MW10", TokenType::Colon)]
#[case("%IX1.2.a", "%IX1.2", TokenType::Period)]
#[case("%QW12;", "%QW12", TokenType::Semicolon)]
#[case("%MW10_", "%MW10", TokenType::Identifier)]
fn tokenize_when_direct_address_is_followed_by_token_then_does_not_take_it(
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
fn parse_when_direct_address_field_does_not_fit_in_32_bits_then_p0002() {
    let source = "PROGRAM main
VAR
    x AT %MW4294967296 : INT;
END_VAR
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());

    assert_eq!(result.unwrap_err().code, "P0002");
}
