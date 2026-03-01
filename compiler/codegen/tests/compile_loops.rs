//! Bytecode-level integration tests for WHILE, REPEAT, and FOR loop compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_while_then_produces_loop_with_jmp_if_not() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  WHILE x > 0 DO
    x := x - 1;
  END_WHILE;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // x=var:0, constants: pool:0=0, pool:1=1
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (condition: x)
    //   3: LOAD_CONST_I32 pool:0 (0)   (condition: 0)
    //   6: GT_I32                       (x > 0)
    //   7: JMP_IF_NOT offset:+13 -> 23 (exit if false)
    //  10: LOAD_VAR_I32 var:0          (body: x)
    //  13: LOAD_CONST_I32 pool:1 (1)   (body: 1)
    //  16: SUB_I32                      (x - 1)
    //  17: STORE_VAR_I32 var:0         (x := ...)
    //  20: JMP offset:-23 -> 0         (back to LOOP)
    //  23: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // Verify JMP_IF_NOT at offset 7 with forward offset +13
    assert_eq!(bytecode[7], 0xB2); // JMP_IF_NOT
    assert_eq!(i16::from_le_bytes([bytecode[8], bytecode[9]]), 13);

    // Verify JMP at offset 20 with backward offset -23
    assert_eq!(bytecode[20], 0xB0); // JMP
    assert_eq!(i16::from_le_bytes([bytecode[21], bytecode[22]]), -23);

    // Verify overall structure
    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x6C, // GT_I32
            0xB2, 0x0D, 0x00, // JMP_IF_NOT offset:+13
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x31, // SUB_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB0, 0xE9, 0xFF, // JMP offset:-23
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_repeat_until_then_produces_backward_jmp_if_not() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  REPEAT
    x := x + 1;
  UNTIL x > 5
  END_REPEAT;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // x=var:0, constants: pool:0=1, pool:1=5
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (body: x)
    //   3: LOAD_CONST_I32 pool:0 (1)   (body: 1)
    //   6: ADD_I32                      (x + 1)
    //   7: STORE_VAR_I32 var:0         (x := ...)
    //  10: LOAD_VAR_I32 var:0          (condition: x)
    //  13: LOAD_CONST_I32 pool:1 (5)   (condition: 5)
    //  16: GT_I32                       (x > 5)
    //  17: JMP_IF_NOT offset:-20 -> 0  (back to LOOP if false)
    //  20: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // Verify JMP_IF_NOT at offset 17 with backward offset -20
    assert_eq!(bytecode[17], 0xB2); // JMP_IF_NOT
    assert_eq!(i16::from_le_bytes([bytecode[18], bytecode[19]]), -20);

    assert_eq!(
        bytecode,
        &[
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x30, // ADD_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            0x6C, // GT_I32
            0xB2, 0xEC, 0xFF, // JMP_IF_NOT offset:-20
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_default_step_then_produces_loop_with_gt() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    y : INT;
  END_VAR
  FOR i := 1 TO 5 DO
    y := y + i;
  END_FOR;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // i=var:0, y=var:1
    // constants: pool:0=1, pool:1=5
    // Bytecode layout:
    //   0: LOAD_CONST_I32 pool:0 (1)   (from value)
    //   3: STORE_VAR_I32 var:0         (i := 1)
    //   6: LOAD_VAR_I32 var:0          (LOOP: load i)
    //   9: LOAD_CONST_I32 pool:1 (5)   (to value)
    //  12: GT_I32                       (i > 5?)
    //  13: JMP_IF_NOT offset:+3 -> 19  (if not past limit, go to BODY)
    //  16: JMP offset:+23 -> 42        (exit loop)
    //  19: LOAD_VAR_I32 var:1          (BODY: y)
    //  22: LOAD_VAR_I32 var:0          (i)
    //  25: ADD_I32                      (y + i)
    //  26: STORE_VAR_I32 var:1         (y := ...)
    //  29: LOAD_VAR_I32 var:0          (increment: i)
    //  32: LOAD_CONST_I32 pool:0 (1)   (step: 1)
    //  35: ADD_I32                      (i + 1)
    //  36: STORE_VAR_I32 var:0         (i := ...)
    //  39: JMP offset:-36 -> 6         (back to LOOP)
    //  42: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // Verify GT_I32 for positive step
    assert_eq!(bytecode[12], 0x6C); // GT_I32

    // Verify structure
    assert_eq!(
        bytecode,
        &[
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            0x6C, // GT_I32
            0xB2, 0x03, 0x00, // JMP_IF_NOT offset:+3
            0xB0, 0x17, 0x00, // JMP offset:+23
            0x10, 0x01, 0x00, // LOAD_VAR_I32 var:1
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x30, // ADD_I32
            0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x30, // ADD_I32
            0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB0, 0xDC, 0xFF, // JMP offset:-36
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_negative_step_then_produces_loop_with_lt() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    y : INT;
  END_VAR
  FOR i := 5 TO 1 BY -1 DO
    y := y + i;
  END_FOR;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // Verify LT_I32 for negative step (instead of GT_I32)
    assert_eq!(bytecode[12], 0x6A); // LT_I32
}
