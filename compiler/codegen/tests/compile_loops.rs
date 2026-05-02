//! Bytecode-level integration tests for WHILE, REPEAT, and FOR loop compilation.

mod common;
use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_while_with_simple_cmp_then_emits_do_while_cmp_br() {
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
    //
    // After do-while restructure with CMP_BR fusion:
    //   0: CMP_BR_I32 LE_S, var:0, pool:0 (0), offset:+18 -> 26  (zero-trip)
    //   8: LOAD_VAR_I32 var:0          (body: x)
    //  11: LOAD_CONST_I32 pool:1 (1)   (body: 1)
    //  14: SUB_I32                      (x - 1)
    //  15: STORE_VAR_I32 var:0         (x := ...)
    //  18: CMP_BR_I32 GT_S, var:0, pool:0 (0), offset:-18 -> 8   (back-edge)
    //  26: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    // Two CMP_BR_I32 instructions, no JMP_IF_NOT, no JMP.
    let cmp_br_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::CMP_BR_I32)
        .count();
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    let jmp_count = bytecode.iter().filter(|&&b| b == opcode::JMP).count();
    assert_eq!(cmp_br_count, 2, "bytecode = {bytecode:?}");
    assert_eq!(jmp_if_not_count, 0, "bytecode = {bytecode:?}");
    assert_eq!(jmp_count, 0, "bytecode = {bytecode:?}");

    assert_eq!(
        bytecode,
        &[
            0xF4, 0x03, 0x00, 0x00, 0x00, 0x00, 0x12,
            0x00, // CMP_BR_I32 LE_S, var:0, pool:0, +18 (zero-trip)
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x24, // SUB_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xF4, 0x04, 0x00, 0x00, 0x00, 0x00, 0xEE,
            0xFF, // CMP_BR_I32 GT_S, var:0, pool:0, -18 (back-edge)
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_repeat_until_with_simple_cmp_then_emits_cmp_br_back_edge() {
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

    // x=var:0, constants: pool:0=5, pool:1=1
    // (try_classify_cmp pools the until literal first; the body's `1` is
    // pooled second.)
    //
    //   0: LOAD_VAR_I32 var:0          (body: x)
    //   3: LOAD_CONST_I32 pool:1 (1)   (body: 1)
    //   6: ADD_I32                      (x + 1)
    //   7: STORE_VAR_I32 var:0         (x := ...)
    //  10: CMP_BR_I32 LE_S, var:0, pool:0 (5), offset:-18 -> 0   (back-edge if !until)
    //  18: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    // One CMP_BR_I32, no JMP_IF_NOT.
    let cmp_br_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::CMP_BR_I32)
        .count();
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    assert_eq!(cmp_br_count, 1, "bytecode = {bytecode:?}");
    assert_eq!(jmp_if_not_count, 0, "bytecode = {bytecode:?}");

    assert_eq!(
        bytecode,
        &[
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool:1 (1)
            0x20, // ADD_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xF4, 0x03, 0x00, 0x00, 0x00, 0x00, 0xEE,
            0xFF, // CMP_BR_I32 LE_S, var:0, pool:0 (5), -18
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_default_step_then_produces_loop_with_cmp_br() {
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
    //
    //   0: LOAD_CONST_I32 pool:0 (1)   (from value)
    //   3: STORE_VAR_I32 var:0         (i := 1)
    //   6: CMP_BR_I32 GT_S, var:0, pool:1 (5), offset:+23 -> 37  (exit if i > 5)
    //  14: LOAD_VAR_I32 var:1          (BODY: y)
    //  17: LOAD_VAR_I32 var:0          (i)
    //  20: ADD_I32                      (y + i)
    //  21: STORE_VAR_I32 var:1         (y := ...)
    //  24: LOAD_VAR_I32 var:0          (increment: i)
    //  27: LOAD_CONST_I32 pool:0 (1)   (step: 1)
    //  30: ADD_I32                      (i + 1)
    //  31: STORE_VAR_I32 var:0         (i := ...)
    //  34: JMP offset:-31 -> 6         (back to head test)
    //  37: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();

    let cmp_br_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::CMP_BR_I32)
        .count();
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    let jmp_count = bytecode.iter().filter(|&&b| b == opcode::JMP).count();
    assert_eq!(cmp_br_count, 1, "bytecode = {bytecode:?}");
    assert_eq!(jmp_if_not_count, 0, "bytecode = {bytecode:?}");
    assert_eq!(jmp_count, 1, "bytecode = {bytecode:?}");

    assert_eq!(
        bytecode,
        &[
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0xF4, 0x04, 0x00, 0x00, 0x01, 0x00, 0x17,
            0x00, // CMP_BR_I32 GT_S, var:0, pool:1 (5), +23
            0x0C, 0x01, 0x00, // LOAD_VAR_I32 var:1
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x20, // ADD_I32
            0x10, 0x01, 0x00, // STORE_VAR_I32 var:1
            0x0C, 0x00, 0x00, // LOAD_VAR_I32 var:0
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool:0 (1)
            0x20, // ADD_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var:0
            0x7C, 0xE1, 0xFF, // JMP offset:-31
            0x8C, // RET_VOID
        ]
    );
}

#[test]
fn compile_when_for_negative_step_then_produces_loop_with_cmp_br_lt() {
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

    // FOR head test fused into one CMP_BR_I32 with cmp_op = LT_S
    // (negation of the negative-step continuation predicate `i >= to`).
    let cmp_br_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::CMP_BR_I32)
        .count();
    let jmp_if_not_count = bytecode
        .iter()
        .filter(|&&b| b == opcode::JMP_IF_NOT)
        .count();
    let jmp_count = bytecode.iter().filter(|&&b| b == opcode::JMP).count();
    assert_eq!(cmp_br_count, 1, "bytecode = {bytecode:?}");
    assert_eq!(jmp_if_not_count, 0, "bytecode = {bytecode:?}");
    assert_eq!(jmp_count, 1, "bytecode = {bytecode:?}");

    // The CMP_BR opcode is at offset 6 (after LOAD_CONST + STORE_VAR for the
    // initial assignment). The cmp_op operand at offset 7 must be LT_S.
    assert_eq!(bytecode[6], opcode::CMP_BR_I32);
    assert_eq!(bytecode[7], opcode::cmp_op::LT_S);
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
