//! End-to-end integration tests: parse IEC 61131-3 source -> code generation -> VM execution.
//!
//! These tests prove the complete pipeline from source text to executing
//! bytecode, verifying that variables contain the expected values after a scan.

use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};

/// Helper struct that allocates Vec-backed buffers for VM usage.
struct VmBuffers {
    stack: Vec<Slot>,
    vars: Vec<Slot>,
    tasks: Vec<TaskState>,
    programs: Vec<ProgramInstanceState>,
    ready: Vec<usize>,
}

impl VmBuffers {
    fn from_container(c: &Container) -> Self {
        let h = &c.header;
        let task_count = c.task_table.tasks.len();
        let program_count = c.task_table.programs.len();
        VmBuffers {
            stack: vec![Slot::default(); h.max_stack_depth as usize],
            vars: vec![Slot::default(); h.num_variables as usize],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
        }
    }
}

fn parse_and_run(source: &str) -> (Container, VmBuffers) {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
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
        vm.run_round(0).unwrap();
    }
    (container, bufs)
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 42);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 42);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 6);
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
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 100);
    assert_eq!(bufs.vars[1].as_i32(), 200);
    assert_eq!(bufs.vars[2].as_i32(), 300);
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
