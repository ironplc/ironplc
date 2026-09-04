//! Bytecode-level tests for the arithmetic-identity peephole — structure
//! only. Behaviour is covered by `end_to_end_arith_identity.rs`.
//!
//! With the constant on the right, `x + 0`, `x - 0`, `x * 1` and `x / 1`
//! compile down to a bare load and store. On REAL/LREAL `x + 0.0` is the
//! exception and keeps its `ADD`: `(-0.0) + 0.0` is `+0.0` under IEEE 754,
//! so removing the add would change the sign of the result.
//!
//! With the constant on the left the constant load is never adjacent to the
//! operator, so nothing is removed. `0 - x` and `1 / x` are not identities
//! anyway; `0 + x` and `1 * x` are, but the pass does not see them.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{bc, parse_and_compile};

/// Bytecode of the scan function (`FunctionId(1)`) for a program that
/// declares `x` and `y` of type `ty` and executes `y := <expr>;`.
fn scan_bytecode(ty: &str, expr: &str) -> Vec<u8> {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {ty};
    y : {ty};
  END_VAR
  y := {expr};
END_PROGRAM
"
    );
    let container = parse_and_compile(&source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// The load, store and constant encodings for one operand width.
struct Width {
    load_var: fn(u16) -> Vec<u8>,
    store_var: fn(u16) -> Vec<u8>,
    load_const: fn(u16) -> Vec<u8>,
}

const I32: Width = Width {
    load_var: bc::load_var_i32,
    store_var: bc::store_var_i32,
    load_const: bc::load_const_i32,
};
const I64: Width = Width {
    load_var: bc::load_var_i64,
    store_var: bc::store_var_i64,
    load_const: bc::load_const_i64,
};
const F32: Width = Width {
    load_var: bc::load_var_f32,
    store_var: bc::store_var_f32,
    load_const: bc::load_const_f32,
};
const F64: Width = Width {
    load_var: bc::load_var_f64,
    store_var: bc::store_var_f64,
    load_const: bc::load_const_f64,
};

#[rstest]
#[case::dint_plus_zero("DINT", "x + 0", I32)]
#[case::dint_minus_zero("DINT", "x - 0", I32)]
#[case::dint_times_one("DINT", "x * 1", I32)]
#[case::dint_over_one("DINT", "x / 1", I32)]
#[case::lint_plus_zero("LINT", "x + 0", I64)]
#[case::lint_minus_zero("LINT", "x - 0", I64)]
#[case::lint_times_one("LINT", "x * 1", I64)]
#[case::lint_over_one("LINT", "x / 1", I64)]
#[case::real_minus_zero("REAL", "x - 0.0", F32)]
#[case::real_times_one("REAL", "x * 1.0", F32)]
#[case::real_over_one("REAL", "x / 1.0", F32)]
#[case::lreal_minus_zero("LREAL", "x - 0.0", F64)]
#[case::lreal_times_one("LREAL", "x * 1.0", F64)]
#[case::lreal_over_one("LREAL", "x / 1.0", F64)]
fn compile_when_identity_with_constant_second_then_operation_removed(
    #[case] ty: &str,
    #[case] expr: &str,
    #[case] width: Width,
) {
    let bytecode = scan_bytecode(ty, expr);

    assert_bytecode!(
        &bytecode,
        [
            (width.load_var)(0),  // var:0 (x)
            (width.store_var)(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[rstest]
#[case::real_plus_zero("REAL", "x + 0.0", F32, opcode::ADD_F32)]
#[case::lreal_plus_zero("LREAL", "x + 0.0", F64, opcode::ADD_F64)]
fn compile_when_float_plus_zero_then_add_kept(
    #[case] ty: &str,
    #[case] expr: &str,
    #[case] width: Width,
    #[case] op: u8,
) {
    let bytecode = scan_bytecode(ty, expr);

    assert_bytecode!(
        &bytecode,
        [
            (width.load_var)(0),   // var:0 (x)
            (width.load_const)(0), // pool:0 (0.0)
            [op],
            (width.store_var)(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}

#[rstest]
#[case::zero_plus_dint("DINT", "0 + x", I32, opcode::ADD_I32)]
#[case::zero_minus_dint("DINT", "0 - x", I32, opcode::SUB_I32)]
#[case::one_times_dint("DINT", "1 * x", I32, opcode::MUL_I32)]
#[case::one_over_dint("DINT", "1 / x", I32, opcode::DIV_I32)]
#[case::zero_plus_lint("LINT", "0 + x", I64, opcode::ADD_I64)]
#[case::zero_minus_lint("LINT", "0 - x", I64, opcode::SUB_I64)]
#[case::zero_plus_real("REAL", "0.0 + x", F32, opcode::ADD_F32)]
#[case::zero_minus_real("REAL", "0.0 - x", F32, opcode::SUB_F32)]
#[case::one_times_real("REAL", "1.0 * x", F32, opcode::MUL_F32)]
#[case::one_over_real("REAL", "1.0 / x", F32, opcode::DIV_F32)]
#[case::zero_plus_lreal("LREAL", "0.0 + x", F64, opcode::ADD_F64)]
#[case::zero_minus_lreal("LREAL", "0.0 - x", F64, opcode::SUB_F64)]
fn compile_when_constant_first_then_operation_kept(
    #[case] ty: &str,
    #[case] expr: &str,
    #[case] width: Width,
    #[case] op: u8,
) {
    let bytecode = scan_bytecode(ty, expr);

    assert_bytecode!(
        &bytecode,
        [
            (width.load_const)(0), // pool:0 (0 or 1)
            (width.load_var)(0),   // var:0 (x)
            [op],
            (width.store_var)(1), // var:1 (y)
            bc::ret_void(),
        ]
    );
}
