//! End-to-end tests for R_TRIG (rising edge detector) function block.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;
use ironplc_container::VarIndex;

const PROGRAM: &str = "
PROGRAM main
  VAR
    edge : R_TRIG;
    clk : BOOL;
    result : BOOL;
  END_VAR
  edge(CLK := clk, Q => result);
END_PROGRAM
";

#[test]
fn end_to_end_when_r_trig_clk_false_then_q_false() {
    let (_container, bufs) = parse_and_run(PROGRAM, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), 0, "Q should be FALSE");
}

#[test]
fn end_to_end_when_r_trig_rising_edge_then_q_true() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // First scan: CLK=FALSE
        vm.run_round(0).unwrap();
        // Rising edge: CLK=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE on rising edge"
        );
    });
}

#[test]
fn end_to_end_when_r_trig_clk_stays_true_then_q_false_next_scan() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // Rising edge
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q TRUE on rising edge"
        );
        // CLK stays TRUE — Q should return to FALSE
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q should be FALSE when CLK stays TRUE"
        );
    });
}

#[test]
fn end_to_end_when_r_trig_second_rising_edge_then_q_true_again() {
    parse_and_run_rounds(PROGRAM, &CompilerOptions::default(), |vm| {
        // First rising edge
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1);
        // CLK falls
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0);
        // Second rising edge
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(2).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE on second rising edge"
        );
    });
}
