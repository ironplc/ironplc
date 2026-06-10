//! Integration tests for the `header.max_call_depth` validation
//! performed by `VmReady::start`.
//!
//! The validation lets a container declare its worst-case PLC call
//! depth so the VM can reject a program that would not fit in the
//! embedder's frame buffer *before* any init bytecode runs.

use ironplc_container::{opcode, ContainerBuilder, FunctionId};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

use crate::common::VmBuffers;

fn empty_init_container_with_depth(max_call_depth: u16) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let scan_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    ContainerBuilder::new()
        .num_variables(1)
        .max_call_depth(max_call_depth)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, &scan_bytecode, 0, 1, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

#[test]
fn start_when_container_declares_call_depth_exceeding_buffer_then_returns_program_exceeds_call_depth(
) {
    // `VmBuffers::from_container` allocates `MAX_CALL_DEPTH = 32` frames
    // (the embedder default). Declaring 64 must be rejected up-front.
    let c = empty_init_container_with_depth(64);
    let mut b = VmBuffers::from_container(&c);
    let fault = match Vm::new().load(&c, &mut b).start() {
        Ok(_) => panic!("start should reject over-deep container"),
        Err(f) => f,
    };
    assert_eq!(
        fault.trap,
        Trap::ProgramExceedsCallDepth {
            required: 64,
            capacity: 32,
        }
    );
}

#[test]
fn start_when_container_declares_zero_call_depth_then_no_validation_runs() {
    // Default / hand-built / legacy containers leave `max_call_depth = 0`.
    // The validation must skip in that case (back-compat).
    let c = empty_init_container_with_depth(0);
    let mut b = VmBuffers::from_container(&c);
    let ok = Vm::new().load(&c, &mut b).start().is_ok();
    assert!(
        ok,
        "start should succeed when max_call_depth is 0 (not computed)"
    );
}

#[test]
fn start_when_container_declares_call_depth_within_buffer_then_succeeds() {
    let c = empty_init_container_with_depth(16);
    let mut b = VmBuffers::from_container(&c);
    let ok = Vm::new().load(&c, &mut b).start().is_ok();
    assert!(ok, "start should succeed when max_call_depth fits");
}

#[test]
fn start_when_container_declares_call_depth_equal_to_buffer_then_succeeds() {
    // Equality is the boundary case — the buffer holds exactly the
    // declared depth, so it should be accepted (the rejection check
    // is strict greater-than).
    let c = empty_init_container_with_depth(32);
    let mut b = VmBuffers::from_container(&c);
    let ok = Vm::new().load(&c, &mut b).start().is_ok();
    assert!(ok, "start should succeed at exact-fit boundary");
}
