//! OOP extensions: EXTENDS/IMPLEMENTS/INTERFACE round-trip.
//! See specs/design/beckhoff-twincat-dialect.md §1.3-1.4.

use super::common::*;

fn inheritance_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn write_to_string_when_extends_implements_and_interface_then_round_trips() {
    assert_round_trips(
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
}

#[test]
fn write_to_string_when_interface_extends_base_then_round_trips() {
    assert_round_trips(
        "
INTERFACE I_BaseAxis
END_INTERFACE

INTERFACE I_Focus EXTENDS I_BaseAxis
END_INTERFACE
",
        &inheritance_options(),
    );
}

#[test]
fn write_to_string_when_abstract_fb_then_round_trips() {
    assert_round_trips(
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
}

#[test]
fn write_to_string_when_fb_has_access_specifier_and_final_then_round_trips() {
    let rendered = assert_round_trips(
        "
FUNCTION_BLOCK PUBLIC FINAL FB_Motor EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK
",
        &inheritance_options(),
    );
    assert!(rendered.contains("FUNCTION_BLOCK PUBLIC FINAL FB_Motor EXTENDS FB_Base"));
}
