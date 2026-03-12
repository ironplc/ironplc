//! Bytecode-level integration tests for MUX function compilation with LINT type.

mod common;

use common::parse;
use ironplc_codegen::compile;

#[test]
fn compile_when_mux_lint_then_produces_i64_builtin() {
    let source = "
PROGRAM main
  VAR
    k : DINT;
    y : LINT;
  END_VAR
  k := 1;
  y := MUX(k, 100, 200, 300);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();

    let bytecode = container.code.get_function_bytecode(1).unwrap();
    // BUILTIN MUX_I64_BASE+3 = 0x0423
    let builtin_pos = bytecode
        .windows(3)
        .position(|w| w[0] == 0xC4 && w[1] == 0x23 && w[2] == 0x04)
        .expect("should contain BUILTIN MUX_I64(3)");
    assert!(builtin_pos > 0);
}
