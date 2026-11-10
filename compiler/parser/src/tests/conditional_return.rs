//! `RETURN(condition)` (TwinCAT): a return that only happens when the
//! condition is TRUE. The argument is not a return value.

use super::common::*;

/// The statements of the first program in `library`.
fn program_body(library: &Library) -> &Vec<StmtKind> {
    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    &cast!(&program.body, FunctionBlockBodyKind::Statements).body
}

#[test]
fn parse_when_return_has_condition_then_if_with_return_body() {
    let source = "
PROGRAM main
VAR b : BOOL; END_VAR
RETURN(b);
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let if_stmt = cast!(&program_body(&library)[0], StmtKind::If);
    assert_eq!(1, if_stmt.body.len());
    assert!(matches!(if_stmt.body[0], StmtKind::Return));
    assert!(if_stmt.else_ifs.is_empty());
    assert!(if_stmt.else_body.is_empty());
}

#[test]
fn parse_when_return_has_no_condition_then_plain_return() {
    let source = "
PROGRAM main
RETURN;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert!(matches!(program_body(&library)[0], StmtKind::Return));
}

#[test]
fn parse_when_return_condition_is_expression_with_space_then_if() {
    let source = "
PROGRAM main
VAR x : INT; END_VAR
RETURN (x > 5);
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert!(matches!(program_body(&library)[0], StmtKind::If(_)));
}

#[test]
fn parse_when_return_condition_has_no_semicolon_then_err() {
    let source = "
PROGRAM main
VAR b, c : BOOL; END_VAR
RETURN(b)
c := TRUE;
END_PROGRAM";
    assert!(parse_program(source, &FileId::default(), &CompilerOptions::default()).is_err());
}
