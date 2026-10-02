//! `CONTINUE` statement parsing (IEC 61131-3 Edition 3).

use super::common::*;

fn opts_with_continue() -> CompilerOptions {
    CompilerOptions {
        allow_continue: true,
        ..CompilerOptions::default()
    }
}

/// The statements of the first program in `library`.
fn program_body(library: &Library) -> &Vec<StmtKind> {
    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    &cast!(&program.body, FunctionBlockBodyKind::Statements).body
}

#[test]
fn parse_when_continue_in_for_then_continue_statement_in_loop_body() {
    let source = "
PROGRAM main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO
  CONTINUE;
END_FOR;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &opts_with_continue()).unwrap();
    let for_stmt = cast!(&program_body(&library)[0], StmtKind::For);
    assert_eq!(1, for_stmt.body.len());
    assert!(matches!(for_stmt.body[0], StmtKind::Continue(_)));
}

#[test]
fn parse_when_continue_in_while_and_repeat_then_continue_statements() {
    let source = "
PROGRAM main
VAR b : BOOL; END_VAR
WHILE b DO
  continue;
END_WHILE;
REPEAT
  Continue;
UNTIL b
END_REPEAT;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &opts_with_continue()).unwrap();
    let body = program_body(&library);
    let while_stmt = cast!(&body[0], StmtKind::While);
    assert!(matches!(while_stmt.body[0], StmtKind::Continue(_)));
    let repeat_stmt = cast!(&body[1], StmtKind::Repeat);
    assert!(matches!(repeat_stmt.body[0], StmtKind::Continue(_)));
}

#[test]
fn parse_when_continue_in_case_branch_then_continue_statement() {
    let source = "
PROGRAM main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO
  CASE i OF
    2: CONTINUE;
  END_CASE;
END_FOR;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &opts_with_continue()).unwrap();
    let for_stmt = cast!(&program_body(&library)[0], StmtKind::For);
    let case = cast!(&for_stmt.body[0], StmtKind::Case);
    assert!(matches!(
        case.statement_groups[0].statements[0],
        StmtKind::Continue(_)
    ));
}

#[test]
fn parse_when_continue_and_edition_2_then_syntax_error() {
    let source = "
PROGRAM main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO
  CONTINUE;
END_FOR;
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_continue_and_edition_3_dialect_then_ok() {
    let source = "
PROGRAM main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO
  CONTINUE;
END_FOR;
END_PROGRAM";
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "parse failed: {:?}", result.err());
}

#[test]
fn parse_when_continue_as_variable_and_edition_2_then_identifier() {
    let source = "
PROGRAM main
VAR continue : INT; END_VAR
continue := 1;
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert!(matches!(program_body(&library)[0], StmtKind::Assignment(_)));
}
