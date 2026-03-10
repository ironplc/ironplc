mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;

/// Builds a container that runs: load fb_ref, store IN, store PT, call TON, load Q, load ET.
///
/// Variable layout:
///   var[0] = fb_ref (offset 0 into data region)
///   var[1] = IN value (set by test via write_variable)
///   var[2] = Q output (read by test)
///   var[3] = ET output (read by test)
/// Constant layout:
///   constant[0] = PT value (i64 microseconds)
fn ton_test_container(pt_us: i64) -> ironplc_container::Container {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_VAR_I32,     0x01, 0x00,  // push IN from var[1]
        opcode::FB_STORE_PARAM,   0x00,         // store to TON.IN (field 0)
        opcode::LOAD_CONST_I64,   0x00, 0x00,  // push PT constant
        opcode::FB_STORE_PARAM,   0x01,         // store to TON.PT (field 1)
        opcode::FB_CALL,          0x10, 0x00,   // call TON (type_id 0x0010)
        opcode::FB_LOAD_PARAM,    0x02,         // load TON.Q (field 2)
        opcode::STORE_VAR_I32,    0x02, 0x00,   // store Q to var[2]
        opcode::FB_LOAD_PARAM,    0x03,         // load TON.ET (field 3)
        opcode::STORE_VAR_I64,    0x03, 0x00,   // store ET to var[3]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];

    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    ContainerBuilder::new()
        .num_variables(4)
        .data_region_bytes(48) // 6 fields * 8 bytes
        .add_i64_constant(pt_us)
        .add_function(0, &init_bytecode, 0, 4)
        .add_function(1, &bytecode, 16, 4)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

#[test]
fn ton_when_in_false_then_q_false_et_zero() {
    let c = ton_test_container(5_000_000); // PT = 5 seconds
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    // var[1] = IN = 0 (FALSE) — default
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_true_before_pt_then_q_false_et_increasing() {
    let c = ton_test_container(5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    // Set IN = TRUE
    vm.write_variable(1, 1).unwrap();

    // Scan at t=1s: rising edge
    vm.run_round(1_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (just started)

    // Scan at t=3s: 2 seconds elapsed
    vm.run_round(3_000_000).unwrap();
    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (< PT)
    assert_eq!(vm.read_variable_i64(3).unwrap(), 2_000_000); // ET = 2s
}

#[test]
fn ton_when_in_true_after_pt_then_q_true_et_clamped() {
    let c = ton_test_container(5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE

    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 5_000_000); // ET clamped to PT
}

#[test]
fn ton_when_in_falls_after_timer_expires_then_resets() {
    let c = ton_test_container(5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: timer expired

    assert_eq!(vm.read_variable(2).unwrap(), 1); // Q = TRUE

    // IN goes FALSE
    vm.write_variable(1, 0).unwrap();
    vm.run_round(8_000_000).unwrap();

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0
}

#[test]
fn ton_when_in_false_before_pt_then_resets() {
    let c = ton_test_container(5_000_000);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(1, 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(3_000_000).unwrap(); // t=3s: 2s elapsed, not yet expired

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE

    // IN goes FALSE before PT expires
    vm.write_variable(1, 0).unwrap();
    vm.run_round(4_000_000).unwrap();

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable_i64(3).unwrap(), 0); // ET = 0

    // IN goes TRUE again — timer restarts from scratch
    vm.write_variable(1, 1).unwrap();
    vm.run_round(5_000_000).unwrap(); // new rising edge at t=5s
    vm.run_round(8_000_000).unwrap(); // t=8s: 3s elapsed (< 5s PT)

    assert_eq!(vm.read_variable(2).unwrap(), 0); // Q = FALSE (< PT from new start)
    assert_eq!(vm.read_variable_i64(3).unwrap(), 3_000_000); // ET = 3s
}
