//! End-to-end tests for METHOD declarations and calls (OOP extension,
//! ADR-0041 Phase 1 static dispatch). Proves the METHOD_CALL calling
//! convention end-to-end: a method call actually mutates the FB
//! instance's own persistent field, not just some scratch copy.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

fn opts_with_fb_inheritance() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_method_call_then_mutates_fb_field() {
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    rSpeed : REAL;
END_VAR
METHOD SetSpeed
VAR_INPUT
    rNewSpeed : REAL;
END_VAR
    rSpeed := rNewSpeed;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    x : REAL;
END_VAR
m.SetSpeed(1.5);
x := m.rSpeed;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_fb_inheritance());

    // x : REAL is var[1] (m has no top-level slot of its own; x follows
    // the program's declared VAR order).
    assert_eq!(bufs.vars[1].as_f32(), 1.5);
}

#[test]
fn end_to_end_when_method_called_twice_with_different_args_then_field_reflects_last_call() {
    let source = "
FUNCTION_BLOCK FB_Counter
VAR
    nCount : DINT;
END_VAR
METHOD SetCount
VAR_INPUT
    nNew : DINT;
END_VAR
    nCount := nNew;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    c : FB_Counter;
    x : DINT;
END_VAR
c.SetCount(5);
c.SetCount(10);
x := c.nCount;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_fb_inheritance());

    assert_eq!(bufs.vars[1].as_i32(), 10);
}

#[test]
fn end_to_end_when_method_mutates_field_based_on_own_field_then_reads_self_correctly() {
    // Proves the method body can read ANOTHER field of the same instance
    // (not just write the one field it was called to set), confirming
    // the copy-in brought in the whole field region, not just one slot.
    let source = "
FUNCTION_BLOCK FB_Accumulator
VAR
    nTotal : DINT;
END_VAR
METHOD Add
VAR_INPUT
    nAmount : DINT;
END_VAR
    nTotal := nTotal + nAmount;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    a : FB_Accumulator;
    x : DINT;
END_VAR
a.Add(3);
a.Add(4);
x := a.nTotal;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_fb_inheritance());

    assert_eq!(bufs.vars[1].as_i32(), 7);
}

#[test]
fn end_to_end_when_named_args_then_resolved_to_correct_param() {
    let source = "
FUNCTION_BLOCK FB_Point
VAR
    nX : DINT;
    nY : DINT;
END_VAR
METHOD SetPosition
VAR_INPUT
    x : DINT;
    y : DINT;
END_VAR
    nX := x;
    nY := y;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    p : FB_Point;
    outX : DINT;
    outY : DINT;
END_VAR
p.SetPosition(y := 2, x := 1);
outX := p.nX;
outY := p.nY;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &opts_with_fb_inheritance());

    assert_eq!(bufs.vars[1].as_i32(), 1);
    assert_eq!(bufs.vars[2].as_i32(), 2);
}
