//! Bytecode-level integration tests for CASE statement compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_case_single_arm_then_produces_eq_and_jmp() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  CASE x OF
    1: y := 10;
  END_CASE;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // x=var:0, y=var:1
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   6: EQ_I32
    //   7: JMP_IF_NOT offset:+9 -> 19  (skip arm body)
    //  10: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+3 -> 22         (jump to END)
    //  19: (next_label — no more arms, no ELSE)
    //  19: (end_label)
    //  19: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x68, // EQ_I32
            0xB2, 0x09, 0x00, // JMP_IF_NOT offset:+9
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (10)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB0, 0x00, 0x00, // JMP offset:+0 (end_label is right here)
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_case_with_else_then_produces_jmp_to_end() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  CASE x OF
    1: y := 10;
  ELSE
    y := 99;
  END_CASE;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   6: EQ_I32
    //   7: JMP_IF_NOT offset:+9 -> 19  (skip arm body)
    //  10: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  13: STORE_VAR_I32 var:1
    //  16: JMP offset:+6 -> 25         (jump past ELSE to END)
    //  19: LOAD_CONST_I32 pool:2 (99)  (ELSE: y := 99)
    //  22: STORE_VAR_I32 var:1
    //  25: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x68, // EQ_I32
            0xB2, 0x09, 0x00, // JMP_IF_NOT offset:+9
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (10)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB0, 0x06, 0x00, // JMP offset:+6
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (99)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
        ]
    );
}
