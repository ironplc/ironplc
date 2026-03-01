//! End-to-end integration tests for the ADD operator.

mod common;

use common::{parse_and_run, VmBuffers};
use ironplc_codegen::compile;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::Vm;

#[test]
fn end_to_end_when_add_expression_then_variable_has_sum() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_chain_of_additions_then_variable_has_total() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 1 + 2 + 3;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 6);
}

#[test]
fn end_to_end_when_multiple_assignments_then_all_variables_correct() {
    let source = "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 100;
  b := 200;
  c := a + b;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 200);
    assert_eq!(bufs.vars[2].as_i32(), 300);
}

#[test]
fn end_to_end_when_counter_program_then_increments_across_scans() {
    let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
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

    assert_eq!(vm.read_variable(0).unwrap(), 5);
}

#[test]
fn end_to_end_when_deeply_nested_expression_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 55);
}
