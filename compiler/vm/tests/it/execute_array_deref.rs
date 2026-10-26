//! VM tests for `LOAD_ARRAY_DEREF` and `STORE_ARRAY_DEREF`: an array element
//! read or written through the variable reference a variable holds, and the
//! values in that slot that trap instead of being followed to a variable.

use crate::common::{freewheeling_task, load_and_start, program_instance, VmBuffers};
use ironplc_container::opcode;
use ironplc_container::reference::data_region_reference;
use ironplc_container::{Container, ContainerBuilder, FunctionId, ProgramInstanceEntry, VarIndex};
use ironplc_vm::error::Trap;
use rstest::rstest;

/// Variable layout of the program instance that follows the reference:
/// var[0] holds the array's data offset, var[1] the reference, var[2] the
/// element read back.
const ARRAY_VAR: u16 = 0;
const REF_VAR: u16 = 1;
const READ_VAR: u16 = 2;
/// The one variable of a second program instance, which a reference the
/// first instance holds must not reach.
const OTHER_INSTANCE_VAR: u16 = 3;

/// The array's size, and the element index and value the tests use.
const ELEMENTS: u32 = 4;
const INDEX: i32 = 2;
const VALUE: i32 = 42;

/// Constant pool indices.
const OFFSET_CONST: u8 = 0;
const REFERENCE_CONST: u8 = 1;
const INDEX_CONST: u8 = 2;
const VALUE_CONST: u8 = 3;

fn le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

/// A container of two program instances on one task. The first, whose
/// variables are `[0, 3)`, stores the array's data offset into var[0] and
/// `reference` into var[1] when initialized, then runs `scan`. The second,
/// whose one variable is var[3], runs nothing.
fn deref_container(scan: &[u8], reference: u64) -> Container {
    #[rustfmt::skip]
    let init: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, OFFSET_CONST, 0x00,
        opcode::STORE_VAR_I32,  le(ARRAY_VAR)[0], le(ARRAY_VAR)[1],
        opcode::LOAD_CONST_I64, REFERENCE_CONST, 0x00,
        opcode::STORE_VAR_I64,  le(REF_VAR)[0], le(REF_VAR)[1],
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(4)
        .data_region_bytes(ELEMENTS * 8)
        .add_i32_constant(0)
        .add_i64_constant(reference as i64)
        .add_i32_constant(INDEX)
        .add_i32_constant(VALUE);
    builder.add_array_descriptor(0, ELEMENTS, 0);
    builder
        .add_function(FunctionId::INIT, &init, 1, 4, 0)
        .add_function(FunctionId::SCAN, scan, 2, 4, 0)
        .add_function(FunctionId::new(2), &[opcode::RET_VOID], 0, 4, 0)
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(program_instance(0, 0, 1, ARRAY_VAR, 3))
        .add_program_instance(ProgramInstanceEntry {
            init_function_id: FunctionId::new(2),
            ..program_instance(1, 0, 2, OTHER_INSTANCE_VAR, 1)
        })
        .max_call_depth(1)
        .build()
}

/// A scan that reads element `INDEX` through the reference into var[2].
#[rustfmt::skip]
fn load_scan() -> Vec<u8> {
    vec![
        opcode::LOAD_CONST_I32, INDEX_CONST, 0x00,
        opcode::LOAD_ARRAY_DEREF, le(REF_VAR)[0], le(REF_VAR)[1], 0x00, 0x00,
        opcode::STORE_VAR_I32, le(READ_VAR)[0], le(READ_VAR)[1],
        opcode::RET_VOID,
    ]
}

/// A scan that writes `VALUE` to element `INDEX` through the reference.
/// `STORE_ARRAY_DEREF` pops the index then the value, so the value goes
/// first.
#[rustfmt::skip]
fn store_scan() -> Vec<u8> {
    vec![
        opcode::LOAD_CONST_I32, VALUE_CONST, 0x00,
        opcode::LOAD_CONST_I32, INDEX_CONST, 0x00,
        opcode::STORE_ARRAY_DEREF, le(REF_VAR)[0], le(REF_VAR)[1], 0x00, 0x00,
        opcode::RET_VOID,
    ]
}

/// A scan that writes `VALUE` to element `INDEX` through the reference and
/// reads it back through the reference into var[2].
fn store_then_load_scan() -> Vec<u8> {
    let mut scan = store_scan();
    scan.pop();
    scan.extend(load_scan());
    scan
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

/// A variable reference to the instance's own array variable reaches the
/// array's elements for both opcodes.
#[test]
fn execute_when_store_then_load_array_deref_then_element_roundtrips() {
    let c = deref_container(&store_then_load_scan(), u64::from(ARRAY_VAR));

    assert_eq!(run(&c), Ok(VALUE));
}

#[rstest]
fn execute_when_array_deref_of_null_then_null_dereference(
    #[values(load_scan(), store_scan())] scan: Vec<u8>,
) {
    assert_eq!(
        run(&deref_container(&scan, u64::MAX)),
        Err(Trap::NullDereference)
    );
}

/// A slot that does not hold a variable reference within the instance's
/// scope traps, as `LOAD_INDIRECT` does, rather than being read as one: a
/// data-region reference and a value above `u16::MAX` would otherwise be
/// truncated to a variable index (here var[0], the instance's own array),
/// and a variable of another program instance is outside the scope.
#[rstest]
#[case::data_region_reference(data_region_reference(0), u16::MAX)]
#[case::above_u16_max(u64::from(u16::MAX) + 1, u16::MAX)]
#[case::other_instance(u64::from(OTHER_INSTANCE_VAR), OTHER_INSTANCE_VAR)]
fn execute_when_array_deref_of_forged_reference_then_invalid_variable_index(
    #[values(load_scan(), store_scan())] scan: Vec<u8>,
    #[case] reference: u64,
    #[case] index: u16,
) {
    assert_eq!(
        run(&deref_container(&scan, reference)),
        Err(Trap::InvalidVariableIndex(VarIndex::new(index)))
    );
}
