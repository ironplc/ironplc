//! End-to-end tests for enumeration code generation.
//!
//! Tests the complete pipeline from IEC 61131-3 source with enumeration
//! types through code generation. Later PRs will add execution tests
//! once variable allocation and expression compilation are implemented.

mod common;

use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn end_to_end_when_enum_type_declared_then_compiles_without_error() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
END_PROGRAM
";
    let _container = parse_and_compile(source, &CompilerOptions::default());
}

#[test]
fn end_to_end_when_multiple_enum_types_then_compiles_without_error() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
END_PROGRAM
";
    let _container = parse_and_compile(source, &CompilerOptions::default());
}
