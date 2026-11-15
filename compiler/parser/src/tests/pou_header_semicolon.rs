//! A `;` after a POU header (`--allow-pou-header-semicolon`).
use super::common::*;

fn problem_codes(source: &str, options: &CompilerOptions) -> Vec<String> {
    match parse_program(source, &FileId::default(), options) {
        Ok(_) => vec![],
        Err(diagnostic) => vec![diagnostic.code],
    }
}

const FUNCTION_BLOCK_SOURCE: &str = "
INTERFACE I_T
END_INTERFACE

FUNCTION_BLOCK FB_B
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB EXTENDS FB_B IMPLEMENTS I_T;
VAR_INPUT
    a : INT;
END_VAR
VAR_OUTPUT
    b : INT;
END_VAR
END_FUNCTION_BLOCK
";

const PROGRAM_SOURCE: &str = "
PROGRAM main;
VAR
    a : INT;
END_VAR
a := 1;
END_PROGRAM
";

const FUNCTION_SOURCE: &str = "
FUNCTION F : INT;
VAR_INPUT
    a : INT;
END_VAR
F := a;
END_FUNCTION
";

const METHOD_SOURCE: &str = "
FUNCTION_BLOCK FB
METHOD M : INT;
VAR_INPUT
    a : INT;
END_VAR
M := a;
END_METHOD
END_FUNCTION_BLOCK
";

#[test]
fn parse_when_function_block_header_semicolon_then_var_blocks_are_declarations() {
    let library = parse_program(
        FUNCTION_BLOCK_SOURCE,
        &FileId::default(),
        &opts_with_pou_header_semicolon(),
    )
    .unwrap();

    let fb = cast!(
        &library.elements[2],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(2, fb.variables.len());
    let oop = fb.oop.as_ref().unwrap();
    assert!(oop.base.is_some());
    assert_eq!(1, oop.implements.len());
}

#[test]
fn parse_when_program_header_semicolon_then_var_blocks_are_declarations() {
    let library = parse_program(
        PROGRAM_SOURCE,
        &FileId::default(),
        &opts_with_pou_header_semicolon(),
    )
    .unwrap();

    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(1, program.variables.len());
}

#[test]
fn parse_when_function_header_semicolon_then_var_blocks_are_declarations() {
    let library = parse_program(
        FUNCTION_SOURCE,
        &FileId::default(),
        &opts_with_pou_header_semicolon(),
    )
    .unwrap();

    let function = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionDeclaration
    );
    assert_eq!(1, function.variables.len());
}

#[test]
fn parse_when_method_header_semicolon_then_var_blocks_are_declarations() {
    let library = parse_program(
        METHOD_SOURCE,
        &FileId::default(),
        &opts_with_pou_header_semicolon(),
    )
    .unwrap();

    let fb = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(1, fb.methods[0].variables.len());
}

/// The `;` carries nothing: the declaration is the one written without it.
/// (`SourceSpan` compares equal unconditionally, so the shifted spans do not
/// matter.)
#[rstest]
#[case::function_block(FUNCTION_BLOCK_SOURCE)]
#[case::program(PROGRAM_SOURCE)]
#[case::function(FUNCTION_SOURCE)]
#[case::method(METHOD_SOURCE)]
fn parse_when_header_semicolon_then_same_ast_as_without(#[case] source: &str) {
    let options = opts_with_pou_header_semicolon();
    let without = source.replacen(";\nVAR", "\nVAR", 1);
    assert_ne!(source, without);
    let with = parse_program(source, &FileId::default(), &options).unwrap();
    let without = parse_program(&without, &FileId::default(), &options).unwrap();
    assert_eq!(with, without);
}

#[rstest]
#[case::function_block(FUNCTION_BLOCK_SOURCE)]
#[case::program(PROGRAM_SOURCE)]
#[case::function(FUNCTION_SOURCE)]
#[case::method(METHOD_SOURCE)]
fn parse_when_header_semicolon_and_flag_off_then_p4098(#[case] source: &str) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_eq!(problem_codes(source, &options), vec!["P4098"]);
}

/// A `;` that a VAR block does not follow is the standard empty statement
/// at the start of the body, so it needs no flag.
#[test]
fn parse_when_semicolon_after_header_before_statement_and_flag_off_then_ok() {
    let source = "
PROGRAM main
VAR
    a : INT;
END_VAR
END_PROGRAM

PROGRAM other;
a := 1;
END_PROGRAM
";
    assert!(problem_codes(source, &CompilerOptions::default()).is_empty());
}

/// The flag is TwinCAT-only.
#[test]
fn parse_when_twincat_dialect_and_header_semicolon_then_ok() {
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    assert!(problem_codes(FUNCTION_BLOCK_SOURCE, &options).is_empty());
}
