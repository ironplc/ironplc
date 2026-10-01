//! OOP extension: PROPERTY declarations with GET/SET accessors, round-trip.
//!
//! A SET accessor carries an implicit input for the assigned value (see
//! `PropertyDeclaration`). The re-parse catches a renderer that writes it
//! out: the rendered VAR_INPUT would parse as a second input, and the ASTs
//! would differ.

use super::common::*;
use rstest::rstest;

#[rstest]
#[case::get_and_set(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
SET
    _speed := Speed;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::get_only_with_var_block(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
PROPERTY Speed : REAL
GET
VAR
    tmp : REAL;
END_VAR
    tmp := _speed;
    Speed := tmp;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::set_only_string(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _name : STRING[20];
END_VAR
PROPERTY Name : STRING[20]
SET
    _name := Name;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"
)]
#[case::interleaved_with_methods(
    "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
METHOD Start
    _speed := 1.0;
END_METHOD
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
END_PROPERTY
METHOD Stop
    _speed := 0.0;
END_METHOD
END_FUNCTION_BLOCK
"
)]
fn write_to_string_when_property_then_round_trips(#[case] source: &str) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_round_trips(source, &options);
}
