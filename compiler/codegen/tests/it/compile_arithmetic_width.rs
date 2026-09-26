//! Bytecode-level tests for the width a numeric operator computes at
//! (`specs/design/arithmetic-operator-overloads.md`, Codegen). Behaviour is
//! covered by `end_to_end_arithmetic_overloads.rs`.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{bc, parse_and_compile};

/// Bytecode of the scan function (`FunctionId(1)`) of `source`.
fn scan_bytecode(source: &str) -> Vec<u8> {
    let container = parse_and_compile(source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

/// A program declaring `x`, `z` and `y` of type `ty` and executing
/// `y := x OP z`. Two operands, since the peephole pass turns a repeated load
/// of one variable into a duplicate.
fn same_type_program(ty: &str, op: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    x : {ty};
    z : {ty};
    y : {ty};
  END_VAR
  y := x {op} z;
END_PROGRAM
"
    )
}

/// The load and store encodings for one operation width.
struct Width {
    load_var: fn(u16) -> Vec<u8>,
    store_var: fn(u16) -> Vec<u8>,
}

const I32: Width = Width {
    load_var: bc::load_var_i32,
    store_var: bc::store_var_i32,
};
const I64: Width = Width {
    load_var: bc::load_var_i64,
    store_var: bc::store_var_i64,
};
const F32: Width = Width {
    load_var: bc::load_var_f32,
    store_var: bc::store_var_f32,
};
const F64: Width = Width {
    load_var: bc::load_var_f64,
    store_var: bc::store_var_f64,
};

/// REQ-AO-codegen-006: where the operands and the target share an operation
/// width and signedness, the expression compiles as it always has: two loads,
/// the operator at that width, a store. No conversion is added.
#[rstest]
#[case::dint_add("DINT", "+", I32, opcode::ADD_I32)]
#[case::dint_mul("DINT", "*", I32, opcode::MUL_I32)]
#[case::dint_div("DINT", "/", I32, opcode::DIV_I32)]
#[case::udint_div("UDINT", "/", I32, opcode::DIV_U32)]
#[case::lint_sub("LINT", "-", I64, opcode::SUB_I64)]
#[case::ulint_div("ULINT", "/", I64, opcode::DIV_U64)]
#[case::real_mul("REAL", "*", F32, opcode::MUL_F32)]
#[case::lreal_div("LREAL", "/", F64, opcode::DIV_F64)]
fn compile_when_operands_and_target_share_width_then_unchanged(
    #[case] ty: &str,
    #[case] op: &str,
    #[case] width: Width,
    #[case] opcode: u8,
) {
    let bytecode = scan_bytecode(&same_type_program(ty, op));

    assert_bytecode!(
        &bytecode,
        [
            (width.load_var)(0), // var:0 (x)
            (width.load_var)(1), // var:1 (z)
            [opcode],
            (width.store_var)(2), // var:2 (y)
            bc::ret_void(),
        ]
    );
}

/// REQ-AO-codegen-012: an expression whose result type is not a concrete
/// elementary numeric type (here a subrange of `LINT`) compiles at the
/// enclosing operation type, as before.
#[test]
fn compile_when_subrange_expression_then_compiles_at_enclosing_type() {
    let bytecode = scan_bytecode(
        "
TYPE
  BIG : LINT (0..10000000000);
END_TYPE
PROGRAM main
  VAR
    x : BIG;
    z : BIG;
    y : BIG;
  END_VAR
  y := x + z;
END_PROGRAM
",
    );

    assert_bytecode!(
        &bytecode,
        [
            bc::load_var_i64(0), // var:0 (x)
            bc::load_var_i64(1), // var:1 (z)
            bc::add_i64(),
            bc::store_var_i64(2), // var:2 (y)
            bc::ret_void(),
        ]
    );
}
