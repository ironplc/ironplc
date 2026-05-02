//! Bytecode-level integration tests for IF/ELSIF/ELSE compilation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

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
    assert_eq!(
        bytecode,
        &[
            0xF4, 0x03, 0x00, 0x00, 0x00, 0x00, 0x06,
            0x00, // CMP_BR_I32 LE_S, var:0, pool:0, +6
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x8C, // RET_VOID
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
    assert_eq!(
        bytecode,
        &[
            0xF4, 0x03, 0x00, 0x00, 0x00, 0x00, 0x09,
            0x00, // CMP_BR_I32 LE_S, var:0, pool:0, +9
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x7C, 0x06, 0x00, // JMP offset:+6
            0x00, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (2)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x8C, // RET_VOID
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
    assert_eq!(
        bytecode,
        &[
            0xF4, 0x03, 0x00, 0x00, 0x00, 0x00, 0x09,
            0x00, // CMP_BR_I32 LE_S, var:0, pool:0 (5), +9        (0)
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)                       (8)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1                              (11)
            0x7C, 0x17, 0x00, // JMP offset:+23                                   (14)
            0xF4, 0x03, 0x00, 0x00, 0x02, 0x00, 0x09,
            0x00, // CMP_BR_I32 LE_S, var:0, pool:2 (0), +9       (17)
            0x00, 0x03, 0x00, // LOAD_CONST_I32 pool:3 (2)                       (25)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1                              (28)
            0x7C, 0x06, 0x00, // JMP offset:+6                                    (31)
            0x00, 0x04, 0x00, // LOAD_CONST_I32 pool:4 (3)                       (34)
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1                              (37)
            0x8C, // RET_VOID                                          (40)
        ]
    );
}
