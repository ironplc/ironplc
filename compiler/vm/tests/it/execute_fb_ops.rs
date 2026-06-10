use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FbTypeId, FunctionId, UserFbDescriptor, VarIndex};
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
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable_i64(VarIndex::new(1)).unwrap(), 99);
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(&mut vm, Trap::InvalidFbTypeId(FbTypeId::new(0xFFFF)));
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
    let c = crate::common::single_function_container(&bytecode, 1, &[42, 99]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);
}

#[test]
fn execute_when_user_fb_call_then_executes_body_and_persists_state() {
    // User-defined FB body (function 2): reads field 0 (x), doubles it,
    // stores to field 1 (y).
    // The FB has 2 fields mapped to var_offset 2..3.
    // Field 0 (x) -> var[2], Field 1 (y) -> var[3]
    #[rustfmt::skip]
    let fb_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load var[2] (x)
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load var[2] (x) again
        opcode::ADD_I32,                     // x + x = 2*x
        opcode::STORE_VAR_I32, 0x03, 0x00,  // store to var[3] (y)
        opcode::RET_VOID,
    ];

    // Main scan bytecode:
    // var[0] = data region offset (fb_ref), pre-initialized to 0
    // var[1] = result variable
    // 1. Store input value 7 to FB field 0 (x)
    // 2. FB_CALL with user type_id 0x1000
    // 3. Read output field 1 (y) into var[1]
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,  // push fb_ref from var[0]
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push constant 7
        opcode::FB_STORE_PARAM, 0x00,           // store to field 0 (x)
        opcode::FB_CALL, 0x00, 0x10,            // call type_id 0x1000
        opcode::FB_LOAD_PARAM, 0x01,            // load field 1 (y)
        opcode::STORE_VAR_I32, 0x01, 0x00,      // store to var[1]
        opcode::POP,                            // discard fb_ref
        opcode::RET_VOID,
    ];

    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];

    // 4 variables: var[0]=fb_ref, var[1]=result, var[2]=fb.x, var[3]=fb.y
    let c = ContainerBuilder::new()
        .num_variables(4)
        .data_region_bytes(16) // 2 fields * 8 bytes
        .add_i32_constant(7)
        .add_function(
            ironplc_container::FunctionId::new(0),
            &init_bytecode,
            0,
            4,
            0,
        )
        .add_function(
            ironplc_container::FunctionId::new(1),
            &scan_bytecode,
            16,
            4,
            0,
        )
        .add_function(ironplc_container::FunctionId::new(2), &fb_body, 4, 2, 0)
        .add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1000),
            function_id: FunctionId::new(2),
            var_offset: 2,
            num_fields: 2,
        })
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // y = x * 2 = 7 * 2 = 14
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 14);
}

#[test]
fn execute_when_user_fb_call_then_internal_state_persists_across_rounds() {
    // User-defined FB ACCUMULATOR body (function 2):
    // 3 fields: field 0 (val/input), field 1 (total/internal), field 2 (sum/output)
    // Mapped to var[2], var[3], var[4]
    // Body: total := total + val; sum := total;
    #[rustfmt::skip]
    let fb_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x03, 0x00,   // load total (var[3])
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load val (var[2])
        opcode::ADD_I32,                     // total + val
        opcode::STORE_VAR_I32, 0x03, 0x00,  // store to total (var[3])
        opcode::LOAD_VAR_I32, 0x03, 0x00,   // load total
        opcode::STORE_VAR_I32, 0x04, 0x00,  // store to sum (var[4])
        opcode::RET_VOID,
    ];

    // Main scan: store 10 to field 0, call FB, read field 2 into var[1]
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 10
        opcode::FB_STORE_PARAM, 0x00,           // store to field 0 (val)
        opcode::FB_CALL, 0x00, 0x10,            // call type_id 0x1000
        opcode::FB_LOAD_PARAM, 0x02,            // load field 2 (sum)
        opcode::STORE_VAR_I32, 0x01, 0x00,
        opcode::POP,
        opcode::RET_VOID,
    ];

    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];

    // 5 variables: var[0]=fb_ref, var[1]=result, var[2..4]=fb fields
    let c = ContainerBuilder::new()
        .num_variables(5)
        .data_region_bytes(24) // 3 fields * 8 bytes
        .add_i32_constant(10)
        .add_function(
            ironplc_container::FunctionId::new(0),
            &init_bytecode,
            0,
            5,
            0,
        )
        .add_function(
            ironplc_container::FunctionId::new(1),
            &scan_bytecode,
            16,
            5,
            0,
        )
        .add_function(ironplc_container::FunctionId::new(2), &fb_body, 4, 3, 0)
        .add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1000),
            function_id: FunctionId::new(2),
            var_offset: 2,
            num_fields: 3,
        })
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    // Round 1: total = 0 + 10 = 10
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 10);

    // Round 2: total = 10 + 10 = 20 (state persists in data region)
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 20);
}

#[test]
fn execute_when_nested_user_fb_calls_then_both_copy_outs_run() {
    // Outer FB (type 0x1000) calls Inner FB (type 0x1001). Both write to
    // their own data-region fields. Verify both copy-outs run on the
    // unwind so each FB's field reflects the body's write — i.e. the
    // iterative dispatch loop pops both fb_return frames in order.
    //
    // Layout:
    //   shared globals: var[0]=outer_ref(=0), var[1]=inner_ref(=8)
    //   scan locals:    var[2]=outer field read-back, var[3]=inner field read-back
    //   outer FB body locals: var[4] (mapped to outer field 0)
    //   inner FB body locals: var[5] (mapped to inner field 0)
    //   data region: 16 bytes, outer instance at 0..8, inner at 8..16
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x02, 0x00, // push 8 (constants[2])
        opcode::STORE_VAR_I32, 0x01, 0x00,  // var[1] := 8 (inner_ref)
        opcode::RET_VOID,
    ];

    #[rustfmt::skip]
    let inner_body: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x01, 0x00, // push 88 (constants[1])
        opcode::STORE_VAR_I32, 0x05, 0x00,  // var[5] := 88
        opcode::RET_VOID,
    ];

    #[rustfmt::skip]
    let outer_body: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,   // push 77 (constants[0])
        opcode::STORE_VAR_I32, 0x04, 0x00,    // var[4] := 77
        opcode::FB_LOAD_INSTANCE, 0x01, 0x00, // push inner_ref (var[1] is in shared globals)
        opcode::FB_CALL, 0x01, 0x10,          // call inner FB (type 0x1001)
        opcode::POP,                          // discard inner_ref
        opcode::RET_VOID,
    ];

    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00, // push outer_ref (var[0])
        opcode::FB_CALL, 0x00, 0x10,          // call outer FB (type 0x1000)
        opcode::FB_LOAD_PARAM, 0x00,          // peek outer_ref, load its field 0
        opcode::STORE_VAR_I32, 0x02, 0x00,    // var[2] := outer field
        opcode::POP,                          // discard outer_ref
        opcode::FB_LOAD_INSTANCE, 0x01, 0x00, // push inner_ref (var[1])
        opcode::FB_LOAD_PARAM, 0x00,          // peek inner_ref, load its field 0
        opcode::STORE_VAR_I32, 0x03, 0x00,    // var[3] := inner field
        opcode::POP,                          // discard inner_ref
        opcode::RET_VOID,
    ];

    let c = ContainerBuilder::new()
        .num_variables(6)
        .shared_globals_size(2)
        .data_region_bytes(16)
        .add_i32_constant(77)
        .add_i32_constant(88)
        .add_i32_constant(8)
        .add_function(FunctionId::INIT, &init_bytecode, 4, 6, 0)
        .add_function(FunctionId::SCAN, &scan_bytecode, 16, 6, 0)
        .add_function(FunctionId::new(2), &outer_body, 16, 1, 0)
        .add_function(FunctionId::new(3), &inner_body, 4, 1, 0)
        .add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1000),
            function_id: FunctionId::new(2),
            var_offset: 4,
            num_fields: 1,
        })
        .add_user_fb_type(UserFbDescriptor {
            type_id: FbTypeId::new(0x1001),
            function_id: FunctionId::new(3),
            var_offset: 5,
            num_fields: 1,
        })
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // Outer's RET_VOID popped its frame and ran the copy-out, so its
    // field in the data region holds 77; FB_LOAD_PARAM 0 then loads it.
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 77);
    // Inner's RET_VOID likewise wrote 88 back to its field, which the
    // scan reads after the outer returns.
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 88);
}
