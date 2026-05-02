//! Bytecode-level integration tests for IF/ELSIF/ELSE compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

#[test]
fn compile_when_simple_if_then_produces_jmp_if_not() {
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
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0
    //   3: LOAD_CONST_I32 pool:0 (0)
    //   6: GT_I32
    //   7: JMP_IF_NOT offset:+6 -> 16
    //  10: LOAD_CONST_I32 pool:1 (1)
    //  13: STORE_VAR_I32 var:1
    //  16: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (0)
            bc::gt_i32(),
            bc::jmp_if_not(6),     // offset:+6
            bc::load_const_i32(1), // pool:1 (1)
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_if_else_then_produces_jmp_and_jmp_if_not() {
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

    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0
    //   3: LOAD_CONST_I32 pool:0 (0)
    //   6: GT_I32
    //   7: JMP_IF_NOT offset:+9 -> 19
    //  10: LOAD_CONST_I32 pool:1 (1)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+6 -> 25
    //  19: LOAD_CONST_I32 pool:2 (2)
    //  22: STORE_VAR_I32 var:1
    //  25: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0
            bc::load_const_i32(0), // pool:0 (0)
            bc::gt_i32(),
            bc::jmp_if_not(9),     // offset:+9
            bc::load_const_i32(1), // pool:1 (1)
            bc::store_var_i32(1),  // var:1
            bc::jmp(6),            // offset:+6
            bc::load_const_i32(2), // pool:2 (2)
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
        ]
    );
}

#[test]
fn compile_when_if_elsif_else_then_produces_chained_jumps() {
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

    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0
    //   3: LOAD_CONST_I32 pool:0 (5)
    //   6: GT_I32
    //   7: JMP_IF_NOT offset:+9 -> 19
    //  10: LOAD_CONST_I32 pool:1 (1)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+25 -> 44
    //  19: LOAD_VAR_I32 var:0
    //  22: LOAD_CONST_I32 pool:2 (0)
    //  25: GT_I32
    //  26: JMP_IF_NOT offset:+9 -> 38
    //  29: LOAD_CONST_I32 pool:3 (2)
    //  32: STORE_VAR_I32 var:1
    //  35: JMP offset:+6 -> 44
    //  38: LOAD_CONST_I32 pool:4 (3)
    //  41: STORE_VAR_I32 var:1
    //  44: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0),   // var:0         (0)
            bc::load_const_i32(0), // pool:0 (5)  (3)
            bc::gt_i32(),          // (6)
            bc::jmp_if_not(9),     // offset:+9       (7)
            bc::load_const_i32(1), // pool:1 (1)  (10)
            bc::store_var_i32(1),  // var:1         (13)
            bc::jmp(25),           // offset:+25              (16)
            bc::load_var_i32(0),   // var:0         (19)
            bc::load_const_i32(2), // pool:2 (0)  (22)
            bc::gt_i32(),          // (25)
            bc::jmp_if_not(9),     // offset:+9       (26)
            bc::load_const_i32(3), // pool:3 (2)  (29)
            bc::store_var_i32(1),  // var:1         (32)
            bc::jmp(6),            // offset:+6              (35)
            bc::load_const_i32(4), // pool:4 (3)  (38)
            bc::store_var_i32(1),  // var:1         (41)
            bc::ret_void(),        // (44)
        ]
    );
}
