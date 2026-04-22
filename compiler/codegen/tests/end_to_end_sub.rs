//! End-to-end integration tests for the SUB operator.

#[macro_use]
mod common;

use common::{parse_and_compile, VmBuffers};
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::Vm;

// Simple single-var DINT result using various forms of `-`.
e2e_i32!(
    end_to_end_when_sub_result_negative_then_correct,
    "PROGRAM main VAR result : DINT; END_VAR result := 3 - 10; END_PROGRAM",
    &[(0, -7)],
);

e2e_i32!(
    end_to_end_when_chain_of_subtractions_then_correct,
    "PROGRAM main VAR result : DINT; END_VAR result := 100 - 30 - 20 - 10; END_PROGRAM",
    &[(0, 40)],
);

e2e_i32!(
    end_to_end_when_mixed_add_sub_then_correct,
    "PROGRAM main VAR result : DINT; END_VAR result := 10 + 5 - 3; END_PROGRAM",
    &[(0, 12)],
);

e2e_i32!(
    end_to_end_when_sub_zero_then_identity,
    "PROGRAM main VAR x : DINT; END_VAR x := 42 - 0; END_PROGRAM",
    &[(0, 42)],
);

e2e_i32!(
    end_to_end_when_sub_from_zero_then_negation,
    "PROGRAM main VAR x : DINT; END_VAR x := 0 - 7; END_PROGRAM",
    &[(0, -7)],
);

// 10 - (-5) = 15.
e2e_i32!(
    end_to_end_when_sub_negative_constant_then_effective_addition,
    "PROGRAM main VAR x : DINT; END_VAR x := 10 - -5; END_PROGRAM",
    &[(0, 15)],
);

// Subtraction with 2 declared vars (x, y).
e2e_i32!(
    end_to_end_when_sub_expression_then_variable_has_difference,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 10; y := x - 3; END_PROGRAM",
    &[(0, 10), (1, 7)],
);

// Subtraction with 3 declared vars (a, b, c).
e2e_i32!(
    end_to_end_when_sub_with_variables_then_correct,
    "PROGRAM main VAR a : DINT; b : DINT; c : DINT; END_VAR a := 100; b := 30; c := a - b; END_PROGRAM",
    &[(0, 100), (1, 30), (2, 70)],
);

// Multi-scan countdown uses the raw VM API; keep as a dedicated test.
#[test]
fn end_to_end_when_countdown_program_then_decrements_across_scans() {
    let source = "PROGRAM main VAR count : DINT; END_VAR count := count - 1; END_PROGRAM";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();

    for _ in 0..5 {
        vm.run_round(0).unwrap();
    }

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), -5);
}
