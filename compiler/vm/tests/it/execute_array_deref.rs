//! VM tests for `LOAD_ARRAY_DEREF` and `STORE_ARRAY_DEREF`: an array element
//! read or written through the variable reference a variable holds, after
//! the checks a reference passes on every path that follows one, which are
//! `LOAD_INDIRECT`'s: null, a value that is not a variable index, and the
//! program instance's scope.

use crate::common::{freewheeling_task, load_and_start, program_instance, VmBuffers};
use ironplc_container::opcode;
use ironplc_container::reference::data_region_reference;
use ironplc_container::{Container, ContainerBuilder, FunctionId, ProgramInstanceEntry, VarIndex};
use ironplc_vm::error::Trap;
use rstest::rstest;

/// Variable layout. The program instance under test owns `[0, 4)`: var[0]
/// holds the array's data offset, var[1] the reference the scan follows,
/// var[2] the element read back, and var[3] is the frame of the function
/// the call test's scan calls. An idle second instance owns `[4, 5)`, so a
/// reference to var[4] names another instance's variable.
const ARRAY_VAR: u16 = 0;
const REF_VAR: u16 = 1;
const READ_VAR: u16 = 2;
const CALLEE_FRAME: u16 = 3;
const OTHER_INSTANCE_VAR: u16 = 4;
const NUM_VARIABLES: u16 = 5;

/// The array: four 8-byte slots at data offset 0, descriptor 0.
const ELEMENTS: u32 = 4;
/// The element the scans read and write.
const INDEX: i32 = 1;
/// The value the scans store in it.
const SEED: i32 = 42;

/// The constant pool: the array's offset, the reference, `INDEX` and
/// `SEED`, as little-endian pool indices.
const OFFSET_CONST: [u8; 2] = [0x00, 0x00];
const REF_CONST: [u8; 2] = [0x01, 0x00];
const INDEX_CONST: [u8; 2] = [0x02, 0x00];
const SEED_CONST: [u8; 2] = [0x03, 0x00];

/// Function ids: the instance under test's init and scan, the idle
/// instance's `RET_VOID`, and the function the call test's scan calls.
const INIT: u16 = 0;
const SCAN: u16 = 1;
const IDLE: u16 = 2;
const CALLEE: u16 = 3;

fn le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

/// A scan that seeds element `INDEX` directly, then reads it back through
/// the reference with `LOAD_ARRAY_DEREF` into var[2].
fn load_scan() -> Vec<u8> {
    #[rustfmt::skip]
    let scan = vec![
        opcode::LOAD_CONST_I32, SEED_CONST[0], SEED_CONST[1],
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::STORE_ARRAY, le(ARRAY_VAR)[0], le(ARRAY_VAR)[1], 0x00, 0x00,
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::LOAD_ARRAY_DEREF, le(REF_VAR)[0], le(REF_VAR)[1], 0x00, 0x00,
        opcode::STORE_VAR_I32, le(READ_VAR)[0], le(READ_VAR)[1],
        opcode::RET_VOID,
    ];
    scan
}

/// A scan that writes element `INDEX` through the reference with
/// `STORE_ARRAY_DEREF`, then reads it back directly into var[2].
fn store_scan() -> Vec<u8> {
    #[rustfmt::skip]
    let scan = vec![
        opcode::LOAD_CONST_I32, SEED_CONST[0], SEED_CONST[1],
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::STORE_ARRAY_DEREF, le(REF_VAR)[0], le(REF_VAR)[1], 0x00, 0x00,
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::LOAD_ARRAY, le(ARRAY_VAR)[0], le(ARRAY_VAR)[1], 0x00, 0x00,
        opcode::STORE_VAR_I32, le(READ_VAR)[0], le(READ_VAR)[1],
        opcode::RET_VOID,
    ];
    scan
}

/// A scan that seeds element `INDEX` directly, then calls `CALLEE` with the
/// reference as its argument and stores what it returns into var[2].
fn call_scan() -> Vec<u8> {
    #[rustfmt::skip]
    let scan = vec![
        opcode::LOAD_CONST_I32, SEED_CONST[0], SEED_CONST[1],
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::STORE_ARRAY, le(ARRAY_VAR)[0], le(ARRAY_VAR)[1], 0x00, 0x00,
        opcode::LOAD_VAR_I64, le(REF_VAR)[0], le(REF_VAR)[1],
        opcode::CALL, le(CALLEE)[0], le(CALLEE)[1], le(CALLEE_FRAME)[0], le(CALLEE_FRAME)[1],
        opcode::STORE_VAR_I32, le(READ_VAR)[0], le(READ_VAR)[1],
        opcode::RET_VOID,
    ];
    scan
}

/// The function `call_scan` calls: its one parameter, the reference, lands
/// in its frame at var[3], and it returns element `INDEX` of the array the
/// reference refers to. Its own scope is `[3, 4)`, which does not cover the
/// array's var[0].
fn callee() -> Vec<u8> {
    #[rustfmt::skip]
    let body = vec![
        opcode::LOAD_CONST_I32, INDEX_CONST[0], INDEX_CONST[1],
        opcode::LOAD_ARRAY_DEREF, le(CALLEE_FRAME)[0], le(CALLEE_FRAME)[1], 0x00, 0x00,
        opcode::RET,
    ];
    body
}

/// A container whose instance under test runs `scan` after an init that
/// stores the array's offset into var[0] and `reference` into var[1],
/// beside the idle second instance.
fn deref_container(scan: &[u8], reference: u64) -> Container {
    #[rustfmt::skip]
    let init: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, OFFSET_CONST[0], OFFSET_CONST[1],
        opcode::STORE_VAR_I32, le(ARRAY_VAR)[0], le(ARRAY_VAR)[1],
        opcode::LOAD_CONST_I64, REF_CONST[0], REF_CONST[1],
        opcode::STORE_VAR_I64, le(REF_VAR)[0], le(REF_VAR)[1],
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(NUM_VARIABLES)
        .data_region_bytes(ELEMENTS * 8)
        .add_i32_constant(0)
        .add_i64_constant(reference as i64)
        .add_i32_constant(INDEX)
        .add_i32_constant(SEED);
    assert_eq!(builder.add_array_descriptor(0, ELEMENTS, 0), 0);
    builder
        .add_function(FunctionId::new(INIT), &init, 2, 4, 0)
        .add_function(FunctionId::new(SCAN), scan, 8, 4, 0)
        .add_function(FunctionId::new(IDLE), &[opcode::RET_VOID], 0, 1, 0)
        .add_function(FunctionId::new(CALLEE), &callee(), 4, 1, 1)
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(ProgramInstanceEntry {
            init_function_id: FunctionId::new(INIT),
            ..program_instance(0, 0, SCAN, 0, 4)
        })
        .add_program_instance(ProgramInstanceEntry {
            init_function_id: FunctionId::new(IDLE),
            ..program_instance(1, 0, IDLE, OTHER_INSTANCE_VAR, 1)
        })
        .max_call_depth(2)
        .build()
}

/// Runs one round, returning var[2] or the trap.
fn run(container: &Container) -> Result<i32, Trap> {
    let mut bufs = VmBuffers::from_container(container);
    let mut vm = load_and_start(container, &mut bufs).unwrap();
    match vm.run_round(0) {
        Ok(_) => Ok(vm.read_variable(VarIndex::new(READ_VAR)).unwrap()),
        Err(fault) => Err(fault.trap),
    }
}

/// A variable reference to the array's variable reaches its element, read
/// or written.
#[rstest]
#[case::load(load_scan())]
#[case::store(store_scan())]
fn execute_when_array_deref_of_variable_reference_then_reaches_referenced_element(
    #[case] scan: Vec<u8>,
) {
    assert_eq!(run(&deref_container(&scan, u64::from(ARRAY_VAR))), Ok(SEED));
}

#[rstest]
#[case::load(load_scan())]
#[case::store(store_scan())]
fn execute_when_array_deref_of_null_then_null_dereference(#[case] scan: Vec<u8>) {
    assert_eq!(
        run(&deref_container(&scan, u64::MAX)),
        Err(Trap::NullDereference)
    );
}

/// A slot holding a value that is not a variable index, a data-region
/// reference or bits a type-punned program wrote, traps rather than naming
/// the variable at `value & 0xFFFF`. Each of these values truncates to
/// var[0], the array's own variable, so a truncating read would have
/// reached the element as if the reference were sound.
#[rstest]
#[case::load(load_scan())]
#[case::store(store_scan())]
fn execute_when_array_deref_of_forged_reference_then_invalid_variable_index(
    #[case] scan: Vec<u8>,
    #[values(data_region_reference(0), 1 << 33, u64::from(u16::MAX) + 1)] reference: u64,
) {
    assert_eq!(
        run(&deref_container(&scan, reference)),
        Err(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))
    );
}

/// A reference into another program instance's partition is outside the
/// instance's scope, as it is for `LOAD_INDIRECT`; the other instance's
/// slot is not read as the array's offset.
#[rstest]
#[case::load(load_scan())]
#[case::store(store_scan())]
fn execute_when_array_deref_into_other_instance_then_invalid_variable_index(#[case] scan: Vec<u8>) {
    assert_eq!(
        run(&deref_container(&scan, u64::from(OTHER_INSTANCE_VAR))),
        Err(Trap::InvalidVariableIndex(VarIndex::new(
            OTHER_INSTANCE_VAR
        )))
    );
}

/// The target is checked against the program instance's scope, not the
/// frame's: a function reads the caller's array through the reference it
/// was passed, though the array's variable lies outside its own frame.
#[test]
fn execute_when_array_deref_in_callee_refers_to_caller_frame_then_reaches_element() {
    assert_eq!(
        run(&deref_container(&call_scan(), u64::from(ARRAY_VAR))),
        Ok(SEED)
    );
}
