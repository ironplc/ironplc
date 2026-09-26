//! End-to-end tests for user-defined functions with `VAR_IN_OUT` parameters.
//!
//! A `VAR_IN_OUT` parameter is passed by reference, so each test asserts on
//! the caller's variable after the call, not only on the function's result.
//! The end-to-end harness runs type resolution but not the semantic rules, so
//! every program here is also run through the full analysis first: a program
//! the checker refuses must not pass an end-to-end test.

use crate::common::{parse_and_run, try_parse_and_compile, VmBuffers};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;

/// Asserts the full semantic analysis accepts `source`, then runs one scan.
fn check_and_run(source: &str, options: &CompilerOptions) -> VmBuffers {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (_, context) = ironplc_analyzer::stages::analyze(&[&library], options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "check refused the program: {:?}",
        context.diagnostics()
    );
    let (_container, bufs) = parse_and_run(source, options);
    bufs
}

fn assert_i32(source: &str, asserts: &[(usize, i32)]) {
    assert_i32_with(source, &CompilerOptions::default(), asserts);
}

fn assert_i32_with(source: &str, options: &CompilerOptions, asserts: &[(usize, i32)]) {
    let bufs = check_and_run(source, options);
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

#[test]
fn end_to_end_when_in_out_dint_then_caller_variable_updated() {
    assert_i32(
        "
FUNCTION ADD_TEN : DINT
VAR_IN_OUT data : DINT; END_VAR
    data := data + 10;
    ADD_TEN := data;
END_FUNCTION
PROGRAM main
VAR x : DINT := 5; result : DINT; END_VAR
    result := ADD_TEN(data := x);
END_PROGRAM",
        &[(0, 15), (1, 15)],
    );
}

// The example from issue #1658.
#[test]
fn end_to_end_when_in_out_and_input_named_then_caller_variable_updated() {
    assert_i32(
        "
FUNCTION ADD_N : DINT
VAR_INPUT n : DINT; END_VAR
VAR_IN_OUT data : DINT; END_VAR
    data := data + n;
    ADD_N := data;
END_FUNCTION
PROGRAM main
VAR x : DINT := 100; result : DINT; END_VAR
    result := ADD_N(n := 42, data := x);
END_PROGRAM",
        &[(0, 142), (1, 142)],
    );
}

#[test]
fn end_to_end_when_in_out_before_input_positional_then_binds_in_declaration_order() {
    assert_i32(
        "
FUNCTION SCALE : INT
VAR_IN_OUT acc : INT; END_VAR
VAR_INPUT factor : INT; END_VAR
    acc := acc * factor;
    SCALE := acc;
END_FUNCTION
PROGRAM main
VAR total : INT := 7; result : INT; END_VAR
    result := SCALE(total, 3);
END_PROGRAM",
        &[(0, 21), (1, 21)],
    );
}

#[test]
fn end_to_end_when_in_out_int_overflows_then_caller_variable_wraps_at_int_width() {
    assert_i32(
        "
FUNCTION BUMP : BOOL
VAR_IN_OUT v : INT; END_VAR
    v := v + 1;
    BUMP := TRUE;
END_FUNCTION
PROGRAM main
VAR x : INT := 32767; ok : BOOL; END_VAR
    ok := BUMP(x);
END_PROGRAM",
        &[(0, -32768), (1, 1)],
    );
}

#[test]
fn end_to_end_when_in_out_bool_then_caller_variable_toggled() {
    assert_i32(
        "
FUNCTION TOGGLE : BOOL
VAR_IN_OUT flag : BOOL; END_VAR
    flag := NOT flag;
    TOGGLE := flag;
END_FUNCTION
PROGRAM main
VAR f : BOOL := FALSE; result : BOOL; END_VAR
    result := TOGGLE(f);
END_PROGRAM",
        &[(0, 1), (1, 1)],
    );
}

#[test]
fn end_to_end_when_in_out_real_then_caller_variable_updated() {
    let bufs = check_and_run(
        "
FUNCTION DOUBLE : BOOL
VAR_IN_OUT r : REAL; END_VAR
    r := r * 2.0;
    DOUBLE := TRUE;
END_FUNCTION
PROGRAM main
VAR x : REAL := 1.5; ok : BOOL; END_VAR
    ok := DOUBLE(x);
END_PROGRAM",
        &CompilerOptions::default(),
    );
    assert_eq!(bufs.vars[0].as_f32(), 3.0);
}

#[test]
fn end_to_end_when_swap_then_caller_variables_exchanged() {
    assert_i32(
        "
FUNCTION SWAP : BOOL
VAR_IN_OUT a : DINT; b : DINT; END_VAR
VAR t : DINT; END_VAR
    t := a;
    a := b;
    b := t;
    SWAP := TRUE;
END_FUNCTION
PROGRAM main
VAR x : DINT := 1; y : DINT := 2; ok : BOOL; END_VAR
    ok := SWAP(x, y);
END_PROGRAM",
        &[(0, 2), (1, 1)],
    );
}

// Both parameters refer to the same variable, so each write is seen by the
// other: by reference, not copy-in/copy-out.
#[test]
fn end_to_end_when_two_in_out_alias_one_variable_then_writes_share_it() {
    assert_i32(
        "
FUNCTION BOTH : DINT
VAR_IN_OUT a : DINT; b : DINT; END_VAR
    a := a + 1;
    b := b + 10;
    BOTH := a;
END_FUNCTION
PROGRAM main
VAR x : DINT := 0; result : DINT; END_VAR
    result := BOTH(x, x);
END_PROGRAM",
        &[(0, 11), (1, 11)],
    );
}

// OUTER passes its own VAR_IN_OUT on to INNER, which writes the program's
// variable, not OUTER's parameter slot.
#[test]
fn end_to_end_when_in_out_forwarded_to_in_out_then_original_variable_updated() {
    assert_i32(
        "
FUNCTION INNER : BOOL
VAR_IN_OUT v : DINT; END_VAR
    v := v + 1;
    INNER := TRUE;
END_FUNCTION
FUNCTION OUTER : DINT
VAR_IN_OUT v : DINT; END_VAR
VAR ok : BOOL; END_VAR
    ok := INNER(v);
    ok := INNER(v := v);
    OUTER := v;
END_FUNCTION
PROGRAM main
VAR x : DINT := 40; result : DINT; END_VAR
    result := OUTER(x);
END_PROGRAM",
        &[(0, 42), (1, 42)],
    );
}

// OUTER passes its own local to INNER. The local lives in OUTER's frame,
// outside INNER's variables, and is reached through the reference.
#[test]
fn end_to_end_when_function_local_passed_to_in_out_then_local_updated() {
    assert_i32(
        "
FUNCTION INNER : BOOL
VAR_IN_OUT v : DINT; END_VAR
    v := v * 3;
    INNER := TRUE;
END_FUNCTION
FUNCTION OUTER : DINT
VAR_INPUT n : DINT; END_VAR
VAR local : DINT; ok : BOOL; END_VAR
    local := n;
    ok := INNER(local);
    OUTER := local;
END_FUNCTION
PROGRAM main
VAR result : DINT; END_VAR
    result := OUTER(5);
END_PROGRAM",
        &[(0, 15)],
    );
}

// A VAR_IN_OUT passed to a VAR_INPUT passes the value it refers to.
#[test]
fn end_to_end_when_in_out_passed_to_input_then_value_passed() {
    assert_i32(
        "
FUNCTION TWICE : DINT
VAR_INPUT n : DINT; END_VAR
    TWICE := n * 2;
END_FUNCTION
FUNCTION APPLY : DINT
VAR_IN_OUT v : DINT; END_VAR
    v := TWICE(v);
    APPLY := v;
END_FUNCTION
PROGRAM main
VAR x : DINT := 21; result : DINT; END_VAR
    result := APPLY(x);
END_PROGRAM",
        &[(0, 42), (1, 42)],
    );
}

#[test]
fn end_to_end_when_in_out_in_condition_and_loop_then_reads_current_value() {
    assert_i32(
        "
FUNCTION CLAMP_COUNT : DINT
VAR_IN_OUT v : DINT; END_VAR
VAR i : DINT; END_VAR
    FOR i := 1 TO 5 DO
        IF v < 12 THEN
            v := v + 1;
        END_IF;
    END_FOR;
    CLAMP_COUNT := v;
END_FUNCTION
PROGRAM main
VAR x : DINT := 10; result : DINT; END_VAR
    result := CLAMP_COUNT(x);
END_PROGRAM",
        &[(0, 12), (1, 12)],
    );
}

// A function block field passed as the argument: the write lands in the
// instance, so it accumulates across invocations.
#[test]
fn end_to_end_when_fb_field_passed_to_in_out_then_field_updated() {
    assert_i32(
        "
FUNCTION INC : BOOL
VAR_IN_OUT v : DINT; END_VAR
    v := v + 1;
    INC := TRUE;
END_FUNCTION
FUNCTION_BLOCK COUNTER
VAR_OUTPUT q : DINT; END_VAR
VAR count : DINT; ok : BOOL; END_VAR
    ok := INC(count);
    q := count;
END_FUNCTION_BLOCK
PROGRAM main
VAR result : DINT; c : COUNTER; END_VAR
    c();
    c();
    result := c.q;
END_PROGRAM",
        &[(0, 2)],
    );
}

// REF of a VAR_IN_OUT parameter refers to the caller's variable.
#[test]
fn end_to_end_when_ref_of_in_out_then_refers_to_caller_variable() {
    assert_i32_with(
        "
FUNCTION SET_VIA_REF : BOOL
VAR_IN_OUT v : DINT; END_VAR
VAR p : REF_TO DINT; END_VAR
    p := REF(v);
    p^ := 99;
    SET_VIA_REF := TRUE;
END_FUNCTION
PROGRAM main
VAR x : DINT := 1; ok : BOOL; END_VAR
    ok := SET_VIA_REF(x);
END_PROGRAM",
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        &[(0, 99)],
    );
}

fn assert_not_implemented(source: &str, options: &CompilerOptions) {
    let result = try_parse_and_compile(source, options);
    let diagnostic = result.err().expect("compile should refuse the program");
    assert_eq!(diagnostic.code, "P9999", "{diagnostic:?}");
}

// A STRING lives in the data region, not in one slot, so passing one by
// reference is not implemented. It is refused rather than passed by value.
#[test]
fn compile_when_in_out_string_then_not_implemented() {
    assert_not_implemented(
        "
FUNCTION MY_FUNC : BOOL
VAR_IN_OUT data : STRING[80]; END_VAR
    MY_FUNC := TRUE;
END_FUNCTION
PROGRAM main
VAR s : STRING[80] := 'hello'; result : BOOL; END_VAR
    result := MY_FUNC(data := s);
END_PROGRAM",
        &CompilerOptions::default(),
    );
}

#[test]
fn compile_when_in_out_argument_is_array_element_then_not_implemented() {
    assert_not_implemented(
        "
FUNCTION INC : BOOL
VAR_IN_OUT v : DINT; END_VAR
    v := v + 1;
    INC := TRUE;
END_FUNCTION
PROGRAM main
VAR arr : ARRAY[0..2] OF DINT; ok : BOOL; END_VAR
    ok := INC(arr[1]);
END_PROGRAM",
        &CompilerOptions::default(),
    );
}

#[test]
fn compile_when_in_out_is_for_control_variable_then_not_implemented() {
    assert_not_implemented(
        "
FUNCTION COUNT_UP : BOOL
VAR_IN_OUT i : DINT; END_VAR
    FOR i := 1 TO 3 DO
        COUNT_UP := TRUE;
    END_FOR;
    COUNT_UP := TRUE;
END_FUNCTION
PROGRAM main
VAR x : DINT; ok : BOOL; END_VAR
    ok := COUNT_UP(x);
END_PROGRAM",
        &CompilerOptions::default(),
    );
}
