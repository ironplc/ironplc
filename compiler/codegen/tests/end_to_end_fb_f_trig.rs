//! End-to-end tests for F_TRIG (falling edge detector) function block.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;
use ironplc_container::VarIndex;

const PROGRAM: &str = "
PROGRAM main
  VAR
    edge : F_TRIG;
    clk : BOOL;
    result : BOOL;
  END_VAR
  edge(CLK := clk, Q => result);
END_PROGRAM
";

#[test]
fn end_to_end_when_f_trig_clk_false_then_q_false() {
    let (_container, bufs) = parse_and_run(PROGRAM, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), 0, "Q should be FALSE");
}

#[test]
fn end_to_end_when_f_trig_falling_edge_then_q_true() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // CLK=TRUE first
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q FALSE while CLK rising"
        );
        // Falling edge: CLK=FALSE
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE on falling edge"
        );
    });
}

#[test]
fn end_to_end_when_f_trig_clk_stays_false_then_q_false_next_scan() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // CLK=TRUE then FALSE (falling edge)
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q TRUE on falling edge"
        );
        // CLK stays FALSE
        vm.run_round(2).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q should be FALSE when CLK stays FALSE"
        );
    });
}

#[test]
fn end_to_end_when_f_trig_second_falling_edge_then_q_true_again() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // First cycle: TRUE then FALSE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1);
        // Second cycle: TRUE then FALSE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(2).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0);
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(3).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE on second falling edge"
        );
    });
}
