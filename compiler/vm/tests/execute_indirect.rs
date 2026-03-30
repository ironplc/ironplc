//! Tests for LOAD_INDIRECT and STORE_INDIRECT opcodes (reference dereference).

mod common;

use ironplc_container::opcode;
use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;

#[test]
fn execute_when_load_indirect_valid_ref_then_loads_value() {
    // var[0] = 42 (target), var[1] = ref to var[0], var[2] = result
    // Init: store 42 into var[0], store 0 (ref to var[0]) into var[1]
    // Scan: LOAD_VAR_I64 var[1] (ref), LOAD_INDIRECT, STORE_VAR_I32 var[2]
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00, // push 42
        opcode::STORE_VAR_I32, 0x00, 0x00,  // var[0] = 42
        opcode::LOAD_CONST_I64, 0x01, 0x00, // push 0i64 (pool[1] = ref to var[0])
        opcode::STORE_VAR_I64, 0x01, 0x00,  // var[1] = ref(0)
        opcode::RET_VOID,
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_VAR_I64, 0x01, 0x00,   // push ref from var[1]
        opcode::LOAD_INDIRECT,               // deref → push value at var[0]
        opcode::STORE_VAR_I32, 0x02, 0x00,   // var[2] = dereferenced value
        opcode::RET_VOID,
    ];

    let c = ironplc_container::ContainerBuilder::new()
        .num_variables(3)
        .add_i32_constant(42) // pool[0]
        .add_i64_constant(0) // pool[1] = ref index 0
        .add_function(
            ironplc_container::FunctionId::INIT,
            &init_bytecode,
            16,
            3,
            0,
        )
        .add_function(
            ironplc_container::FunctionId::SCAN,
            &scan_bytecode,
            16,
            3,
            0,
        )
        .init_function_id(ironplc_container::FunctionId::INIT)
        .entry_function_id(ironplc_container::FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[2].as_i32(), 42);
}

#[test]
fn execute_when_store_indirect_valid_ref_then_stores_value() {
    // var[0] = target (initially 0), var[1] = ref to var[0]
    // Scan: push 99, LOAD_VAR_I64 var[1] (ref), STORE_INDIRECT → var[0] should be 99
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I64, 0x01, 0x00, // push 0i64 (pool[1] = ref to var[0])
        opcode::STORE_VAR_I64, 0x01, 0x00,  // var[1] = ref(0)
        opcode::RET_VOID,
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 99 (pool[0])
        opcode::LOAD_VAR_I64, 0x01, 0x00,    // push ref from var[1]
        opcode::STORE_INDIRECT,               // store 99 into var[0] via ref
        opcode::RET_VOID,
    ];

    let c = ironplc_container::ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(99) // pool[0]
        .add_i64_constant(0) // pool[1] = ref index 0
        .add_function(
            ironplc_container::FunctionId::INIT,
            &init_bytecode,
            16,
            2,
            0,
        )
        .add_function(
            ironplc_container::FunctionId::SCAN,
            &scan_bytecode,
            16,
            2,
            0,
        )
        .init_function_id(ironplc_container::FunctionId::INIT)
        .entry_function_id(ironplc_container::FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32(), 99);
}

#[test]
fn execute_when_load_indirect_null_ref_then_null_dereference_trap() {
    // var[0] = null ref (u64::MAX), try LOAD_INDIRECT → trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I64, 0x00, 0x00, // push null ref (u64::MAX)
        opcode::LOAD_INDIRECT,               // should trap
        opcode::RET_VOID,
    ];

    let c = common::single_function_container_i64(&bytecode, 1, &[u64::MAX as i64]);
    let mut b = common::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn execute_when_store_indirect_null_ref_then_null_dereference_trap() {
    // Push value, push null ref, STORE_INDIRECT → trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I64, 0x01, 0x00, // push 42i64
        opcode::LOAD_CONST_I64, 0x00, 0x00, // push null ref (u64::MAX)
        opcode::STORE_INDIRECT,              // should trap
        opcode::RET_VOID,
    ];

    let c = common::single_function_container_i64(&bytecode, 1, &[u64::MAX as i64, 42]);
    let mut b = common::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn execute_when_store_then_load_indirect_then_roundtrips() {
    // Write 77 through ref, then read it back through same ref
    // var[0] = target, var[1] = ref(0), var[2] = result
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I64, 0x00, 0x00, // push 0i64 (ref to var[0])
        opcode::STORE_VAR_I64, 0x01, 0x00,  // var[1] = ref(0)
        opcode::RET_VOID,
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        // Store 77 through ref
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push 77
        opcode::LOAD_VAR_I64, 0x01, 0x00,    // push ref
        opcode::STORE_INDIRECT,               // var[0] = 77
        // Load through ref
        opcode::LOAD_VAR_I64, 0x01, 0x00,    // push ref
        opcode::LOAD_INDIRECT,                // push var[0] value
        opcode::STORE_VAR_I32, 0x02, 0x00,    // var[2] = 77
        opcode::RET_VOID,
    ];

    let c = ironplc_container::ContainerBuilder::new()
        .num_variables(3)
        .add_i64_constant(0) // ref index 0
        .add_i32_constant(77)
        .add_function(
            ironplc_container::FunctionId::INIT,
            &init_bytecode,
            16,
            3,
            0,
        )
        .add_function(
            ironplc_container::FunctionId::SCAN,
            &scan_bytecode,
            16,
            3,
            0,
        )
        .init_function_id(ironplc_container::FunctionId::INIT)
        .entry_function_id(ironplc_container::FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i32(), 77);
    assert_eq!(b.vars[2].as_i32(), 77);
}
