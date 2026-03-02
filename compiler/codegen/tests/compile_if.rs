//! Bytecode-level integration tests for IF/ELSIF/ELSE compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

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
    let library = parse(source);
    let container = compile(&library).unwrap();

    // x=var:0, y=var:1
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0
    //   3: LOAD_CONST_I32 pool:0 (0)
    //   6: GT_I32
    //   7: JMP_IF_NOT offset:+6 -> 16
    //  10: LOAD_CONST_I32 pool:1 (1)
    //  13: STORE_VAR_I32 var:1
    //  16: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x6C, // GT_I32
            0xB2, 0x06, 0x00, // JMP_IF_NOT offset:+6
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
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
    let library = parse(source);
    let container = compile(&library).unwrap();

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
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x6C, // GT_I32
            0xB2, 0x09, 0x00, // JMP_IF_NOT offset:+9
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB0, 0x06, 0x00, // JMP offset:+6
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (2)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0xB5, // RET_VOID
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
    let library = parse(source);
    let container = compile(&library).unwrap();

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
    let bytecode = container.code.get_function_bytecode(0).unwrap();
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0         (0)
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (5)  (3)
            0x6C, // GT_I32                      (6)
            0xB2, 0x09, 0x00, // JMP_IF_NOT offset:+9       (7)
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)  (10)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1         (13)
            0xB0, 0x19, 0x00, // JMP offset:+25              (16)
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0         (19)
            0x01, 0x02, 0x00, // LOAD_CONST_I32 pool:2 (0)  (22)
            0x6C, // GT_I32                      (25)
            0xB2, 0x09, 0x00, // JMP_IF_NOT offset:+9       (26)
            0x01, 0x03, 0x00, // LOAD_CONST_I32 pool:3 (2)  (29)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1         (32)
            0xB0, 0x06, 0x00, // JMP offset:+6              (35)
            0x01, 0x04, 0x00, // LOAD_CONST_I32 pool:4 (3)  (38)
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1         (41)
            0xB5, // RET_VOID                    (44)
        ]
    );
}
