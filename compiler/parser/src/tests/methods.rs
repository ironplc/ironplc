//! OOP extension: METHOD ... END_METHOD declarations on a function block.
//! See ADR-0041 (specs/adrs/0041-staged-method-and-interface-dispatch.md).

use super::common::*;

/// Proves that METHOD/END_METHOD remain valid identifiers in standard
/// IEC 61131-3 mode, mirroring the same prerequisite already established
/// for EXTENDS/IMPLEMENTS/INTERFACE/ABSTRACT in `fb_inheritance.rs`.
#[test]
fn parse_when_standard_mode_then_method_keywords_are_valid_identifiers() {
    let program = "
FUNCTION_BLOCK FB_ALL_METHOD_KEYWORDS_AS_VARS
VAR
    METHOD : INT;
    END_METHOD : INT;
END_VAR

METHOD := 1;
END_METHOD := 2;
END_FUNCTION_BLOCK
";
    let result = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "METHOD/END_METHOD must remain valid identifiers in standard mode: {:?}",
        result.err()
    );
}

#[test]
fn parse_when_method_and_default_dialect_then_err() {
    // Without allow_fb_inheritance, METHOD is just an identifier, so this
    // is a parse error (a bare identifier where a var block or the end of
    // the function block was expected).
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
END_METHOD
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_method_has_no_params_and_no_return_then_ok() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.methods.len(), 1);
    let method = &fb.methods[0];
    assert_eq!(method.name, Id::from("Start"));
    assert_eq!(method.return_type, None);
    assert!(method.variables.is_empty());
    assert_eq!(method.body.len(), 1);
}

#[test]
fn parse_when_method_has_params_and_return_then_ok() {
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
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.methods.len(), 1);
    let method = &fb.methods[0];
    assert_eq!(method.name, Id::from("SetSpeed"));
    assert_eq!(
        method.return_type,
        Some(FunctionReturnType::Named(TypeName::from("BOOL")))
    );
    assert_eq!(method.variables.len(), 1);
    assert_eq!(method.body.len(), 2);
}

#[test]
fn parse_when_multiple_methods_then_all_captured_in_order() {
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
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.methods.len(), 2);
    assert_eq!(fb.methods[0].name, Id::from("Start"));
    assert_eq!(fb.methods[1].name, Id::from("Stop"));
}

#[test]
fn parse_when_no_methods_then_empty() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert!(fb.methods.is_empty());
}

#[test]
fn parse_when_extends_and_methods_then_both_captured() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(
        fb.oop.as_ref().unwrap().base,
        Some(TypeName::from("FB_Motor"))
    );
    assert_eq!(fb.methods.len(), 1);
}
