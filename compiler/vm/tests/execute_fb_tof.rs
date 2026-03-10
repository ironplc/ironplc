mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;

/// Builds a container that runs: load fb_ref, store IN, store PT, call TOF, load Q, load ET.
///
/// Variable layout:
///   var[0] = fb_ref (offset 0 into data region)
///   var[1] = IN value (set by test via write_variable)
///   var[2] = Q output (read by test)
///   var[3] = ET output (read by test)
/// Constant layout:
///   constant[0] = PT value (i64 microseconds)
fn tof_test_container(pt_us: i64) -> ironplc_container::Container {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_VAR_I32,     0x01, 0x00,  // push IN from var[1]
        opcode::FB_STORE_PARAM,   0x00,         // store to TOF.IN (field 0)
        opcode::LOAD_CONST_I64,   0x00, 0x00,  // push PT constant
        opcode::FB_STORE_PARAM,   0x01,         // store to TOF.PT (field 1)
        opcode::FB_CALL,          0x11, 0x00,   // call TOF (type_id 0x0011)
        opcode::FB_LOAD_PARAM,    0x02,         // load TOF.Q (field 2)
        opcode::STORE_VAR_I32,    0x02, 0x00,   // store Q to var[2]
        opcode::FB_LOAD_PARAM,    0x03,         // load TOF.ET (field 3)
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
fn tof_when_in_true_then_q_true_et_zero() {
    let c = tof_test_container(5_000_000); // PT = 5 seconds
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
    let c = tof_test_container(5_000_000);
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
    let c = tof_test_container(5_000_000);
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
    let c = tof_test_container(5_000_000);
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
    let c = tof_test_container(5_000_000); // PT = 5 seconds
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
