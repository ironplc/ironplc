//! OOP extension: METHOD declarations and instance.Method(args) calls,
//! round-trip. See
//! specs/plans/2026-08-12-oop-method-declarations-static-dispatch.md.

use super::common::*;

#[test]
fn write_to_string_when_method_has_params_and_return_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    rSpeed : REAL;
END_VAR
METHOD SetSpeed : BOOL
VAR_INPUT
    rNewSpeed : REAL;
END_VAR
    rSpeed := rNewSpeed;
    SetSpeed := TRUE;
END_METHOD
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("METHOD SetSpeed : BOOL"));
    assert!(rendered.contains("END_METHOD"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_method_has_no_return_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_multiple_methods_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
METHOD Stop
    bRunning := FALSE;
END_METHOD
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_method_call_statement_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    rSpeed : REAL;
END_VAR
METHOD SetSpeed : BOOL
VAR_INPUT
    rNewSpeed : REAL;
END_VAR
    rSpeed := rNewSpeed;
    SetSpeed := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.SetSpeed(1.5);
END_PROGRAM
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("SetSpeed"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}
