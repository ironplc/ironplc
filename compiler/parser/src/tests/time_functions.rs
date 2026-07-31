//! TIME function/type declarations gated behind their flag.

use super::common::*;

#[test]
fn parse_when_time_function_call_and_flag_enabled_then_ok() {
    let source = "FUNCTION MY_FUNC : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
MY_FUNC := TIME_TO_DWORD(tx);
END_FUNCTION";

    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..Default::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn parse_when_time_function_call_and_flag_disabled_then_err() {
    let source = "FUNCTION MY_FUNC : DWORD
VAR
    tx : TIME;
END_VAR
tx := TIME();
END_FUNCTION";

    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_time_duration_literal_and_flag_enabled_then_ok() {
    let source = "PROGRAM main
VAR
    t : TIME;
END_VAR
    t := TIME#5s;
END_PROGRAM";

    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..Default::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn parse_when_time_type_decl_and_flag_enabled_then_ok() {
    let source = "PROGRAM main
VAR
    tx : TIME;
END_VAR
    tx := TIME#1s;
END_PROGRAM";

    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..Default::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn parse_when_time_function_declaration_and_flag_enabled_then_ok() {
    let source = "FUNCTION TIME : TIME
VAR
    t : TIME;
END_VAR
TIME := T#0s;
END_FUNCTION

PROGRAM main
VAR
    t : TIME;
END_VAR
t := TIME();
END_PROGRAM";

    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..Default::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
}

#[test]
fn parse_when_time_function_declaration_and_flag_disabled_then_err() {
    let source = "FUNCTION TIME : TIME
TIME := T#0s;
END_FUNCTION";

    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
