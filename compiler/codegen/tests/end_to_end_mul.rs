//! End-to-end integration tests for the MUL operator.

#[macro_use]
mod common;

use common::{parse_and_compile, VmBuffers};
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::Vm;

// Single-var DINT result: `result := <expr>;`.
e2e_i32!(
    end_to_end_when_mul_by_zero_then_zero,
    "PROGRAM main VAR result : DINT; END_VAR result := 999 * 0; END_PROGRAM",
    &[(0, 0)],
);

e2e_i32!(
    end_to_end_when_mul_by_one_then_identity,
    "PROGRAM main VAR result : DINT; END_VAR result := 42 * 1; END_PROGRAM",
    &[(0, 42)],
);

e2e_i32!(
    end_to_end_when_mul_negative_then_negative_result,
    "PROGRAM main VAR result : DINT; END_VAR result := 7 * -6; END_PROGRAM",
    &[(0, -42)],
);

e2e_i32!(
    end_to_end_when_mul_two_negatives_then_positive,
    "PROGRAM main VAR result : DINT; END_VAR result := -7 * -6; END_PROGRAM",
    &[(0, 42)],
);

e2e_i32!(
    end_to_end_when_chain_of_multiplications_then_correct,
    "PROGRAM main VAR result : DINT; END_VAR result := 2 * 3 * 4; END_PROGRAM",
    &[(0, 24)],
);

// MUL precedes ADD: 2 + (3 * 4) = 14.
e2e_i32!(
    end_to_end_when_add_and_mul_precedence_then_correct,
    "PROGRAM main VAR result : DINT; END_VAR result := 2 + 3 * 4; END_PROGRAM",
    &[(0, 14)],
);

// 2-var form: x := 7; y := x * 6;
e2e_i32!(
    end_to_end_when_mul_expression_then_variable_has_product,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 7; y := x * 6; END_PROGRAM",
    &[(0, 7), (1, 42)],
);

// 3-var form: a := 7; b := 6; c := a * b;
e2e_i32!(
    end_to_end_when_mul_with_variables_then_correct,
    "PROGRAM main VAR a : DINT; b : DINT; c : DINT; END_VAR a := 7; b := 6; c := a * b; END_PROGRAM",
    &[(0, 7), (1, 6), (2, 42)],
);

// Multi-scan doubling: uses the raw VM API to run 3 rounds.
//   Scan 1: x = 0*2+1 = 1; Scan 2: 1*2+1 = 3; Scan 3: 3*2+1 = 7.
#[test]
fn end_to_end_when_mul_doubling_across_scans_then_accumulates() {
    let source = "PROGRAM main VAR x : DINT; END_VAR x := x * 2 + 1; END_PROGRAM";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();

    for _ in 0..3 {
        vm.run_round(0).unwrap();
    }

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 7);
}
