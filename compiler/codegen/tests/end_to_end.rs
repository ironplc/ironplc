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
//! - end_to_end_pow.rs (POW/EXPT operator)
//! - end_to_end_neg.rs (NEG unary operator)
//! - end_to_end_cmp.rs (comparison operators)
//! - end_to_end_bool.rs (boolean operators)
//! - end_to_end_types.rs (multi-width integer type tests)
//! - end_to_end_float.rs (REAL/LREAL floating-point type tests)
//! - end_to_end_bitstring.rs (BYTE/WORD/DWORD/LWORD bit string type tests)

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use common::{parse_and_compile, parse_and_run, VmBuffers};
use ironplc_vm::Vm;

#[rstest]
#[case::simple_assignment("DINT", "x := 42;", 42)]
#[case::negative_constant("DINT", "x := -5;", -5)]
#[case::zero("DINT", "x := 0;", 0)]
#[case::dint_initial_value("DINT := 100", "", 100)]
#[case::lint_initial_value("LINT := 1000000", "", 1_000_000)]
#[case::sint_initial_value("SINT := 42", "", 42)]
#[case::usint_initial_value("USINT := 200", "", 200)]
#[case::uint_initial_value("UINT := 50000", "", 50_000)]
#[case::udint_initial_value("UDINT := 100000", "", 100_000)]
#[case::ulint_initial_value("ULINT := 5000000", "", 5_000_000)]
fn end_to_end_single_var_scalar(
    #[case] decl: &str,
    #[case] body: &str,
    #[case] expected: i64,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {decl};
  END_VAR
  {body}
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i64(), expected);
}

#[test]
fn end_to_end_when_variable_copy_then_both_equal() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 7;
  y := x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 7);
}

#[test]
fn end_to_end_when_int_initial_value_then_used_in_expression() {
    let source = "
PROGRAM main
  VAR
    x : INT := 10;
    y : INT := 32;
  END_VAR
  y := y + x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 10);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_mixed_initialized_and_uninitialized_then_correct() {
    let source = "
PROGRAM main
  VAR
    a : DINT := 5;
    b : DINT;
    c : DINT := 15;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    assert_eq!(bufs.vars[0].as_i32(), 5);
    assert_eq!(bufs.vars[1].as_i32(), 0);
    assert_eq!(bufs.vars[2].as_i32(), 15);
}

#[test]
fn end_to_end_when_multiple_scans_then_idempotent() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();

    // Run multiple scans - result should be the same each time
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 99);

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 99);
    assert_eq!(vm.scan_count(), 2);
}

#[test]
fn end_to_end_when_init_with_accumulator_then_init_runs_once() {
    // x starts at 10, each scan adds 1. If init re-ran every scan,
    // x would be 11 after every scan. With separate init, x accumulates.
    let source = "
PROGRAM main
  VAR
    x : DINT := 10;
  END_VAR
  x := x + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();

    // After scan 1: x = 10 + 1 = 11
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 11);

    // After scan 2: x = 11 + 1 = 12 (NOT 11, which would mean re-init)
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 12);

    // After scan 3: x = 12 + 1 = 13
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 13);
}
