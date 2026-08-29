//! COPY_REGION execution: the block copy behind whole-aggregate assignment.
//!
//! Nominal copies of every aggregate shape are owned by the codegen
//! end-to-end tests (`codegen/tests/it/end_to_end_aggregate_copy.rs`), which
//! reach them from ST. This file covers what codegen cannot emit: descriptor
//! disagreements, out-of-range offsets, and the degenerate lengths.

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::ContainerBuilder;
use ironplc_container::VarIndex;
use ironplc_vm::error::Trap;
use ironplc_vm::test_support::load_and_start;
use rstest::rstest;

/// Descriptor indices used by every container this module builds.
const DST_DESC: u16 = 0;
const SRC_DESC: u16 = 1;

/// Variable layout: var[0] destination offset, var[1] source offset,
/// var[2] read-back slot.
const DST_VAR: u16 = 0;
const SRC_VAR: u16 = 1;
const READ_VAR: u16 = 2;

/// Constant pool layout, shared by every container here.
const K_DST_OFFSET: u16 = 0;
const K_SRC_OFFSET: u16 = 1;
const K_INDEX_0: u16 = 2;
const K_SEED: u16 = 3;

fn le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

/// Builds a container with two slot regions and one COPY_REGION between them.
///
/// The scan function seeds `src[0]` with `seed`, runs the copy, then reads
/// `dst[0]` back into var[2] so a test can assert on it through
/// `read_variable`.
///
/// `dst_elements` and `src_elements` size the two descriptors independently so
/// a test can make them disagree.
fn copy_container(
    dst_elements: u32,
    src_elements: u32,
    dst_offset: i32,
    src_offset: i32,
    data_region_bytes: u32,
    seed: i32,
) -> ironplc_container::Container {
    // Init: var[0] = dst_offset, var[1] = src_offset.
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, le(K_DST_OFFSET)[0], le(K_DST_OFFSET)[1],
        opcode::STORE_VAR_I32,  le(DST_VAR)[0],      le(DST_VAR)[1],
        opcode::LOAD_CONST_I32, le(K_SRC_OFFSET)[0], le(K_SRC_OFFSET)[1],
        opcode::STORE_VAR_I32,  le(SRC_VAR)[0],      le(SRC_VAR)[1],
        opcode::RET_VOID,
    ];

    let mut scan: Vec<u8> = Vec::new();
    // Seed src[0] when the source has room for it. STORE_ARRAY pops the index
    // then the value, so the value is pushed first.
    if src_elements > 0 {
        scan.extend_from_slice(&[
            opcode::LOAD_CONST_I32,
            le(K_SEED)[0],
            le(K_SEED)[1],
            opcode::LOAD_CONST_I32,
            le(K_INDEX_0)[0],
            le(K_INDEX_0)[1],
            opcode::STORE_ARRAY,
            le(SRC_VAR)[0],
            le(SRC_VAR)[1],
            le(SRC_DESC)[0],
            le(SRC_DESC)[1],
        ]);
    }
    // The copy: push the source offset, then COPY_REGION into var[0].
    scan.extend_from_slice(&[
        opcode::LOAD_VAR_I32,
        le(SRC_VAR)[0],
        le(SRC_VAR)[1],
        opcode::COPY_REGION,
        le(DST_VAR)[0],
        le(DST_VAR)[1],
        le(DST_DESC)[0],
        le(DST_DESC)[1],
        le(SRC_DESC)[0],
        le(SRC_DESC)[1],
    ]);
    // Read dst[0] back into var[2].
    if dst_elements > 0 {
        scan.extend_from_slice(&[
            opcode::LOAD_CONST_I32,
            le(K_INDEX_0)[0],
            le(K_INDEX_0)[1],
            opcode::LOAD_ARRAY,
            le(DST_VAR)[0],
            le(DST_VAR)[1],
            le(DST_DESC)[0],
            le(DST_DESC)[1],
            opcode::STORE_VAR_I32,
            le(READ_VAR)[0],
            le(READ_VAR)[1],
        ]);
    }
    scan.push(opcode::RET_VOID);

    let mut builder = ContainerBuilder::new()
        .num_variables(3)
        .data_region_bytes(data_region_bytes);
    builder = builder.add_i32_constant(dst_offset);
    builder = builder.add_i32_constant(src_offset);
    builder = builder.add_i32_constant(0);
    builder = builder.add_i32_constant(seed);

    // `add_array_descriptor` dedupes by (element_type, total_elements,
    // element_extra), so equal-sized ends would collapse onto one index. Give
    // the source a different element type -- I64 is also an 8-byte slot, so
    // the derived size is unchanged -- to keep DST_DESC and SRC_DESC distinct.
    let dst = builder.add_array_descriptor(0, dst_elements, 0);
    let src = builder.add_array_descriptor(2, src_elements, 0);
    assert_eq!(dst, DST_DESC);
    assert_eq!(src, SRC_DESC);

    builder
        .add_function(
            ironplc_container::FunctionId::new(0),
            &init_bytecode,
            2,
            3,
            0,
        )
        .add_function(ironplc_container::FunctionId::new(1), &scan, 8, 3, 0)
        .init_function_id(ironplc_container::FunctionId::new(0))
        .entry_function_id(ironplc_container::FunctionId::new(1))
        .max_call_depth(1)
        .build()
}

/// Runs one round, returning var[2] (the destination's first slot) or the trap.
fn run(container: &ironplc_container::Container) -> Result<i32, Trap> {
    let mut bufs = VmBuffers::from_container(container);
    let mut vm = load_and_start(container, &mut bufs).unwrap();
    match vm.run_round(0) {
        Ok(_) => Ok(vm.read_variable(VarIndex::new(READ_VAR)).unwrap()),
        Err(fault) => Err(fault.trap),
    }
}

#[test]
fn execute_when_copy_region_then_bytes_move_to_destination() {
    // Two 2-slot regions: destination at 0, source at 16.
    let container = copy_container(2, 2, 0, 16, 32, 7);
    assert_eq!(run(&container).unwrap(), 7);
}

#[test]
fn execute_when_copy_region_offsets_identical_then_contents_preserved() {
    // `x := x`. copy_within over identical ranges must not corrupt.
    let container = copy_container(2, 2, 0, 0, 16, 11);
    assert_eq!(run(&container).unwrap(), 11);
}

#[test]
fn execute_when_copy_region_regions_overlap_then_copy_is_well_defined() {
    // Destination at 0, source at 8: the two 16-byte spans overlap by a slot.
    // copy_within is defined here, so this must not trap or corrupt.
    let container = copy_container(2, 2, 0, 8, 32, 5);
    assert_eq!(run(&container).unwrap(), 5);
}

#[test]
fn execute_when_copy_region_zero_length_then_succeeds_without_trapping() {
    let container = copy_container(0, 0, 0, 8, 16, 0);
    // No read-back is emitted for an empty destination, so var[2] stays 0.
    assert_eq!(run(&container).unwrap(), 0);
}

#[rstest]
#[case::destination_larger(4, 2)]
#[case::source_larger(2, 4)]
fn execute_when_copy_region_descriptors_disagree_then_traps(
    #[case] dst_elements: u32,
    #[case] src_elements: u32,
) {
    // Plenty of data region, so only the size disagreement can trap.
    let container = copy_container(dst_elements, src_elements, 0, 64, 256, 1);
    assert_eq!(
        run(&container).unwrap_err(),
        Trap::RegionSizeMismatch {
            dst_bytes: dst_elements * 8,
            src_bytes: src_elements * 8,
        }
    );
}

#[test]
fn execute_when_copy_region_destination_past_data_region_then_traps() {
    // 16 bytes copied to offset 24 of a 32-byte region overruns by 8.
    let container = copy_container(2, 2, 24, 0, 32, 1);
    assert_eq!(
        run(&container).unwrap_err(),
        Trap::DataRegionOutOfBounds(24)
    );
}

#[test]
fn execute_when_copy_region_source_past_data_region_then_traps() {
    // The destination fits; the source runs off the end.
    let container = copy_container(2, 2, 0, 24, 32, 1);
    assert_eq!(
        run(&container).unwrap_err(),
        Trap::DataRegionOutOfBounds(24)
    );
}

#[test]
fn execute_when_copy_region_source_offset_negative_then_traps() {
    // A negative offset reinterpreted as u32 lands far past the region rather
    // than wrapping into a valid-looking range.
    let container = copy_container(2, 2, 0, -8, 32, 1);
    assert!(matches!(
        run(&container).unwrap_err(),
        Trap::DataRegionOutOfBounds(_)
    ));
}

#[test]
fn execute_when_copy_region_descriptor_index_unknown_then_traps() {
    // src_desc = 1 with only one descriptor registered.
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::STORE_VAR_I32,  0x00, 0x00,
        opcode::RET_VOID,
    ];
    #[rustfmt::skip]
    let scan: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::COPY_REGION, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(3)
        .data_region_bytes(32);
    builder = builder.add_i32_constant(0);
    builder.add_array_descriptor(0, 2, 0);
    let container = builder
        .add_function(
            ironplc_container::FunctionId::new(0),
            &init_bytecode,
            1,
            3,
            0,
        )
        .add_function(ironplc_container::FunctionId::new(1), &scan, 8, 3, 0)
        .init_function_id(ironplc_container::FunctionId::new(0))
        .entry_function_id(ironplc_container::FunctionId::new(1))
        .max_call_depth(1)
        .build();

    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    assert_eq!(
        vm.run_round(0).unwrap_err().trap,
        Trap::InvalidVariableIndex(VarIndex::new(DST_VAR))
    );
}
