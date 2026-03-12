mod common;
use common::VmBuffers;
use ironplc_container::opcode;

#[test]
fn tp_when_in_false_then_q_false_et_zero() {
    let c = common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    // var[1] = IN = 0 (FALSE) — default
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(3).unwrap(), 0); // ET = 0
}

#[test]
fn tp_when_in_true_before_pt_then_q_true_et_increasing() {
    let c = common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Set IN = TRUE
    vm.write_variable(1, 1).unwrap();

    // Scan at t=1s: rising edge, pulse starts
    vm.run_round(1_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (pulse active)

    // Scan at t=3s: 2 seconds elapsed
    vm.run_round(3_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (< PT)
    assert_eq!(vm.read_variable(3).unwrap(), 2000); // ET = 2s = 2000 ms
}

#[test]
fn tp_when_in_true_after_pt_then_q_false_et_clamped() {
    let c = common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE

    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge, pulse starts
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (pulse ended)
    assert_eq!(vm.read_variable(3).unwrap(), 5000); // ET clamped to PT (5000 ms)
}

#[test]
fn tp_when_in_falls_during_pulse_then_pulse_continues() {
    let c = common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge, pulse starts
    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE

    // IN goes FALSE mid-pulse — pulse should continue
    vm.write_variable(1, 0).unwrap();
    vm.run_round(3_000_000).unwrap(); // t=3s: 2s elapsed, still pulsing

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (pulse ignores IN)
    assert_eq!(vm.read_variable(3).unwrap(), 2000); // ET = 2s = 2000 ms

    // Pulse expires
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT
    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(3).unwrap(), 5000); // ET clamped to PT (5000 ms)
}

#[test]
fn tp_when_retrigger_after_pulse_then_new_pulse() {
    let c = common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // First pulse
    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: pulse expired

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE

    // IN must go FALSE then TRUE for new rising edge
    vm.write_variable(1, 0).unwrap();
    vm.run_round(8_000_000).unwrap(); // t=8s: IN = FALSE

    vm.write_variable(1, 1).unwrap();
    vm.run_round(9_000_000).unwrap(); // t=9s: new rising edge

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE (new pulse)
    assert_eq!(vm.read_variable(3).unwrap(), 0); // ET = 0 (just started)

    // New pulse timing
    vm.run_round(12_000_000).unwrap(); // t=12s: 3s into new pulse
    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable(3).unwrap(), 3000); // ET = 3s = 3000 ms
}
