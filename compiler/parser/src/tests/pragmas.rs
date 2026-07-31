//! CODESYS/TwinCAT pragma skipping.

use super::common::*;

#[test]
fn parse_program_when_pragma_header_and_codesys_dialect_then_ok() {
    let source = enum_with_pragma_header();
    let options = CompilerOptions::from_dialect(Dialect::Codesys);

    let result = parse_program(&source, &FileId::default(), &options);

    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

#[test]
fn parse_program_when_pragma_header_and_default_dialect_then_err() {
    let source = enum_with_pragma_header();

    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());

    assert!(
        result.is_err(),
        "pragmas should still be unrecognized syntax without allow_pragmas"
    );
}

#[test]
fn parse_program_when_pragma_between_declarations_then_ok() {
    let source = "
        TYPE E_Color :
            (Red, Green, Blue);
        END_TYPE
        {attribute 'qualified_only'}
        FUNCTION_BLOCK FB_Example
        VAR
            x : INT;
        END_VAR
        END_FUNCTION_BLOCK";
    let options = CompilerOptions::from_dialect(Dialect::Codesys);

    let result = parse_program(source, &FileId::default(), &options);

    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

#[test]
fn parse_program_when_unclosed_pragma_and_codesys_dialect_then_err() {
    let source = "
        {attribute 'qualified_only'
        TYPE E_Color :
            (Red, Green, Blue);
        END_TYPE";
    let options = CompilerOptions::from_dialect(Dialect::Codesys);

    let result = parse_program(source, &FileId::default(), &options);

    assert!(result.is_err());
}
