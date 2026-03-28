//! End-to-end integration tests for bare literal type inference (ANY_INT / ANY_REAL).
//!
//! Bare integer literals (e.g. `5`) resolve as ANY_INT and are compatible with
//! any integer parameter type. Bare real literals resolve as ANY_REAL and are
//! compatible with any real parameter type.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_bare_int_literal_to_int_param_then_correct() {
    let source = "
FUNCTION ADD_ONE : INT
VAR_INPUT
    x : INT;
END_VAR
    ADD_ONE := x + INT#1;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_ONE(5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 6);
}

#[test]
fn end_to_end_when_bare_int_literal_to_sint_param_then_correct() {
    let source = "
FUNCTION DOUBLE : SINT
VAR_INPUT
    x : SINT;
END_VAR
    DOUBLE := x + x;
END_FUNCTION

PROGRAM main
VAR
    result : SINT;
END_VAR
    result := DOUBLE(7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 14);
}

#[test]
fn end_to_end_when_bare_int_literal_to_dint_param_then_correct() {
    let source = "
FUNCTION TRIPLE : DINT
VAR_INPUT
    x : DINT;
END_VAR
    TRIPLE := x + x + x;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
END_VAR
    result := TRIPLE(100);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 300);
}

#[test]
fn end_to_end_when_bare_real_literal_to_lreal_param_then_correct() {
    let source = "
FUNCTION ADD_PI : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
    ADD_PI := x;
END_FUNCTION

PROGRAM main
VAR
    result : LREAL;
END_VAR
    result := ADD_PI(3.14);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let val = bufs.vars[0].as_f64();
    assert!((val - 3.14).abs() < 0.001);
}

#[test]
fn end_to_end_when_bare_literal_in_expression_with_int_var_then_correct() {
    let source = "
PROGRAM main
VAR
    x : INT;
    result : INT;
END_VAR
    x := INT#10;
    result := x + 5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 15);
}
