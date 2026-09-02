//! Bytecode-level tests for the arithmetic-identity peephole — structure
//! only. Behaviour is covered by `end_to_end_arith_identity.rs`.
//!
//! `x + 0` and `x - 0` on integers compile down to a bare load and store.
//! On REAL/LREAL only `x - 0.0` does: `x + 0.0` must keep its `ADD`, because
//! `(-0.0) + 0.0` is `+0.0` under IEEE 754 and removing the add would change
//! the sign of the result.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

/// Bytecode of the scan function (`FunctionId(1)`).
fn scan_bytecode(source: &str) -> Vec<u8> {
    let container = parse_and_compile(source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

#[test]
fn compile_when_dint_plus_zero_then_add_removed() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := x + 0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_i32(0),  // var:0 (x)
            bc::store_var_i32(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_lint_plus_zero_then_add_removed() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  y := x + 0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_i64(0),  // var:0 (x)
            bc::store_var_i64(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_dint_minus_zero_then_sub_removed() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := x - 0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_i32(0),  // var:0 (x)
            bc::store_var_i32(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_real_plus_zero_then_add_kept() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := x + 0.0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_f32(0),   // var:0 (x)
            bc::load_const_f32(0), // pool:0 (0.0)
            bc::add_f32(),
            bc::store_var_f32(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_lreal_plus_zero_then_add_kept() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := x + 0.0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_f64(0),   // var:0 (x)
            bc::load_const_f64(0), // pool:0 (0.0)
            bc::add_f64(),
            bc::store_var_f64(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_real_minus_zero_then_sub_removed() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  y := x - 0.0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_f32(0),  // var:0 (x)
            bc::store_var_f32(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_lreal_minus_zero_then_sub_removed() {
    let bytecode = scan_bytecode(
        "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  y := x - 0.0;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_f64(0),  // var:0 (x)
            bc::store_var_f64(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}
