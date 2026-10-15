//! VM-specific tests for the BUILTIN CHECK_SUBSCRIPT_* opcodes, which pass
//! an array subscript in the range of `DINT` on as a `DINT` and trap on any
//! other.
//!
//! The end-to-end subscript tests (`end_to_end_traps.rs`) cover the
//! subscripts a program reaches them with. These cover the edges of the
//! range for each operand type.

use ironplc_vm::error::Trap;
use rstest::rstest;

/// `BUILTIN func_id` on the 64-bit constant at pool[0], stored to var[0].
fn check_i64_constant(func_id: u16) -> Vec<u8> {
    let [low, high] = func_id.to_le_bytes();
    #[rustfmt::skip]
    let bytecode = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0x94, low, high,   // BUILTIN func_id
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

/// `BUILTIN func_id` on the 32-bit constant at pool[0], stored to var[0].
fn check_i32_constant(func_id: u16) -> Vec<u8> {
    let [low, high] = func_id.to_le_bytes();
    #[rustfmt::skip]
    let bytecode = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0x94, low, high,   // BUILTIN func_id
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    bytecode
}

#[rstest]
#[case::i64_min_dint(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_I64, i32::MIN.into())]
#[case::i64_max_dint(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_I64, i32::MAX.into())]
#[case::u64_max_dint(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U64, i32::MAX.into())]
#[case::u64_zero(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U64, 0)]
fn execute_when_64_bit_subscript_in_dint_then_dint(#[case] func_id: u16, #[case] value: i64) {
    assert_eq!(
        crate::common::run_and_read_i64(&check_i64_constant(func_id), 1, &[value]),
        value
    );
}

#[rstest]
#[case::i64_above_dint(
    ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_I64,
    i64::from(i32::MAX) + 1,
    2147483648
)]
#[case::i64_below_dint(
    ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_I64,
    i64::from(i32::MIN) - 1,
    -2147483649
)]
#[case::u64_above_dint(
    ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U64,
    i64::from(i32::MAX) + 1,
    2147483648
)]
#[case::u64_top_bit_set(
    ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U64,
    -1,
    18446744073709551615
)]
fn execute_when_64_bit_subscript_outside_dint_then_trap(
    #[case] func_id: u16,
    #[case] value: i64,
    #[case] reported: i128,
) {
    assert_eq!(
        crate::common::run_and_expect_trap_i64(&check_i64_constant(func_id), 1, &[value]),
        Trap::SubscriptOutOfRange { value: reported }
    );
}

#[test]
fn execute_when_u32_subscript_in_dint_then_dint() {
    let bytecode = check_i32_constant(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U32);
    assert_eq!(
        crate::common::run_and_read_i32(&bytecode, 1, &[i32::MAX]),
        i32::MAX
    );
}

#[test]
fn execute_when_u32_subscript_top_bit_set_then_trap() {
    // The bits of -1 are u32::MAX, which a DINT does not hold.
    let bytecode = check_i32_constant(ironplc_container::opcode::builtin::CHECK_SUBSCRIPT_U32);
    assert_eq!(
        crate::common::run_and_expect_trap_i32(&bytecode, 1, &[-1]),
        Trap::SubscriptOutOfRange { value: 4294967295 }
    );
}
