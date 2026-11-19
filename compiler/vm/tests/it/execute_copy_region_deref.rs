//! VM tests for `COPY_REGION_DEREF`: a whole array or structure copied into
//! the one a reference refers to, a data-region reference or a variable
//! reference.

use crate::common::{freewheeling_task, load_and_start, program_instance, VmBuffers};
use ironplc_container::opcode;
use ironplc_container::reference::data_region_reference;
use ironplc_container::{Container, ContainerBuilder, FunctionId, ProgramInstanceEntry, VarIndex};
use ironplc_vm::error::Trap;
use rstest::rstest;

/// The destination's offset in the data region, and the source's: two
/// regions of up to four 8-byte slots.
const DST_OFFSET: i32 = 0;
const SRC_OFFSET: i32 = 32;

/// Variable layout. The program instance under test owns `[0, 4)`: var[0]
/// holds the destination's offset, var[1] the source's, var[2] the
/// reference to the destination, var[3] the destination's first slot read
/// back. An idle second instance owns `[4, 5)`, so a reference to var[4]
/// names another instance's variable.
const DST_VAR: u16 = 0;
const SRC_VAR: u16 = 1;
const REF_VAR: u16 = 2;
const READ_VAR: u16 = 3;
const OTHER_INSTANCE_VAR: u16 = 4;
const NUM_VARIABLES: u16 = 5;

/// Function ids: the instance under test's init and scan, and the idle
/// instance's `RET_VOID`, which serves as both its init and its scan.
const INIT: u16 = 0;
const SCAN: u16 = 1;
const IDLE: u16 = 2;

/// The value stored in the source's first slot before the copy.
const SEED: i32 = 42;

fn le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

/// A container whose scan seeds the source's first slot, copies the source,
/// of `src_elements` slots, into the region of `dst_elements` slots that the
/// reference `reference` in var[2] refers to, then reads the destination's
/// first slot into var[3]. It runs beside the idle second instance.
fn deref_container(dst_elements: u32, src_elements: u32, reference: u64) -> Container {
    // Constant pool: the two offsets, the reference, index 0 and the seed.
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::STORE_VAR_I32,  le(DST_VAR)[0], le(DST_VAR)[1],
        opcode::LOAD_CONST_I32, 0x01, 0x00,
        opcode::STORE_VAR_I32,  le(SRC_VAR)[0], le(SRC_VAR)[1],
        opcode::LOAD_CONST_I64, 0x02, 0x00,
        opcode::STORE_VAR_I64,  le(REF_VAR)[0], le(REF_VAR)[1],
        opcode::RET_VOID,
    ];
    // STORE_ARRAY pops the index then the value, so the value goes first;
    // descriptor 0 is the destination's and descriptor 1 the source's.
    #[rustfmt::skip]
    let scan: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x04, 0x00,                   // SEED
        opcode::LOAD_CONST_I32, 0x03, 0x00,                   // index 0
        opcode::STORE_ARRAY, le(SRC_VAR)[0], le(SRC_VAR)[1], 0x01, 0x00,
        opcode::LOAD_VAR_I32, le(SRC_VAR)[0], le(SRC_VAR)[1],
        opcode::COPY_REGION_DEREF, le(REF_VAR)[0], le(REF_VAR)[1], 0x00, 0x00, 0x01, 0x00,
        opcode::LOAD_CONST_I32, 0x03, 0x00,                   // index 0
        opcode::LOAD_ARRAY, le(DST_VAR)[0], le(DST_VAR)[1], 0x00, 0x00,
        opcode::STORE_VAR_I32, le(READ_VAR)[0], le(READ_VAR)[1],
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(NUM_VARIABLES)
        .data_region_bytes(64)
        .add_i32_constant(DST_OFFSET)
        .add_i32_constant(SRC_OFFSET)
        .add_i64_constant(reference as i64)
        .add_i32_constant(0)
        .add_i32_constant(SEED);
    // Distinct element types keep the two descriptors apart when their sizes
    // agree: both are 8-byte slots.
    assert_eq!(builder.add_array_descriptor(0, dst_elements, 0), 0);
    assert_eq!(builder.add_array_descriptor(2, src_elements, 0), 1);
    builder
        .add_function(FunctionId::new(INIT), &init_bytecode, 2, 4, 0)
        .add_function(FunctionId::new(SCAN), &scan, 8, 4, 0)
        .add_function(FunctionId::new(IDLE), &[opcode::RET_VOID], 0, 1, 0)
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(ProgramInstanceEntry {
            init_function_id: FunctionId::new(INIT),
            ..program_instance(0, 0, SCAN, 0, 4)
        })
        .add_program_instance(ProgramInstanceEntry {
            init_function_id: FunctionId::new(IDLE),
            ..program_instance(1, 0, IDLE, OTHER_INSTANCE_VAR, 1)
        })
        .max_call_depth(1)
        .build()
}

/// Runs one round, returning the destination's first slot or the trap.
fn run(container: &Container) -> Result<i32, Trap> {
    let mut bufs = VmBuffers::from_container(container);
    let mut vm = load_and_start(container, &mut bufs).unwrap();
    match vm.run_round(0) {
        Ok(_) => Ok(vm.read_variable(VarIndex::new(READ_VAR)).unwrap()),
        Err(fault) => Err(fault.trap),
    }
}

/// A data-region reference to the destination's first byte, and a variable
/// reference to the variable that holds its offset, both reach the
/// destination.
#[rstest]
#[case::data_region_reference(data_region_reference(DST_OFFSET as u32))]
#[case::variable_reference(u64::from(DST_VAR))]
fn execute_when_copy_region_deref_then_bytes_move_to_referenced_region(#[case] reference: u64) {
    assert_eq!(run(&deref_container(4, 4, reference)), Ok(SEED));
}

#[test]
fn execute_when_copy_region_deref_of_null_then_null_dereference() {
    assert_eq!(
        run(&deref_container(4, 4, u64::MAX)),
        Err(Trap::NullDereference)
    );
}

/// A slot holding a value of neither form, bits a type-punned program
/// wrote, traps rather than naming the variable at `value & 0xFFFF`. Each
/// of these values truncates to var[0], the destination's own variable, so
/// a truncating read would have reached the destination as if the
/// reference were sound. A well-formed data-region reference is a
/// destination this instruction accepts; one with a bit above the tag set
/// as well is not.
#[rstest]
#[case::data_region_tag_with_a_higher_bit(data_region_reference(0) | 1 << 33)]
#[case::bit_above_the_tag(1 << 33)]
#[case::just_past_the_variable_indices(u64::from(u16::MAX) + 1)]
fn execute_when_copy_region_deref_of_forged_reference_then_invalid_variable_index(
    #[case] reference: u64,
) {
    assert_eq!(
        run(&deref_container(4, 4, reference)),
        Err(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))
    );
}

/// A variable reference into another program instance's partition is
/// outside the instance's scope, as it is for `LOAD_INDIRECT`; the other
/// instance's slot is not read as the destination's offset.
#[test]
fn execute_when_copy_region_deref_into_other_instance_then_invalid_variable_index() {
    assert_eq!(
        run(&deref_container(4, 4, u64::from(OTHER_INSTANCE_VAR))),
        Err(Trap::InvalidVariableIndex(VarIndex::new(
            OTHER_INSTANCE_VAR
        )))
    );
}

/// The two ends are sized from their descriptors, as `COPY_REGION`'s are.
#[test]
fn execute_when_copy_region_deref_descriptors_disagree_then_region_size_mismatch() {
    assert_eq!(
        run(&deref_container(3, 4, u64::from(DST_VAR))),
        Err(Trap::RegionSizeMismatch {
            dst_bytes: 24,
            src_bytes: 32,
        })
    );
}

/// A data-region reference whose region runs past the data region traps
/// rather than writing outside it.
#[test]
fn execute_when_copy_region_deref_past_data_region_then_out_of_bounds() {
    assert_eq!(
        run(&deref_container(4, 4, data_region_reference(48))),
        Err(Trap::DataRegionOutOfBounds(48))
    );
}
