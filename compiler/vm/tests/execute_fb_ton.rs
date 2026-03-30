mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::VarIndex;

#[test]
fn ton_when_in_false_then_q_false_et_zero() {
    let c = common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    // var[1] = IN = 0 (FALSE) — default
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_true_before_pt_then_q_false_et_increasing() {
    let c = common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Set IN = TRUE
    vm.write_variable(VarIndex::new(1), 1).unwrap();

    // Scan at t=1s: rising edge
    vm.run_round(1_000_000).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE (just started)

    // Scan at t=3s: 2 seconds elapsed
    vm.run_round(3_000_000).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE (< PT)
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 2000); // ET = 2s = 2000 ms
}

#[test]
fn ton_when_in_true_after_pt_then_q_true_et_clamped() {
    let c = common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE

    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 5000); // ET clamped to PT (5000 ms)
}

#[test]
fn ton_when_in_falls_after_timer_expires_then_resets() {
    let c = common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: timer expired

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE

    // IN goes FALSE
    vm.write_variable(VarIndex::new(1), 0).unwrap();
    vm.run_round(8_000_000).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_false_before_pt_then_resets() {
    let c = common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(3_000_000).unwrap(); // t=3s: 2s elapsed, not yet expired

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE

    // IN goes FALSE before PT expires
    vm.write_variable(VarIndex::new(1), 0).unwrap();
    vm.run_round(4_000_000).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0

    // IN goes TRUE again — timer restarts from scratch
    vm.write_variable(VarIndex::new(1), 1).unwrap();
    vm.run_round(5_000_000).unwrap(); // new rising edge at t=5s
    vm.run_round(8_000_000).unwrap(); // t=8s: 3s elapsed (< 5s PT)

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE (< PT from new start)
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 3000); // ET = 3s = 3000 ms
}
