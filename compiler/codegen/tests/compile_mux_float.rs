//! Bytecode-level integration tests for MUX function compilation with float types.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_mux_real_then_produces_f32_builtin() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
  END_VAR
  y := MUX(0, 1.0, 2.0, 3.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // K=0 is an integer constant (i32), IN values are f32
    // BUILTIN MUX_F32_BASE+3 = 0x0443
    // Look for the BUILTIN opcode byte (0xC4) followed by the func_id
    let builtin_pos = bytecode
        .windows(3)
        .position(|w| w[0] == 0xC4 && w[1] == 0x43 && w[2] == 0x04)
        .expect("should contain BUILTIN MUX_F32(3)");
    assert!(builtin_pos > 0);
}

#[test]
fn compile_when_mux_lreal_then_produces_f64_builtin() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
  END_VAR
  y := MUX(1, 1.0, 2.0);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // BUILTIN MUX_F64_BASE+2 = 0x0462
    let builtin_pos = bytecode
        .windows(3)
        .position(|w| w[0] == 0xC4 && w[1] == 0x62 && w[2] == 0x04)
        .expect("should contain BUILTIN MUX_F64(2)");
    assert!(builtin_pos > 0);
}
