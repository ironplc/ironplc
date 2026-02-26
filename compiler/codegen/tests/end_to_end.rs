//! End-to-end integration tests: parse IEC 61131-3 source -> code generation -> VM execution.
//!
//! These tests prove the complete pipeline from source text to executing
//! bytecode, verifying that variables contain the expected values after a scan.

use ironplc_codegen::compile;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::Vm;

fn parse_and_run(source: &str) -> ironplc_vm::VmRunning {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let container = compile(&library).unwrap();
    let mut vm = Vm::new().load(container).start();
    vm.run_round().unwrap();
    vm
}

#[test]
fn end_to_end_when_simple_assignment_then_variable_has_value() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 42);
}

#[test]
fn end_to_end_when_add_expression_then_variable_has_sum() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 10);
    assert_eq!(vm.read_variable(1).unwrap(), 42);
}

#[test]
fn end_to_end_when_chain_of_additions_then_variable_has_total() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 1 + 2 + 3;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 6);
}

#[test]
fn end_to_end_when_multiple_assignments_then_all_variables_correct() {
    let source = "
PROGRAM main
  VAR
    a : INT;
    b : INT;
    c : INT;
  END_VAR
  a := 100;
  b := 200;
  c := a + b;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 100);
    assert_eq!(vm.read_variable(1).unwrap(), 200);
    assert_eq!(vm.read_variable(2).unwrap(), 300);
}

#[test]
fn end_to_end_when_negative_constant_then_variable_is_negative() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := -5;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), -5);
}

#[test]
fn end_to_end_when_zero_then_variable_is_zero() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 0;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn end_to_end_when_variable_copy_then_both_equal() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 7;
  y := x;
END_PROGRAM
";
    let vm = parse_and_run(source);

    assert_eq!(vm.read_variable(0).unwrap(), 7);
    assert_eq!(vm.read_variable(1).unwrap(), 7);
}

#[test]
fn end_to_end_when_multiple_scans_then_idempotent() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 99;
END_PROGRAM
";
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let container = compile(&library).unwrap();
    let mut vm = Vm::new().load(container).start();

    // Run multiple scans - result should be the same each time
    vm.run_round().unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 99);

    vm.run_round().unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 99);
    assert_eq!(vm.scan_count(), 2);
}
