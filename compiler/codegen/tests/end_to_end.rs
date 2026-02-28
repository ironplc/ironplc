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

#[test]
fn end_to_end_when_counter_program_then_increments_across_scans() {
    let source = "
PROGRAM main
  VAR
    count : INT;
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
    result : INT;
  END_VAR
  result := 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    assert_eq!(bufs.vars[0].as_i32(), 55);
}

#[test]
fn end_to_end_when_sub_expression_then_variable_has_difference() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
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
    result : INT;
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
    result : INT;
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
    result : INT;
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
    a : INT;
    b : INT;
    c : INT;
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
    x : INT;
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
    x : INT;
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
    count : INT;
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
    x : INT;
  END_VAR
  x := 10 - -5;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // 10 - (-5) = 15
    assert_eq!(bufs.vars[0].as_i32(), 15);
}

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
