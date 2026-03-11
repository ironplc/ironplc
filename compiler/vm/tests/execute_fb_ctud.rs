mod common;
use common::VmBuffers;

#[test]
fn ctud_when_no_edges_then_cv_zero_qd_true() {
    let c = common::ctud_test_container(5);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // All inputs default to FALSE
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(5).unwrap(), 0); // QU = FALSE (0 < 5)
    assert_eq!(vm.read_variable(6).unwrap(), 1); // QD = TRUE (0 <= 0)
    assert_eq!(vm.read_variable(7).unwrap(), 0); // CV = 0
}

#[test]
fn ctud_when_cu_rising_edges_then_cv_increments() {
    let c = common::ctud_test_container(5);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // CU rising edge: FALSE -> TRUE
    vm.write_variable(1, 1).unwrap(); // CU = TRUE
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 1); // CV = 1
    assert_eq!(vm.read_variable(6).unwrap(), 0); // QD = FALSE (1 > 0)

    // CU stays TRUE — no edge, no increment
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 1); // CV still 1

    // CU falling then rising — new edge
    vm.write_variable(1, 0).unwrap();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 1); // CV still 1 (no rising edge)

    vm.write_variable(1, 1).unwrap();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 2); // CV = 2
}

#[test]
fn ctud_when_cd_rising_edges_then_cv_decrements() {
    let c = common::ctud_test_container(5);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // First count up to 3
    for _ in 0..3 {
        vm.write_variable(1, 1).unwrap(); // CU = TRUE
        vm.run_round(0).unwrap();
        vm.write_variable(1, 0).unwrap(); // CU = FALSE
        vm.run_round(0).unwrap();
    }
    assert_eq!(vm.read_variable(7).unwrap(), 3); // CV = 3

    // CD rising edge
    vm.write_variable(2, 1).unwrap(); // CD = TRUE
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 2); // CV = 2

    // CD stays TRUE — no edge
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 2); // CV still 2
}

#[test]
fn ctud_when_cv_reaches_pv_then_qu_true() {
    let c = common::ctud_test_container(3);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Count up to PV (3)
    for i in 0..3 {
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        if i < 2 {
            assert_eq!(
                vm.read_variable(5).unwrap(),
                0,
                "QU should be FALSE at CV={}",
                i + 1
            );
        }
        vm.write_variable(1, 0).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(vm.read_variable(7).unwrap(), 3); // CV = 3
    assert_eq!(vm.read_variable(5).unwrap(), 1); // QU = TRUE (CV >= PV)
}

#[test]
fn ctud_when_reset_then_cv_zero() {
    let c = common::ctud_test_container(5);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Count up to 3
    for _ in 0..3 {
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(1, 0).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(vm.read_variable(7).unwrap(), 3); // CV = 3

    // Reset
    vm.write_variable(3, 1).unwrap(); // R = TRUE
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 0); // CV = 0
    assert_eq!(vm.read_variable(6).unwrap(), 1); // QD = TRUE
}

#[test]
fn ctud_when_load_then_cv_equals_pv() {
    let c = common::ctud_test_container(10);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Load PV into CV
    vm.write_variable(4, 1).unwrap(); // LD = TRUE
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 10); // CV = PV = 10
    assert_eq!(vm.read_variable(5).unwrap(), 1); // QU = TRUE (CV >= PV)
}

#[test]
fn ctud_when_reset_and_load_then_reset_wins() {
    let c = common::ctud_test_container(10);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Both R and LD — reset takes priority
    vm.write_variable(3, 1).unwrap(); // R = TRUE
    vm.write_variable(4, 1).unwrap(); // LD = TRUE
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(7).unwrap(), 0); // CV = 0 (reset wins)
}

#[test]
fn ctud_when_cu_and_cd_simultaneously_then_net_zero() {
    let c = common::ctud_test_container(5);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Both CU and CD rising edge at the same time
    vm.write_variable(1, 1).unwrap(); // CU = TRUE
    vm.write_variable(2, 1).unwrap(); // CD = TRUE
    vm.run_round(0).unwrap();
    // +1 from CU, -1 from CD = net 0
    assert_eq!(vm.read_variable(7).unwrap(), 0); // CV = 0
}
