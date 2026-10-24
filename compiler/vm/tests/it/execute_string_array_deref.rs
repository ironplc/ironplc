//! VM tests for `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF`: a
//! string element read or written through the reference a variable holds,
//! a data-region reference or a variable reference.

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::reference::data_region_reference;
use ironplc_container::{Container, ContainerBuilder, FieldType, FunctionId, VarIndex};
use ironplc_vm::error::Trap;
use rstest::rstest;

/// The capacity of each element of the array the tests reach.
const MAX_LEN: u16 = 5;
/// The byte distance between the elements: a header, then the characters.
const STRIDE: usize = 6 + MAX_LEN as usize;

/// A container with a two-element `STRING[5]` array at data offset 0, whose
/// base offset variable 0 holds, and the reference `reference` in variable
/// 1, which the scan function `bytecode` follows. The constant pool is
/// `i32_constants`, then `str_constants`, then the base offset and the
/// reference.
fn deref_container(
    bytecode: &[u8],
    reference: u64,
    i32_constants: &[i32],
    str_constants: &[&[u8]],
) -> Container {
    let base_index = (i32_constants.len() + str_constants.len()) as u16;
    let base = base_index.to_le_bytes();
    let reference_index = (base_index + 1).to_le_bytes();
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, base[0], base[1],
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::LOAD_CONST_I64, reference_index[0], reference_index[1],
        opcode::STORE_VAR_I64, 0x01, 0x00,
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(2)
        .data_region_bytes(64)
        .num_temp_bufs(4)
        .max_temp_buf_bytes(64);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for s in str_constants {
        builder = builder.add_str_constant(s);
    }
    builder = builder
        .add_i32_constant(0)
        .add_i64_constant(reference as i64);
    builder.add_array_descriptor(FieldType::String as u8, 2, MAX_LEN);
    builder
        .add_function(FunctionId::new(0), &init_bytecode, 1, 2, 0)
        .add_function(FunctionId::new(1), bytecode, 16, 2, 0)
        .init_function_id(FunctionId::new(0))
        .entry_function_id(FunctionId::new(1))
        .max_call_depth(1)
        .build()
}

/// Writes a narrow string element of capacity `MAX_LEN` holding `text` at
/// `offset` of the data region.
fn write_element(data_region: &mut [u8], offset: usize, text: &[u8]) {
    data_region[offset..offset + 2].copy_from_slice(&MAX_LEN.to_le_bytes());
    data_region[offset + 2..offset + 4].copy_from_slice(&(text.len() as u16).to_le_bytes());
    data_region[offset + 4..offset + 6].copy_from_slice(&1u16.to_le_bytes());
    data_region[offset + 6..offset + 6 + text.len()].copy_from_slice(text);
}

/// A data-region reference to the array's first element, and a variable
/// reference to the variable that holds the array's base offset, both
/// reach element 1.
#[rstest]
#[case::data_region_reference(data_region_reference(0))]
#[case::variable_reference(0)]
fn execute_when_str_load_array_deref_then_loads_referenced_element(#[case] reference: u64) {
    // Load element 1 through the reference, and store it into element 0.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,                       // index 1
        opcode::STR_LOAD_ARRAY_DEREF, 0x01, 0x00, 0x00, 0x00,
        opcode::LOAD_CONST_I32, 0x01, 0x00,                       // index 0
        opcode::STR_STORE_ARRAY_ELEM, 0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = deref_container(&bytecode, reference, &[1, 0], &[]);
    let mut b = VmBuffers::from_container(&c);
    write_element(&mut b.data_region, 0, b"");
    write_element(&mut b.data_region, STRIDE, b"ABC");
    {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }

    assert_eq!(&b.data_region[2..4], &[3, 0]);
    assert_eq!(&b.data_region[6..9], b"ABC");
}

/// A store through a reference writes the referenced element, truncated to
/// the descriptor's capacity, and leaves the other element as it was.
#[test]
fn execute_when_str_store_array_deref_then_writes_referenced_element_truncated() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_STR, 0x01, 0x00,                       // "abcdefgh"
        opcode::LOAD_CONST_I32, 0x00, 0x00,                       // index 1
        opcode::STR_STORE_ARRAY_DEREF, 0x01, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = deref_container(&bytecode, data_region_reference(0), &[1], &[b"abcdefgh"]);
    let mut b = VmBuffers::from_container(&c);
    write_element(&mut b.data_region, 0, b"xy");
    write_element(&mut b.data_region, STRIDE, b"");
    {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }

    assert_eq!(&b.data_region[STRIDE + 2..STRIDE + 4], &[5, 0]);
    assert_eq!(&b.data_region[STRIDE + 6..STRIDE + 11], b"abcde");
    assert_eq!(&b.data_region[2..4], &[2, 0]);
    assert_eq!(&b.data_region[6..8], b"xy");
}

#[test]
fn execute_when_str_load_array_deref_of_null_then_null_dereference() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,                       // index 0
        opcode::STR_LOAD_ARRAY_DEREF, 0x01, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = deref_container(&bytecode, u64::MAX, &[0], &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    assert_eq!(vm.run_round(0).unwrap_err().trap, Trap::NullDereference);
}

/// An index outside the array traps, naming the reference variable, as
/// `LOAD_ARRAY_DEREF` does.
#[test]
fn execute_when_str_store_array_deref_index_past_end_then_out_of_bounds() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_STR, 0x01, 0x00,                       // "ab"
        opcode::LOAD_CONST_I32, 0x00, 0x00,                       // index 2
        opcode::STR_STORE_ARRAY_DEREF, 0x01, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = deref_container(&bytecode, data_region_reference(0), &[2], &[b"ab"]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    assert_eq!(
        vm.run_round(0).unwrap_err().trap,
        Trap::ArrayIndexOutOfBounds {
            var_index: VarIndex::new(1),
            index: 2,
            total_elements: 2,
        }
    );
}
