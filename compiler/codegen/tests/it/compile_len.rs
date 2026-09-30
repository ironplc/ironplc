//! Bytecode-level tests for folding `LEN` of a string that cannot change --
//! structure only. Behaviour is covered by `end_to_end_len.rs`.
//!
//! `LEN` of an operand whose length is known at compile time -- a literal,
//! a `CONCAT` of such operands, or a `CONSTANT` string variable -- is one
//! integer constant. These pin that such a call loads that constant and
//! nothing else: no data-region slot for the operand, no temp buffer to
//! carry it there, and no `LEN_STR` to read it back.

use ironplc_container::{ConstantIndex, Container, FunctionId};
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};

fn compile(source: &str) -> Container {
    parse_and_compile(source, &CompilerOptions::default())
}

/// Bytecode of the scan function (`FunctionId(1)`).
fn scan_bytecode(container: &Container) -> Vec<u8> {
    container
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap()
        .to_vec()
}

fn i32_constant(container: &Container, index: u16) -> i32 {
    container
        .constant_pool
        .get_i32(ConstantIndex::new(index))
        .unwrap()
}

#[test]
fn compile_len_when_string_literal_then_loads_constant_length() {
    let container = compile(
        "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('Hello');
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_i32(0), // pool:0 (5)
            bc::store_var_i32(0),  // var:0 (n)
            bc::ret_void(),
        ]
    );
    assert_eq!(i32_constant(&container, 0), 5);
}

#[test]
fn compile_len_when_string_literal_then_no_data_region_or_temp_buffer() {
    let container = compile(
        "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN('Hello');
END_PROGRAM
",
    );

    assert_eq!(container.header.data_region_bytes, 0);
    assert_eq!(container.header.num_temp_bufs, 0);
    assert_eq!(container.header.max_temp_buf_bytes, 0);
}

#[test]
fn compile_len_when_wstring_literal_with_escapes_then_loads_code_unit_count() {
    // `$0041` and `$"` are one code unit each, as is `é`.
    let container = compile(
        "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(\"$0041$\"é\");
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_i32(0), // pool:0 (3)
            bc::store_var_i32(0),  // var:0 (n)
            bc::ret_void(),
        ]
    );
    assert_eq!(i32_constant(&container, 0), 3);
    assert_eq!(container.header.data_region_bytes, 0);
}

#[test]
fn compile_len_when_concat_of_literals_then_loads_constant_length() {
    let container = compile(
        "
PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := LEN(CONCAT('ab', 'cde'));
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_i32(0), // pool:0 (5)
            bc::store_var_i32(0),  // var:0 (n)
            bc::ret_void(),
        ]
    );
    assert_eq!(i32_constant(&container, 0), 5);
    assert_eq!(container.header.data_region_bytes, 0);
    assert_eq!(container.header.num_temp_bufs, 0);
}

#[test]
fn compile_len_when_declared_constant_string_then_loads_constant_length() {
    // The variable keeps its slot -- it is still a variable of the program --
    // but LEN no longer reads it. The init function adds 'Hello' to the pool
    // first, so the length is at pool:1.
    let container = compile(
        "
PROGRAM main
  VAR CONSTANT
    msg : STRING := 'Hello';
  END_VAR
  VAR
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_i32(1), // pool:1 (5)
            bc::store_var_i32(1),  // var:1 (n)
            bc::ret_void(),
        ]
    );
    assert_eq!(i32_constant(&container, 1), 5);
}

#[test]
fn compile_len_when_never_written_string_then_loads_constant_length() {
    // `msg` is never written, so the analyzer marks it CONSTANT and codegen
    // folds it exactly as the declared case above.
    let container = compile(
        "
PROGRAM main
  VAR
    msg : STRING := 'Hello';
    n : INT;
  END_VAR
  n := LEN(msg);
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_i32(1), // pool:1 (5)
            bc::store_var_i32(1),  // var:1 (n)
            bc::ret_void(),
        ]
    );
    assert_eq!(i32_constant(&container, 1), 5);
}

#[test]
fn compile_len_when_written_string_then_reads_current_length() {
    // A variable the program writes can change, so its length is read at
    // run time from the slot's header.
    let container = compile(
        "
PROGRAM main
  VAR
    msg : STRING := 'Hello';
    n : INT;
  END_VAR
  msg := 'Hi';
  n := LEN(msg);
END_PROGRAM
",
    );

    assert_bytecode!(
        &scan_bytecode(&container),
        [
            bc::load_const_str(1), // pool:1 ('Hi')
            bc::str_store_var(0),  // msg
            bc::len_str(0),        // msg
            bc::trunc_i16(),
            bc::store_var_i32(1), // var:1 (n)
            bc::ret_void(),
        ]
    );
}
