//! End-to-end integration tests: parse IEC 61131-3 source -> code generation -> VM execution.
//!
//! These tests prove the complete pipeline from source text to executing
//! bytecode, verifying that variables contain the expected values after a scan.
//!
//! Operator-specific tests are in separate files:
//! - end_to_end_add.rs (ADD operator)
//! - end_to_end_sub.rs (SUB operator)
//! - end_to_end_mul.rs (MUL operator)
//! - end_to_end_div.rs (DIV operator)
//! - end_to_end_mod.rs (MOD operator)

mod common;

use common::{parse_and_run, VmBuffers};
use ironplc_codegen::compile;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::Vm;

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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -5);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 0);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 7);
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
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new()
        .load(
            &container,
            &mut bufs.stack,
            &mut bufs.vars,
            &mut bufs.tasks,
            &mut bufs.programs,
            &mut bufs.ready,
        )
        .start();

    // Run multiple scans - result should be the same each time
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 99);

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 99);
    assert_eq!(vm.scan_count(), 2);
}
