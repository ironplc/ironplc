//! Integration tests for data region boundary violation traps.

mod common;

use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId};
use ironplc_vm::error::Trap;

/// Helper: builds a container with a specific data_region_bytes size.
fn data_region_container(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    i64_constants: &[i64],
    data_region_bytes: u32,
) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(num_vars)
        .data_region_bytes(data_region_bytes);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in i64_constants {
        builder = builder.add_i64_constant(c);
    }
    builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

#[test]
fn execute_when_fb_load_param_oob_then_traps() {
    // var[0] holds fb_ref = offset 0 into data region.
    // Data region is only 8 bytes (1 field). Accessing field 1 (offset 8) is OOB.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::FB_LOAD_PARAM, 0x01,            // load field 1 (offset 8) — OOB
        opcode::STORE_VAR_I32, 0x01, 0x00,
        opcode::POP,
        opcode::RET_VOID,
    ];
    let c = data_region_container(&bytecode, 2, &[], &[], 8); // only 8 bytes
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::DataRegionOutOfBounds(8));
}

#[test]
fn execute_when_fb_store_param_oob_then_traps() {
    // Data region is 8 bytes. Storing to field 1 (offset 8) is OOB.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_CONST_I64, 0x00, 0x00,    // push value 42
        opcode::FB_STORE_PARAM, 0x01,           // store to field 1 (offset 8) — OOB
        opcode::POP,
        opcode::RET_VOID,
    ];
    let c = data_region_container(&bytecode, 1, &[], &[42i64], 8);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::DataRegionOutOfBounds(8));
}

#[test]
fn execute_when_fb_instance_at_high_offset_oob_then_traps() {
    // Set var[0] to a large offset that exceeds the data region.
    // We pre-write var[0] to hold offset 1000.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,      // push 1000
        opcode::STORE_VAR_I32, 0x00, 0x00,        // store to var[0] (fb_ref = 1000)
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,     // push fb_ref from var[0]
        opcode::FB_LOAD_PARAM, 0x00,              // try to load field 0 at offset 1000 — OOB
        opcode::POP,
        opcode::POP,
        opcode::RET_VOID,
    ];
    let c = data_region_container(&bytecode, 1, &[1000], &[], 16);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::DataRegionOutOfBounds(1000));
}

#[test]
fn execute_when_data_region_access_within_bounds_then_succeeds() {
    // Verify that accessing field 0 of an 8-byte data region works.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0] (offset 0)
        opcode::LOAD_CONST_I64, 0x00, 0x00,    // push 77
        opcode::FB_STORE_PARAM, 0x00,           // store to field 0 (offset 0) — in bounds
        opcode::FB_LOAD_PARAM, 0x00,            // load field 0 back
        opcode::STORE_VAR_I32, 0x01, 0x00,      // store to var[1]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];
    let c = data_region_container(&bytecode, 2, &[], &[77i64], 16);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(
        vm.read_variable(ironplc_container::VarIndex::new(1))
            .unwrap(),
        77
    );
}
