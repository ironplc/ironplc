//! Integration tests for instruction profiling.
//!
//! These tests only compile when the `profiling` feature is enabled.
#![cfg(feature = "profiling")]

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_container::opcode;
use ironplc_vm::{InstructionProfile, Vm};

#[test]
fn profile_new_when_created_then_all_counts_zero() {
    let profile = InstructionProfile::new();
    assert_eq!(profile.total(), 0);
    assert_eq!(profile.count(opcode::ADD_I32), 0);
}

#[test]
fn profile_record_when_single_opcode_then_count_incremented() {
    let mut profile = InstructionProfile::new();
    profile.record(opcode::ADD_I32);
    profile.record(opcode::ADD_I32);
    profile.record(opcode::SUB_I32);
    assert_eq!(profile.count(opcode::ADD_I32), 2);
    assert_eq!(profile.count(opcode::SUB_I32), 1);
    assert_eq!(profile.total(), 3);
}

#[test]
fn profile_reset_when_called_then_all_counts_zero() {
    let mut profile = InstructionProfile::new();
    profile.record(opcode::ADD_I32);
    profile.reset();
    assert_eq!(profile.count(opcode::ADD_I32), 0);
    assert_eq!(profile.total(), 0);
}

#[test]
fn vm_stop_when_profiling_enabled_then_profile_has_counts() {
    // Bytecode: LOAD_CONST_I32 [0], LOAD_CONST_I32 [1], ADD_I32, STORE_VAR_I32 [0], RET_VOID
    #[rustfmt::skip]
    let bytecode: &[u8] = &[
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::LOAD_CONST_I32, 0x01, 0x00,
        opcode::ADD_I32,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let container = single_function_container(bytecode, 1, &[10, 20]);
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = Vm::new().load(&container, &mut bufs).start().unwrap();
    vm.run_round(0).unwrap();
    let stopped = vm.stop();

    let profile = stopped.profile();
    // The init function executes RET_VOID once, and the scan function executes
    // the bytecode above, so we expect counts from both.
    assert!(profile.count(opcode::LOAD_CONST_I32) >= 2);
    assert!(profile.count(opcode::ADD_I32) >= 1);
    assert!(profile.count(opcode::STORE_VAR_I32) >= 1);
    assert!(profile.count(opcode::RET_VOID) >= 2); // init + scan
    assert!(profile.total() > 0);
}
