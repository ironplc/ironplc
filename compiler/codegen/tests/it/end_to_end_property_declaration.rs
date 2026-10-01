//! A function block that declares PROPERTY members compiles and runs. The
//! accessors are not compiled yet (nothing can call them), and declaring them
//! must not disturb the instance's fields or its methods.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

#[test]
fn end_to_end_when_fb_declares_unused_properties_then_methods_and_fields_still_work() {
    let source = "
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
METHOD SetSpeed
VAR_INPUT
    value : REAL;
END_VAR
    _speed := value;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    x : REAL;
END_VAR
m.SetSpeed(1.5);
x := m._speed;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let (_c, bufs) = parse_and_run(source, &options);

    // x : REAL is var[1], as in `end_to_end_methods.rs`.
    assert_eq!(bufs.vars[1].as_f32(), 1.5);
}
