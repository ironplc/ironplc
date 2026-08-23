//! OOP extensions: EXTENDS/IMPLEMENTS/INTERFACE round-trip.
//! See specs/plans/2026-07-18-twincat-extends-implements-interface.md.

use super::common::*;

fn inheritance_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn write_to_string_when_extends_implements_and_interface_then_round_trips() {
    let rendered = assert_round_trips(
        "
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
",
        &inheritance_options(),
    );

    assert!(rendered.contains("EXTENDS FB_Motor"));
    assert!(rendered.contains("IMPLEMENTS I_Drivable , I_Loggable"));
    assert!(rendered.contains("INTERFACE I_Drivable"));
    assert!(rendered.contains("END_INTERFACE"));
}

#[test]
fn write_to_string_when_interface_extends_base_then_round_trips() {
    let rendered = assert_round_trips(
        "
INTERFACE I_BaseAxis
END_INTERFACE

INTERFACE I_Focus EXTENDS I_BaseAxis
END_INTERFACE
",
        &inheritance_options(),
    );

    assert!(rendered.contains("INTERFACE I_Focus"));
    assert!(rendered.contains("EXTENDS I_BaseAxis"));
}

#[test]
fn write_to_string_when_abstract_fb_then_round_trips() {
    let rendered = assert_round_trips(
        "
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

INTERFACE I_BaseAxis
END_INTERFACE
",
        &inheritance_options(),
    );

    assert!(rendered.contains("FUNCTION_BLOCK ABSTRACT FB_BaseAxis"));
}
