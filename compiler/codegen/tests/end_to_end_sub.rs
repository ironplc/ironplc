//! End-to-end integration tests for the SUB operator.

mod common;

use common::{parse_and_run, VmBuffers};
use ironplc_codegen::compile;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::Vm;

#[test]
fn end_to_end_when_sub_expression_then_variable_has_difference() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x - 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 7);
}

#[test]
fn end_to_end_when_sub_result_negative_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 3 - 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -7);
}

#[test]
fn end_to_end_when_chain_of_subtractions_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 100 - 30 - 20 - 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 40);
}

#[test]
fn end_to_end_when_mixed_add_sub_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 10 + 5 - 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 12);
}

#[test]
fn end_to_end_when_sub_with_variables_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 100;
  b := 30;
  c := a - b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 30);
    assert_eq!(bufs.vars[2].as_i32(), 70);
}

#[test]
fn end_to_end_when_sub_zero_then_identity() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42 - 0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_sub_from_zero_then_negation() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0 - 7;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), -7);
}

#[test]
fn end_to_end_when_countdown_program_then_decrements_across_scans() {
    let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count - 1;
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

    for _ in 0..5 {
        vm.run_round(0).unwrap();
    }

    assert_eq!(vm.read_variable(0).unwrap(), -5);
}

#[test]
fn end_to_end_when_sub_negative_constant_then_effective_addition() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10 - -5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // 10 - (-5) = 15
    assert_eq!(bufs.vars[0].as_i32(), 15);
}
