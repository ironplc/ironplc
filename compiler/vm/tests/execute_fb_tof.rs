mod common;
use common::VmBuffers;
use ironplc_container::opcode;

#[test]
fn tof_when_in_true_then_q_true_et_zero() {
    let c = common::timer_test_container(5_000_000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Set IN = TRUE
    vm.write_variable(1, 1).unwrap();
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0
}

#[test]
fn tof_when_in_false_before_pt_then_q_true_et_increasing() {
    let c = common::timer_test_container(5_000_000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // IN = TRUE first
    vm.write_variable(1, 1).unwrap();
    vm.run_round(1_000_000).unwrap(); // t=1s: Q=TRUE

    // IN falls to FALSE
    vm.write_variable(1, 0).unwrap();
    vm.run_round(2_000_000).unwrap(); // t=2s: falling edge, starts timing

    // Still timing
    vm.run_round(4_000_000).unwrap(); // t=4s: 2s elapsed
    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (still in off-delay)
    assert_eq!(vm.read_variable_i64(3).unwrap(), 2_000_000); // ET = 2s
}

#[test]
fn tof_when_in_false_after_pt_then_q_false_et_clamped() {
    let c = common::timer_test_container(5_000_000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // IN = TRUE first
    vm.write_variable(1, 1).unwrap();
    vm.run_round(1_000_000).unwrap(); // t=1s

    // IN falls
    vm.write_variable(1, 0).unwrap();
    vm.run_round(2_000_000).unwrap(); // t=2s: falling edge

    // After PT
    vm.run_round(8_000_000).unwrap(); // t=8s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 5_000_000); // ET clamped to PT
}

#[test]
fn tof_when_in_rises_during_timing_then_resets() {
    let c = common::timer_test_container(5_000_000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // IN = TRUE
    vm.write_variable(1, 1).unwrap();
    vm.run_round(1_000_000).unwrap(); // t=1s

    // IN falls
    vm.write_variable(1, 0).unwrap();
    vm.run_round(2_000_000).unwrap(); // t=2s: falling edge
    vm.run_round(4_000_000).unwrap(); // t=4s: 2s elapsed, timing

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (still timing)

    // IN goes TRUE again before PT expires
    vm.write_variable(1, 1).unwrap();
    vm.run_round(5_000_000).unwrap(); // t=5s

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0 (reset)
}

#[test]
fn tof_when_in_never_true_then_q_false() {
    let c = common::timer_test_container(5_000_000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // IN = FALSE (default), no prior TRUE state
    vm.run_round(1_000_000).unwrap(); // t = 1s

    // First scan with IN=FALSE starts timing from "falling edge"
    // Q starts TRUE (off-delay holds Q=TRUE during timing)
    // After PT expires, Q goes FALSE
    vm.run_round(7_000_000).unwrap(); // t=7s: > 5s elapsed

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE after PT
}
