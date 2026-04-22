//! End-to-end integration tests for the ADD operator.

#[macro_use]
mod common;

use common::{parse_and_compile, VmBuffers};
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::Vm;

e2e_i32!(
    end_to_end_when_add_expression_then_variable_has_sum,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 10; y := x + 32; END_PROGRAM",
    &[(0, 10), (1, 42)],
);

e2e_i32!(
    end_to_end_when_chain_of_additions_then_variable_has_total,
    "PROGRAM main VAR result : DINT; END_VAR result := 1 + 2 + 3; END_PROGRAM",
    &[(0, 6)],
);

e2e_i32!(
    end_to_end_when_multiple_assignments_then_all_variables_correct,
    "PROGRAM main VAR a : DINT; b : DINT; c : DINT; END_VAR a := 100; b := 200; c := a + b; END_PROGRAM",
    &[(0, 100), (1, 200), (2, 300)],
);

e2e_i32!(
    end_to_end_when_deeply_nested_expression_then_correct_result,
    "PROGRAM main VAR result : DINT; END_VAR result := 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10; END_PROGRAM",
    &[(0, 55)],
);

// Counter across scans: 5 increments → count = 5. Uses the raw VM API.
#[test]
fn end_to_end_when_counter_program_then_increments_across_scans() {
    let source = "PROGRAM main VAR count : DINT; END_VAR count := count + 1; END_PROGRAM";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();
    for _ in 0..5 {
        vm.run_round(0).unwrap();
    }
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 5);
}
