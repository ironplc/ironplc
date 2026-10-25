//! Bytecode-level integration tests for CASE statement compilation.

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

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
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x=var:0, y=var:1. The selector is evaluated once (REQ-LOW-codegen-124)
    // and stays on the stack while the labels are tested; the arm that runs
    // pops it first, and so does the end of the statement when none does.
    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: DUP
    //   4: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   7: EQ_I32
    //   8: JMP_IF_NOT offset:+10 -> 21 (skip arm body)
    //  11: POP                         (the selector)
    //  12: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  15: STORE_VAR_I32 var:1
    //  18: JMP offset:+1 -> 22         (jump to END)
    //  21: POP                         (no arm ran)
    //  22: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // var:0
            bc::dup(),
            bc::load_const_i32(0), // pool:0 (1)
            bc::eq_i32(),
            bc::jmp_if_not(10), // offset:+10
            bc::pop(),
            bc::load_const_i32(1), // pool:1 (10)
            bc::store_var_i32(1),  // var:1
            bc::jmp(1),            // offset:+1
            bc::pop(),
            bc::ret_void(),
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
    let container = parse_and_compile(source, &CompilerOptions::default());

    // Bytecode layout:
    //   0: LOAD_VAR_I32 var:0          (selector)
    //   3: DUP
    //   4: LOAD_CONST_I32 pool:0 (1)   (case value)
    //   7: EQ_I32
    //   8: JMP_IF_NOT offset:+10 -> 21 (skip arm body)
    //  11: POP                         (the selector)
    //  12: LOAD_CONST_I32 pool:1 (10)  (y := 10)
    //  15: STORE_VAR_I32 var:1
    //  18: JMP offset:+7 -> 28         (jump past ELSE to END)
    //  21: POP                         (the selector)
    //  22: LOAD_CONST_I32 pool:2 (99)  (ELSE: y := 99)
    //  25: STORE_VAR_I32 var:1
    //  28: RET_VOID
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // var:0
            bc::dup(),
            bc::load_const_i32(0), // pool:0 (1)
            bc::eq_i32(),
            bc::jmp_if_not(10), // offset:+10
            bc::pop(),
            bc::load_const_i32(1), // pool:1 (10)
            bc::store_var_i32(1),  // var:1
            bc::jmp(7),            // offset:+7
            bc::pop(),
            bc::load_const_i32(2), // pool:2 (99)
            bc::store_var_i32(1),  // var:1
            bc::ret_void(),
        ]
    );
}
