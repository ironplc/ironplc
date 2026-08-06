//! `--allow-*` dialect flags (missing semicolon, empty VAR blocks).

use super::common::*;

/// Dialect-flag acceptance tests.
///
/// Each case: given a program that the default dialect rejects, parsing
/// should succeed when the corresponding `--allow-*` flag is enabled.
/// Collapses 14 hand-written tests (cargo-dupes group `c3c13bfc`) into
/// one table; each row is still run as an individually-named test.
#[rstest]
// --allow-missing-semicolon: trailing STRUCT/END_IF/END_WHILE/etc. without `;`
#[case::struct_without_trailing_semicolon(
    "TYPE MY_POINT :
STRUCT
    X : REAL;
    Y : REAL;
END_STRUCT
END_TYPE

PROGRAM main
VAR
    pt : MY_POINT;
END_VAR
    pt.X := 1.0;
    pt.Y := 2.0;
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::end_if_without_semicolon(
    "PROGRAM main
VAR
    x : BOOL;
END_VAR
    IF x THEN
        x := FALSE;
    END_IF
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::end_while_without_semicolon(
    "PROGRAM main
VAR
    x : BOOL;
END_VAR
    WHILE x DO
        x := FALSE;
    END_WHILE
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::end_for_without_semicolon(
    "PROGRAM main
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        i := i;
    END_FOR
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::end_case_without_semicolon(
    "PROGRAM main
VAR
    x : INT;
END_VAR
    CASE x OF
        1: x := 2;
    END_CASE
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::end_repeat_without_semicolon(
    "PROGRAM main
VAR
    x : BOOL;
END_VAR
    REPEAT
        x := FALSE;
    UNTIL x
    END_REPEAT
END_PROGRAM",
    with_missing_semicolon_flag
)]
#[case::function_end_if_without_semicolon(
    "FUNCTION MY_FUNC : REAL
VAR_INPUT
    x : INT;
END_VAR
IF x > 0 THEN
    MY_FUNC := 1.0;
ELSE
    MY_FUNC := 0.0;
END_IF
END_FUNCTION",
    with_missing_semicolon_flag
)]
// --allow-empty-var-blocks: empty VAR / VAR_INPUT / VAR_OUTPUT / etc.
#[case::empty_var_in_function(
    "
FUNCTION myFunc : INT
VAR
END_VAR
    myFunc := 1;
END_FUNCTION",
    with_empty_var_blocks_flag
)]
#[case::empty_var_input(
    "
FUNCTION myFunc : INT
VAR_INPUT
END_VAR
    myFunc := 1;
END_FUNCTION",
    with_empty_var_blocks_flag
)]
#[case::empty_var_output(
    "
FUNCTION myFunc : INT
VAR_OUTPUT
END_VAR
    myFunc := 1;
END_FUNCTION",
    with_empty_var_blocks_flag
)]
#[case::empty_var_in_out(
    "
FUNCTION myFunc : INT
VAR_IN_OUT
END_VAR
    myFunc := 1;
END_FUNCTION",
    with_empty_var_blocks_flag
)]
#[case::empty_var_constant(
    "
FUNCTION myFunc : INT
VAR CONSTANT
END_VAR
    myFunc := 1;
END_FUNCTION",
    with_empty_var_blocks_flag
)]
#[case::empty_var_in_program(
    "
PROGRAM main
VAR
END_VAR
END_PROGRAM",
    with_empty_var_blocks_flag
)]
#[case::empty_var_in_function_block(
    "
FUNCTION_BLOCK myFb
VAR
END_VAR
END_FUNCTION_BLOCK",
    with_empty_var_blocks_flag
)]
fn parse_with_dialect_flag_then_ok(
    #[case] source: &str,
    #[case] build_options: fn() -> CompilerOptions,
) {
    let options = build_options();
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok());
}

/// Dialect-flag rejection tests.
///
/// Each case: given a program that the default dialect rejects, parsing with
/// the default (strict) options should fail because the corresponding
/// `--allow-*` flag is not enabled; each row still runs as an
/// individually-named test.
#[rstest]
#[case::struct_without_trailing_semicolon_and_flag_disabled(
    "TYPE MY_POINT :
STRUCT
    X : REAL;
    Y : REAL;
END_STRUCT
END_TYPE

PROGRAM main
VAR
    pt : MY_POINT;
END_VAR
    pt.X := 1.0;
    pt.Y := 2.0;
END_PROGRAM"
)]
#[case::empty_var_in_function_without_flag(
    "
FUNCTION myFunc : INT
VAR
END_VAR
    myFunc := 1;
END_FUNCTION"
)]
fn parse_without_dialect_flag_then_err(#[case] source: &str) {
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
