//! TwinCAT/CODESYS OOP extensions: EXTENDS/IMPLEMENTS/INTERFACE.
//! See specs/plans/2026-07-18-twincat-extends-implements-interface.md.

use super::common::*;

/// Proves that EXTENDS/IMPLEMENTS/INTERFACE/END_INTERFACE remain valid
/// identifiers in standard IEC 61131-3 mode. If keyword demotion for the
/// OOP extensions is missing or leaks into standard parsing, this test
/// fails. This must exist and pass before any OOP keyword tokens are
/// added (mirrors the same prerequisite in the Beckhoff TwinCAT dialect
/// design doc's Phase 0).
#[test]
fn parse_when_standard_mode_then_oop_keywords_are_valid_identifiers() {
    let program = "
FUNCTION_BLOCK FB_ALL_OOP_KEYWORDS_AS_VARS
VAR
    EXTENDS : INT;
    IMPLEMENTS : INT;
    INTERFACE : INT;
    END_INTERFACE : INT;
END_VAR

EXTENDS := 1;
IMPLEMENTS := 2;
INTERFACE := 3;
END_INTERFACE := 4;
END_FUNCTION_BLOCK
";
    let result = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "OOP keywords must remain valid identifiers in standard mode: {:?}",
        result.err()
    );
}

#[test]
fn parse_when_extends_only_then_ok_and_extends_set() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_oop_extensions()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.extends, Some(TypeName::from("FB_Motor")));
    assert!(fb.implements.is_empty());
}

#[test]
fn parse_when_implements_only_then_ok_and_implements_set() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_oop_extensions()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.extends, None);
    assert_eq!(fb.implements, vec![TypeName::from("I_Drivable")]);
}

#[test]
fn parse_when_extends_and_implements_then_ok() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_oop_extensions()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.extends, Some(TypeName::from("FB_Motor")));
    assert_eq!(fb.implements, vec![TypeName::from("I_Drivable")]);
}

#[test]
fn parse_when_implements_multiple_interfaces_then_ok() {
    // Real-world example: `IMPLEMENTS I_Hydraulics, I_Brake`.
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Hydraulics, I_Brake
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_oop_extensions()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(
        fb.implements,
        vec![TypeName::from("I_Hydraulics"), TypeName::from("I_Brake")]
    );
}

#[test]
fn parse_when_no_extends_or_implements_then_fields_empty() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_oop_extensions()).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.extends, None);
    assert!(fb.implements.is_empty());
}

#[test]
fn parse_when_extends_and_default_dialect_then_err() {
    // Without allow_oop_extensions, EXTENDS is just an identifier, so
    // this is a parse error (two consecutive identifiers).
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
