//! OOP extensions: EXTENDS/IMPLEMENTS/INTERFACE round-trip.
//! See specs/plans/2026-07-18-twincat-extends-implements-interface.md.

use super::common::*;

#[test]
fn write_to_string_when_extends_implements_and_interface_then_round_trips() {
    let source = "
INTERFACE I_Drivable
END_INTERFACE

INTERFACE I_Loggable
END_INTERFACE

FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable, I_Loggable
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("EXTENDS FB_Motor"));
    assert!(rendered.contains("IMPLEMENTS I_Drivable , I_Loggable"));
    assert!(rendered.contains("INTERFACE I_Drivable"));
    assert!(rendered.contains("END_INTERFACE"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_interface_extends_base_then_round_trips() {
    let source = "
INTERFACE I_BaseAxis
END_INTERFACE

INTERFACE I_Focus EXTENDS I_BaseAxis
END_INTERFACE
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("INTERFACE I_Focus"));
    assert!(rendered.contains("EXTENDS I_BaseAxis"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn write_to_string_when_abstract_fb_then_round_trips() {
    let source = "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

INTERFACE I_BaseAxis
END_INTERFACE
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("FUNCTION_BLOCK ABSTRACT FB_BaseAxis"));

    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}
