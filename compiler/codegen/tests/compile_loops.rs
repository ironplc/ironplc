//! Bytecode-level integration tests for WHILE, REPEAT, and FOR loop compilation.

mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_while_then_produces_loop_with_jmp_if_not() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  WHILE x > 0 DO
    x := x - 1;
  END_WHILE;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

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
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

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
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (0)
            0x6C, // GT_I32
            0xB2, 0x0D, 0x00, // JMP_IF_NOT offset:+13
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x24, // SUB_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
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
    x : DINT;
  END_VAR
  REPEAT
    x := x + 1;
  UNTIL x > 5
  END_REPEAT;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x=var:0, constants: pool:0=1, pool:1=5
    // Bytecode layout (store-load optimization inserts DUP before STORE x):
    //   0: LOAD_VAR_I32 var:0          (body: x)
    //   3: LOAD_CONST_I32 pool:0 (1)   (body: 1)
    //   6: ADD_I32                      (x + 1)
    //   7: DUP                          (store-load optimization)
    //   8: STORE_VAR_I32 var:0         (x := ...)
    //  11: LOAD_CONST_I32 pool:1 (5)   (condition: 5)
    //  14: GT_I32                       (x > 5)
    //  15: JMP_IF_NOT offset:-18 -> 0  (back to LOOP if false)
    //  18: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    // Verify JMP_IF_NOT at offset 15 with backward offset -18
    assert_eq!(bytecode[15], 0xB2); // JMP_IF_NOT
    assert_eq!(i16::from_le_bytes([bytecode[16], bytecode[17]]), -18);

    assert_eq!(
        bytecode,
        &[
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x20, // ADD_I32
            0xA1, // DUP (store-load optimization)
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            0x6C, // GT_I32
            0xB2, 0xEE, 0xFF, // JMP_IF_NOT offset:-18
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_default_step_then_produces_loop_with_le() {
    let source = "
PROGRAM main
  VAR
    i : DINT;
    y : DINT;
  END_VAR
  FOR i := 1 TO 5 DO
    y := y + i;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // i=var:0, y=var:1
    // constants: pool:0=1, pool:1=5
    // Bytecode layout (specs/plans/2026-04-30-elide-for-loop-exit-jmp.md):
    //   0: LOAD_CONST_I32 pool:0 (1)   (from value)
    //   3: STORE_VAR_I32 var:0         (i := 1)
    //   6: LOAD_VAR_I32 var:0          (LOOP: load i)
    //   9: LOAD_CONST_I32 pool:1 (5)   (to value)
    //  12: LE_I32                       (i <= 5? continuation)
    //  13: JMP_IF_NOT offset:+23 -> 39 (exit when i > 5)
    //  16: LOAD_VAR_I32 var:1          (BODY: y)
    //  19: LOAD_VAR_I32 var:0          (i)
    //  22: ADD_I32                      (y + i)
    //  23: STORE_VAR_I32 var:1         (y := ...)
    //  26: LOAD_VAR_I32 var:0          (increment: i)
    //  29: LOAD_CONST_I32 pool:0 (1)   (step: 1)
    //  32: ADD_I32                      (i + 1)
    //  33: STORE_VAR_I32 var:0         (i := ...)
    //  36: JMP offset:-33 -> 6         (back to LOOP)
    //  39: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    // Verify LE_I32 for positive step (replaces old GT_I32)
    assert_eq!(bytecode[12], opcode::LE_I32);

    // Exactly one JMP_IF_NOT (the exit) and one JMP (the loop back-edge) —
    // the per-iteration exit JMP that used to follow JMP_IF_NOT body_label is
    // now elided.
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    let jmp_count = bytecode.iter().filter(|&&b| b == opcode::JMP).count();
    assert_eq!(jmp_if_not_count, 1, "bytecode = {bytecode:?}");
    assert_eq!(jmp_count, 1, "bytecode = {bytecode:?}");

    // Verify structure
    assert_eq!(
        bytecode,
        &[
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (5)
            0x6B, // LE_I32
            0xB2, 0x17, 0x00, // JMP_IF_NOT offset:+23
            0x0C, 0x01, 0x00, // LOAD_VAR_I32 var:1
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x20, // ADD_I32
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x20, // ADD_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xB0, 0xDF, 0xFF, // JMP offset:-33
            0xB5, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_negative_step_then_produces_loop_with_ge() {
    let source = "
PROGRAM main
  VAR
    i : DINT;
    y : DINT;
  END_VAR
  FOR i := 5 TO 1 BY -1 DO
    y := y + i;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    // Verify GE_I32 for negative step (continuation predicate; replaces old LT_I32)
    assert_eq!(bytecode[12], opcode::GE_I32);

    // And confirm the per-iteration exit JMP is gone here too.
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    let jmp_count = bytecode.iter().filter(|&&b| b == opcode::JMP).count();
    assert_eq!(jmp_if_not_count, 1, "bytecode = {bytecode:?}");
    assert_eq!(jmp_count, 1, "bytecode = {bytecode:?}");
}

// FOR-loop TRUNC elision (specs/plans/2026-04-30-elide-for-loop-trunc.md):
// the per-iteration TRUNC opcode is elided when the control variable's bounds
// are constants that keep every visible value (init, body, and the
// post-final-increment) within the declared narrow type's range.

#[test]
fn compile_when_for_int_with_safe_constant_bounds_then_omits_trunc() {
    // Body uses a DINT sink so the only candidate TRUNC opcodes are the two
    // that wrap the FOR-loop's init and increment.
    let source = "
PROGRAM main
  VAR
    i : INT;
    sink : DINT;
  END_VAR
  FOR i := 1 TO 100 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        !bytecode.contains(&opcode::TRUNC_I16),
        "TRUNC_I16 should be elided for in-range constant bounds; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_int_with_boundary_to_then_emits_trunc() {
    // to + step = 32767 + 1 = 32768 overflows INT, so TRUNC must remain to
    // preserve the wrap-around terminating behaviour.
    let source = "
PROGRAM main
  VAR
    i : INT;
    sink : DINT;
  END_VAR
  FOR i := 1 TO 32767 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        bytecode.contains(&opcode::TRUNC_I16),
        "TRUNC_I16 must remain at boundary to=INT_MAX; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_int_with_non_constant_to_then_emits_trunc() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    n : INT;
    sink : DINT;
  END_VAR
  FOR i := 1 TO n DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        bytecode.contains(&opcode::TRUNC_I16),
        "TRUNC_I16 must remain when 'to' is non-constant; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_sint_with_safe_constant_bounds_then_omits_trunc() {
    let source = "
PROGRAM main
  VAR
    i : SINT;
    sink : DINT;
  END_VAR
  FOR i := 1 TO 10 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        !bytecode.contains(&opcode::TRUNC_I8),
        "TRUNC_I8 should be elided for in-range constant bounds; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_uint_with_safe_constant_bounds_then_omits_trunc() {
    let source = "
PROGRAM main
  VAR
    i : UINT;
    sink : DINT;
  END_VAR
  FOR i := 1 TO 100 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        !bytecode.contains(&opcode::TRUNC_U16),
        "TRUNC_U16 should be elided for in-range constant bounds; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_sint_negative_step_at_boundary_then_emits_trunc() {
    // to + step = -128 + (-1) = -129 underflows SINT, so TRUNC must remain.
    let source = "
PROGRAM main
  VAR
    i : SINT;
    sink : DINT;
  END_VAR
  FOR i := 0 TO -128 BY -1 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        bytecode.contains(&opcode::TRUNC_I8),
        "TRUNC_I8 must remain at boundary to=SINT_MIN with negative step; bytecode = {bytecode:?}"
    );
}

#[test]
fn compile_when_for_int_negative_step_safe_then_omits_trunc() {
    let source = "
PROGRAM main
  VAR
    i : INT;
    sink : DINT;
  END_VAR
  FOR i := 100 TO 1 BY -1 DO
    sink := sink + 1;
  END_FOR;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    assert!(
        !bytecode.contains(&opcode::TRUNC_I16),
        "TRUNC_I16 should be elided for in-range constant bounds with negative step; bytecode = {bytecode:?}"
    );
}
