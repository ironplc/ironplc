//! Integration tests for the `header.max_call_depth` validation
//! performed by `Vm::load`.
//!
//! The validation lets a container declare its worst-case PLC call
//! depth so the VM can reject a program that would not fit in the
//! embedder's frame buffer *before* any bytecode runs, whichever entry
//! path (`VmReady::start` or `VmReady::resume`) follows.

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
fn load_when_container_declares_call_depth_exceeding_buffer_then_returns_program_exceeds_call_depth(
) {
    // Construct the frame buffer from a small container, then load a
    // deeper container into it. This mirrors the embedded scenario the
    // check is for: a fixed-size buffer allocated up front (or shared
    // across loads) that can't grow to fit a freshly loaded program.
    let small = empty_init_container_with_depth(8);
    let mut b = VmBuffers::from_container(&small);
    assert_eq!(b.frames.len(), 8, "buffer sized from the small container");

    let deep = empty_init_container_with_depth(64);
    let trap = match Vm::new().load(&deep, &mut b) {
        Ok(_) => panic!("load should reject over-deep container"),
        Err(t) => t,
    };
    assert_eq!(
        trap,
        Trap::ProgramExceedsCallDepth {
            required: 64,
            capacity: 8,
        }
    );
}

#[test]
fn from_container_when_max_call_depth_set_then_buffer_sized_to_declared_depth() {
    let c = empty_init_container_with_depth(7);
    let b = VmBuffers::from_container(&c);
    assert_eq!(b.frames.len(), 7);
}

#[test]
fn from_container_when_max_call_depth_zero_then_buffer_is_empty() {
    // A declared depth of 0 is invalid (codegen always declares >= 1),
    // so the buffer allocates no frames. Such a container is rejected by
    // `Vm::load` before any code runs.
    let c = empty_init_container_with_depth(0);
    let b = VmBuffers::from_container(&c);
    assert_eq!(b.frames.len(), 0);
}

#[test]
fn load_when_container_declares_zero_call_depth_then_rejected() {
    // Every program needs at least one call frame for its entry function.
    // A declared depth of 0 means the field was never computed (a legacy
    // or hand-built container) and is rejected at load.
    let c = empty_init_container_with_depth(0);
    let mut b = VmBuffers::from_container(&c);
    let trap = match Vm::new().load(&c, &mut b) {
        Ok(_) => panic!("load should reject a zero-call-depth container"),
        Err(t) => t,
    };
    assert_eq!(trap, Trap::ZeroCallDepth);
}

#[test]
fn start_when_container_declares_call_depth_within_buffer_then_succeeds() {
    let c = empty_init_container_with_depth(16);
    let mut b = VmBuffers::from_container(&c);
    let ok = Vm::new().load(&c, &mut b).unwrap().start().is_ok();
    assert!(ok, "start should succeed when max_call_depth fits");
}

#[test]
fn start_when_container_declares_call_depth_equal_to_buffer_then_succeeds() {
    // Equality is the boundary case — the buffer holds exactly the
    // declared depth, so it should be accepted (the rejection check
    // is strict greater-than).
    let c = empty_init_container_with_depth(32);
    let mut b = VmBuffers::from_container(&c);
    let ok = Vm::new().load(&c, &mut b).unwrap().start().is_ok();
    assert!(ok, "start should succeed at exact-fit boundary");
}

#[test]
fn resume_when_container_declares_call_depth_within_buffer_then_continues_scan_count() {
    let c = empty_init_container_with_depth(16);
    let mut b = VmBuffers::from_container(&c);
    let mut running = Vm::new().load(&c, &mut b).unwrap().resume(41);
    assert_eq!(running.scan_count(), 41);
    running.run_round(0).unwrap();
    assert_eq!(running.scan_count(), 42);
}
