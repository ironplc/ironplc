mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;
use ironplc_vm::error::Trap;

/// Helper: builds a container with a data region for FB testing.
fn fb_container(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[i64],
    data_region_bytes: u32,
) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(num_vars)
        .data_region_bytes(data_region_bytes);
    for &c in constants {
        builder = builder.add_i64_constant(c);
    }
    builder
        .add_function(0, &init_bytecode, 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

#[test]
fn execute_when_fb_store_param_then_writes_data_region() {
    // var[0] = 0 (fb_ref pointing to data region offset 0)
    // Load fb_ref, push value 42, store to field 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_CONST_I64, 0x00, 0x00,    // push constant[0] = 42
        opcode::FB_STORE_PARAM, 0x00,           // store to field 0
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 1, &[42i64], 48); // 6 fields * 8 bytes
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // Check data region: field 0 at offset 0 should contain 42
    let data = vm.data_region();
    let value = i64::from_le_bytes(data[0..8].try_into().unwrap());
    assert_eq!(value, 42);
}

#[test]
fn execute_when_fb_load_param_then_reads_data_region() {
    // Pre-fill data region field 1 with value 99, then read it
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::FB_LOAD_PARAM, 0x01,            // load field 1
        opcode::STORE_VAR_I64, 0x01, 0x00,      // store to var[1]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 2, &[], 48);
    let mut b = VmBuffers::from_container(&c);
    // Pre-fill field 1 (offset 8) with value 99
    b.data_region[8..16].copy_from_slice(&99i64.to_le_bytes());
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable_i64(1).unwrap(), 99);
}

#[test]
fn execute_when_fb_call_unknown_type_then_traps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref
        opcode::FB_CALL, 0xFF, 0xFF,            // call unknown type 0xFFFF
        opcode::POP,
        opcode::RET_VOID,
    ];
    let c = fb_container(&bytecode, 1, &[], 48);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::InvalidFbTypeId(0xFFFF));
}

#[test]
fn execute_when_pop_then_discards_top() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 42
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push 99
        opcode::POP,                          // discard 99
        opcode::STORE_VAR_I32, 0x00, 0x00,   // store 42 to var[0]
        opcode::RET_VOID,
    ];
    let c = common::single_function_container(&bytecode, 1, &[42, 99]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 42);
}
