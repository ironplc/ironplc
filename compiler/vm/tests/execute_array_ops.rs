mod common;
use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;
use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;

/// Helper: builds a container with one array variable at var[0].
///
/// var[0] holds the data_offset (0) pointing into the data region.
/// The data region is sized for `total_elements` slots of 8 bytes each.
/// The init function sets var[0] = 0 (data_offset for the array).
fn array_container(
    bytecode: &[u8],
    total_elements: u32,
    constants: &[i32],
) -> ironplc_container::Container {
    let data_region_bytes = total_elements * 8;

    // Init function: LOAD_CONST_I32 pool[last] (= 0), STORE_VAR_I32 var[0], RET_VOID
    // We add a 0 constant at the end of the constant pool for the data offset.
    let init_const_index = constants.len() as u16;
    let init_const_bytes = init_const_index.to_le_bytes();
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, init_const_bytes[0], init_const_bytes[1],
        opcode::STORE_VAR_I32,  0x00, 0x00,
        opcode::RET_VOID,
    ];

    let mut builder = ContainerBuilder::new()
        .num_variables(2) // var[0] = array data_offset, var[1] = result
        .data_region_bytes(data_region_bytes);

    for &c in constants {
        builder = builder.add_i32_constant(c);
    }
    // Add the 0 constant for data_offset init
    builder = builder.add_i32_constant(0);

    // Add array descriptor: element_type 0 (I32)
    builder.add_array_descriptor(0, total_elements, 0);

    builder
        .add_function(
            ironplc_container::FunctionId::new(0),
            &init_bytecode,
            1,
            2,
            0,
        )
        .add_function(ironplc_container::FunctionId::new(1), bytecode, 16, 2, 0)
        .init_function_id(ironplc_container::FunctionId::new(0))
        .entry_function_id(ironplc_container::FunctionId::new(1))
        .build()
}

#[test]
fn execute_when_store_array_then_load_array_roundtrips_i32() {
    // Store 42 at index 2, then load from index 2 into var[1].
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // STORE_ARRAY: push value, push index, STORE_ARRAY var[0] desc[0]
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 42 (constant[0])
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 2 (constant[1] = index)
        opcode::STORE_ARRAY,    0x00, 0x00,     // var_index=0, desc_index=0
                                0x00, 0x00,

        // LOAD_ARRAY: push index, LOAD_ARRAY var[0] desc[0]
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 2 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00,     // var_index=0, desc_index=0
                                0x00, 0x00,

        // Store result to var[1]
        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[42, 2]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 42);
}

#[test]
fn execute_when_store_array_at_index_0_then_loads_correctly() {
    // Store 99 at index 0, then load from index 0.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 99
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 0 (index)
        opcode::STORE_ARRAY,    0x00, 0x00, 0x00, 0x00,

        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 0 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,

        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 3, &[99, 0]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 99);
}

#[test]
fn execute_when_load_array_negative_index_then_trap() {
    // Push index -1, LOAD_ARRAY => should trap.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push -1 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[-1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();

    assert_eq!(
        err.trap,
        Trap::ArrayIndexOutOfBounds {
            var_index: ironplc_container::VarIndex::new(0),
            index: -1,
            total_elements: 5,
        }
    );
}

#[test]
fn execute_when_load_array_index_equals_size_then_trap() {
    // Push index 5 for an array of 5 elements => out of bounds.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 5 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[5]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();

    assert_eq!(
        err.trap,
        Trap::ArrayIndexOutOfBounds {
            var_index: ironplc_container::VarIndex::new(0),
            index: 5,
            total_elements: 5,
        }
    );
}

#[test]
fn execute_when_store_array_negative_index_then_trap() {
    // Push value and index -1, STORE_ARRAY => should trap.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 42 (value)
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push -1 (index)
        opcode::STORE_ARRAY,    0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[42, -1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();

    assert_eq!(
        err.trap,
        Trap::ArrayIndexOutOfBounds {
            var_index: ironplc_container::VarIndex::new(0),
            index: -1,
            total_elements: 5,
        }
    );
}

#[test]
fn execute_when_store_array_at_last_valid_index_then_succeeds() {
    // Store at index 4 (last valid) for array of 5 elements.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 77
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 4 (index)
        opcode::STORE_ARRAY,    0x00, 0x00, 0x00, 0x00,

        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push 4 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,

        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[77, 4]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 77);
}

#[test]
fn execute_when_load_array_uninitialized_then_returns_zero() {
    // Load from index 0 without storing => should return 0 (data region is zero-initialized).
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push 0 (index)
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,

        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 3, &[0]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 0);
}
