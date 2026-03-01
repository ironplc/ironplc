//! Bytecode-level integration tests for EXIT and RETURN statement compilation.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_exit_in_while_then_produces_jmp_to_end() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  WHILE TRUE DO
    EXIT;
  END_WHILE;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Bytecode layout:
    //   0: LOAD_TRUE                    (condition: TRUE)
    //   1: JMP_IF_NOT offset:+6 -> 10  (exit if false)
    //   4: JMP offset:+3 -> 10         (EXIT → jump to end)
    //   7: JMP offset:-10 -> 0         (back to LOOP)
    //  10: RET_VOID
    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // EXIT produces a JMP at offset 4 targeting the same end label as JMP_IF_NOT
    assert_eq!(bytecode[4], 0xB0); // JMP (from EXIT)
    let exit_offset = i16::from_le_bytes([bytecode[5], bytecode[6]]);
    assert_eq!(exit_offset, 3); // jumps forward 3 to offset 10 (end)

    // The end should be RET_VOID
    assert_eq!(bytecode[10], 0xB5); // RET_VOID
}

#[test]
fn compile_when_return_then_produces_ret_void() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
  RETURN;
  x := 20;
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();

    // Bytecode layout:
    //   0: LOAD_CONST_I32 pool:0 (10)
    //   3: STORE_VAR_I32 var:0
    //   6: RET_VOID                     (RETURN)
    //   7: LOAD_CONST_I32 pool:1 (20)   (dead code)
    //  10: STORE_VAR_I32 var:0
    //  13: RET_VOID                     (program end)
    let bytecode = container.code.get_function_bytecode(0).unwrap();

    // RETURN produces RET_VOID at offset 6
    assert_eq!(bytecode[6], 0xB5); // RET_VOID (from RETURN)

    // Program still ends with RET_VOID
    assert_eq!(*bytecode.last().unwrap(), 0xB5); // RET_VOID (program end)
}
