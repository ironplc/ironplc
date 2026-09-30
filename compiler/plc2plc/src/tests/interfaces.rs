//! OOP extension: INTERFACE member prototypes, round-trip.
//!
//! Methods render before properties, so an interleaved source still
//! round-trips: the re-parsed AST keeps each kind's own order.

use super::common::*;
use rstest::rstest;

fn inheritance_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[rstest]
#[case::method_prototypes(
    "
INTERFACE I_Brake
METHOD CloseBrake : BOOL
VAR_INPUT
    force : REAL;
END_VAR
VAR_OUTPUT
    done : BOOL;
END_VAR
END_METHOD
METHOD Reset
END_METHOD
END_INTERFACE
"
)]
#[case::interface_typed_variables(
    "
INTERFACE I_Comm
END_INTERFACE

FUNCTION_BLOCK FB_Device
VAR_INPUT
    comm : I_Comm;
END_VAR
VAR
    backup : I_Comm;
END_VAR
    backup := comm;
END_FUNCTION_BLOCK
"
)]
#[case::method_prototype_qualifiers(
    "
INTERFACE I_Telescope
METHOD PUBLIC ABSTRACT Park : BOOL
END_METHOD
END_INTERFACE
"
)]
#[case::property_prototypes(
    "
INTERFACE I_Axis
PROPERTY Position : LREAL
GET END_GET
END_PROPERTY
PROPERTY Target : LREAL
SET END_SET
END_PROPERTY
PROPERTY Name : STRING[20]
GET END_GET
SET END_SET
END_PROPERTY
END_INTERFACE
"
)]
#[case::interleaved_with_extends(
    "
INTERFACE I_Axis
END_INTERFACE

INTERFACE I_Telescope EXTENDS I_Axis
PROPERTY Ready : BOOL
GET END_GET
END_PROPERTY
METHOD Park : BOOL
END_METHOD
PROPERTY Busy : BOOL
GET END_GET
END_PROPERTY
END_INTERFACE
"
)]
fn write_to_string_when_interface_members_then_round_trips(#[case] source: &str) {
    assert_round_trips(source, &inheritance_options());
}
