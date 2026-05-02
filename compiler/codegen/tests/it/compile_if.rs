//! Bytecode-level integration tests for IF/ELSIF/ELSE compilation.

use ironplc_container::opcode::cmp_op;
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_simple_if_then_produces_cmp_br() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  IF x > 0 THEN
    y := 1;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x=var:0, y=var:1
    // Fused emission via CMP_BR_I32 (compare-and-branch): IF x > 0 THEN ...
    // becomes "branch to END if NOT(x > 0)" i.e. cmp_op = LE_S (= negation
    // of GT_S).
    //   0: CMP_BR_I32 LE_S, var:0, pool:0 (0), offset:+6 -> 14
    //   8: LOAD_CONST_I32 pool:1 (1)
    //  11: STORE_VAR_I32 var:1
    //  14: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::cmp_br_i32(cmp_op::LE_S, 0, 0, 6), // exit if NOT (x > 0)
            bc::load_const_i32(1),                 // pool:1 (1)
            bc::store_var_i32(1),                  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_if_else_then_produces_cmp_br_and_jmp() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  IF x > 0 THEN
    y := 1;
  ELSE
    y := 2;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Fused emission via CMP_BR_I32:
    //   0: CMP_BR_I32 LE_S, var:0, pool:0 (0), offset:+9 -> 17
    //   8: LOAD_CONST_I32 pool:1 (1)
    //  11: STORE_VAR_I32 var:1
    //  14: JMP offset:+6 -> 23
    //  17: LOAD_CONST_I32 pool:2 (2)
    //  20: STORE_VAR_I32 var:1
    //  23: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::cmp_br_i32(cmp_op::LE_S, 0, 0, 9), // exit-to-ELSE if NOT (x > 0)
            bc::load_const_i32(1),                 // pool:1 (1)
            bc::store_var_i32(1),                  // var:1
            bc::jmp(6),                            // offset:+6
            bc::load_const_i32(2),                 // pool:2 (2)
            bc::store_var_i32(1),                  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_if_elsif_else_then_produces_chained_cmp_br() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  IF x > 5 THEN
    y := 1;
  ELSIF x > 0 THEN
    y := 2;
  ELSE
    y := 3;
  END_IF;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Fused emission via CMP_BR_I32:
    //   0: CMP_BR_I32 LE_S, var:0, pool:0 (5), offset:+9 -> 17
    //   8: LOAD_CONST_I32 pool:1 (1)
    //  11: STORE_VAR_I32 var:1
    //  14: JMP offset:+23 -> 40
    //  17: CMP_BR_I32 LE_S, var:0, pool:2 (0), offset:+9 -> 34
    //  25: LOAD_CONST_I32 pool:3 (2)
    //  28: STORE_VAR_I32 var:1
    //  31: JMP offset:+6 -> 40
    //  34: LOAD_CONST_I32 pool:4 (3)
    //  37: STORE_VAR_I32 var:1
    //  40: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::cmp_br_i32(cmp_op::LE_S, 0, 0, 9), // (0) exit-to-ELSIF if NOT (x > 5)
            bc::load_const_i32(1),                 // (8) pool:1 (1)
            bc::store_var_i32(1),                  // (11) var:1
            bc::jmp(23),                           // (14) offset:+23 -> END
            bc::cmp_br_i32(cmp_op::LE_S, 0, 2, 9), // (17) exit-to-ELSE if NOT (x > 0)
            bc::load_const_i32(3),                 // (25) pool:3 (2)
            bc::store_var_i32(1),                  // (28) var:1
            bc::jmp(6),                            // (31) offset:+6 -> END
            bc::load_const_i32(4),                 // (34) pool:4 (3)
            bc::store_var_i32(1),                  // (37) var:1
            bc::ret_void(),                        // (40)
        ]
    );
}
