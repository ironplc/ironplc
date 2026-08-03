//! OOP extensions: EXTENDS/IMPLEMENTS/INTERFACE.
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
    ABSTRACT : INT;
END_VAR

EXTENDS := 1;
IMPLEMENTS := 2;
INTERFACE := 3;
END_INTERFACE := 4;
ABSTRACT := 5;
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
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert_eq!(oop.base, Some(TypeName::from("FB_Motor")));
    assert!(oop.implements.is_empty());
}

#[test]
fn parse_when_implements_only_then_ok_and_implements_set() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert_eq!(oop.base, None);
    assert_eq!(oop.implements, vec![TypeName::from("I_Drivable")]);
}

#[test]
fn parse_when_extends_and_implements_then_ok() {
    let source = "
FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert_eq!(oop.base, Some(TypeName::from("FB_Motor")));
    assert_eq!(oop.implements, vec![TypeName::from("I_Drivable")]);
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
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert_eq!(
        oop.implements,
        vec![TypeName::from("I_Hydraulics"), TypeName::from("I_Brake")]
    );
}

#[test]
fn parse_when_no_extends_or_implements_then_oop_none() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert!(fb.oop.is_none());
}

#[test]
fn parse_when_extends_and_default_dialect_then_err() {
    // Without allow_fb_inheritance, EXTENDS is just an identifier, so
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

#[test]
fn parse_when_abstract_only_then_ok_and_is_abstract_set() {
    let source = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert!(oop.is_abstract);
    assert_eq!(oop.base, None);
    assert!(oop.implements.is_empty());
}

#[test]
fn parse_when_abstract_and_extends_and_implements_then_ok() {
    // Real-world shape: `ABSTRACT` combined with `EXTENDS`/`IMPLEMENTS`.
    let source = "
FUNCTION_BLOCK ABSTRACT FB_AxisControl EXTENDS FB_BaseAxis IMPLEMENTS I_Axis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    let oop = fb.oop.as_ref().expect("expected oop to be Some");
    assert!(oop.is_abstract);
    assert_eq!(oop.base, Some(TypeName::from("FB_BaseAxis")));
    assert_eq!(oop.implements, vec![TypeName::from("I_Axis")]);
}

#[test]
fn parse_when_no_abstract_then_oop_none() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let fb = extract_fb(&library);
    assert!(fb.oop.is_none());
}

#[test]
fn parse_when_abstract_and_default_dialect_then_ok_as_identifier() {
    // Without allow_fb_inheritance, ABSTRACT demotes to an ordinary
    // identifier, so `FUNCTION_BLOCK ABSTRACT` is parsed as a function
    // block literally named "ABSTRACT" -- matching how EXTENDS/IMPLEMENTS
    // behave when the flag is off.
    let source = "
FUNCTION_BLOCK ABSTRACT
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default())
        .expect("ABSTRACT must remain a valid identifier in standard mode");
    let fb = extract_fb(&library);
    assert_eq!(fb.name, TypeName::from("ABSTRACT"));
    assert!(fb.oop.is_none());
}
