//! End-to-end integration tests for the MUL operator.

mod common;

use common::{parse_and_run, VmBuffers};
use ironplc_codegen::compile;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::Vm;

#[test]
fn end_to_end_when_mul_expression_then_variable_has_product() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 7;
  y := x * 6;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_mul_by_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 999 * 0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 0);
}

#[test]
fn end_to_end_when_mul_by_one_then_identity() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 42 * 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_mul_negative_then_negative_result() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 7 * -6;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -42);
}

#[test]
fn end_to_end_when_mul_two_negatives_then_positive() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := -7 * -6;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_chain_of_multiplications_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 2 * 3 * 4;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 24);
}

#[test]
fn end_to_end_when_mul_with_variables_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : INT;
    b : INT;
    c : INT;
  END_VAR
  a := 7;
  b := 6;
  c := a * b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 6);
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_add_and_mul_precedence_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 2 + 3 * 4;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // Multiplication has higher precedence: 2 + (3 * 4) = 14
    assert_eq!(bufs.vars[0].as_i32(), 14);
}

#[test]
fn end_to_end_when_mul_doubling_across_scans_then_accumulates() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := x * 2 + 1;
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

    // Scan 1: x = 0*2+1 = 1
    // Scan 2: x = 1*2+1 = 3
    // Scan 3: x = 3*2+1 = 7
    for _ in 0..3 {
        vm.run_round(0).unwrap();
    }

    assert_eq!(vm.read_variable(0).unwrap(), 7);
}
