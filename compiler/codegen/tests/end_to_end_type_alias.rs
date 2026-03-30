//! End-to-end tests for type alias resolution through codegen.
//!
//! These tests validate that the analyzer's `resolve_types` pass correctly
//! resolves type aliases to their elementary types, enabling codegen to select
//! the correct opcodes.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_type_alias_byte_assignment_then_correct() {
    let source = "
TYPE MyByte : BYTE := 0; END_TYPE
PROGRAM main
  VAR
    x : MyByte;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // BYTE is an unsigned 8-bit type; 42 fits within u8 range
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_type_alias_byte_truncation_then_correct() {
    let source = "
TYPE MyByte : BYTE := 0; END_TYPE
PROGRAM main
  VAR
    x : MyByte;
  END_VAR
  x := 300;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // 300 truncated to u8 = 300 - 256 = 44
    assert_eq!(bufs.vars[0].as_i32(), 44);
}

#[test]
fn end_to_end_when_type_alias_int_arithmetic_then_correct() {
    let source = "
TYPE MyInt : INT := 0; END_TYPE
PROGRAM main
  VAR
    x : MyInt;
    y : MyInt;
  END_VAR
  x := 100;
  y := x + 200;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 300);
}

#[test]
fn end_to_end_when_type_alias_int_overflow_then_truncated() {
    let source = "
TYPE MyInt : INT := 0; END_TYPE
PROGRAM main
  VAR
    x : MyInt;
  END_VAR
  x := 40000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // INT is signed 16-bit; 40000 truncated to i16 = 40000 - 65536 = -25536
    assert_eq!(bufs.vars[0].as_i32(), -25536);
}
