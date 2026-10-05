//! Compile tests for how a WSTRING is stored: its header declares a char width
//! of 2, and its literals are UTF-16LE (ADR-0016). The end-to-end tests read a
//! WSTRING by name, so this is the one place these bytes are checked.

use ironplc_container::{opcode, ConstType, FunctionId};
use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_compile;

/// The `max_len` and `char_width` operands of the first `STR_INIT` in the
/// program-init function.
fn str_init_operands(source: &str) -> (u16, u8) {
    let container = parse_and_compile(source, &CompilerOptions::default());
    let init = container
        .code
        .get_function_bytecode(FunctionId::new(0))
        .unwrap();
    let pos = init
        .iter()
        .position(|&b| b == opcode::STR_INIT)
        .expect("the init function initializes the string");
    (
        u16::from_le_bytes([init[pos + 5], init[pos + 6]]),
        init[pos + 7],
    )
}

/// The bytes of every wide string constant, in pool order.
fn wide_constants(source: &str) -> Vec<Vec<u8>> {
    parse_and_compile(source, &CompilerOptions::default())
        .constant_pool
        .iter()
        .filter(|entry| entry.const_type == ConstType::WStr)
        .map(|entry| entry.bytes().to_vec())
        .collect()
}

#[test]
fn compile_when_wstring_declared_then_header_declares_wide_char_width() {
    let operands = str_init_operands(
        "
PROGRAM main
  VAR
    ws : WSTRING[10];
  END_VAR
END_PROGRAM
",
    );

    assert_eq!(operands, (10, 2), "max_len in code units, char_width 2");
}

#[test]
fn compile_when_wstring_literal_then_constant_is_utf16le() {
    // U+00E9 (é) and U+20AC (€) need the high byte of their code unit.
    let wide = wide_constants(
        "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"hé€\";
  END_VAR
END_PROGRAM
",
    );

    assert_eq!(wide, [vec![0x68, 0x00, 0xE9, 0x00, 0xAC, 0x20]]);
}
