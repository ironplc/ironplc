use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;
use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;
use rstest::rstest;

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
        .max_call_depth(1)
        .build()
}

/// Store `value` at `index`, then load it back into var[1] and confirm the
/// value round-trips. Each case shares the same bytecode shape and differs only
/// by array size, stored value, and index (constant[0] = value, constant[1] =
/// index); each row still runs as an individually-named test.
#[rstest]
#[case::roundtrips_index_2(5, 42, 2)]
#[case::index_0(3, 99, 0)]
#[case::last_valid_index(5, 77, 4)]
fn execute_when_store_array_then_load_array_roundtrips_i32(
    #[case] total_elements: u32,
    #[case] value: i32,
    #[case] index: i32,
) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // STORE_ARRAY: push value, push index, STORE_ARRAY var[0] desc[0]
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push value (constant[0])
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push index (constant[1])
        opcode::STORE_ARRAY,    0x00, 0x00, 0x00, 0x00,

        // LOAD_ARRAY: push index, LOAD_ARRAY var[0] desc[0]
        opcode::LOAD_CONST_I32, 0x01, 0x00,    // push index
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,

        // Store result to var[1]
        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, total_elements, &[value, index]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), value);
}

/// Loading at an out-of-bounds index traps. Both cases share the load-only
/// bytecode shape (constant[0] = index) against a 5-element array and differ
/// only by the offending index.
#[rstest]
#[case::negative_index(-1)]
#[case::index_equals_size(5)]
fn execute_when_load_array_out_of_bounds_then_trap(#[case] index: i32) {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,    // push index
        opcode::LOAD_ARRAY,     0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = array_container(&bytecode, 5, &[index]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err();

    assert_eq!(
        err.trap,
        Trap::ArrayIndexOutOfBounds {
            var_index: ironplc_container::VarIndex::new(0),
            index,
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 0);
}
