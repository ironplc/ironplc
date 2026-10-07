//! Property-based tests for VM robustness.
//!
//! These tests verify that the VM never panics on arbitrary input
//! and that arithmetic identities hold across the full value range.

use ironplc_vm::VmBuffers;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn execute_when_arbitrary_bytecode_then_never_panics(
        bytecode in proptest::collection::vec(any::<u8>(), 0..512)
    ) {
        // Provide a generous container so that valid-looking operand
        // indices have something to hit rather than always trapping
        // on the first operand.
        let constants: Vec<i32> = (0..256).collect();
        let c = crate::common::single_function_container(&bytecode, 256, &constants);
        let mut b = VmBuffers::from_container(&c);
        if let Ok(mut vm) = crate::common::load_and_start(&c, &mut b) {
            // We don't care whether it succeeds or traps --
            // only that it doesn't panic.
            let _ = vm.run_round(0);
        }
    }
}

/// A `u64` biased toward the edges where scheduler arithmetic can overflow.
fn edge_biased_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0),
        Just(1),
        Just(1 << 63),
        Just(u64::MAX - 1),
        Just(u64::MAX),
        any::<u64>(),
    ]
}

proptest! {
    #[test]
    fn run_round_when_cyclic_interval_and_uptime_arbitrary_then_never_panics(
        interval_us in edge_biased_u64(),
        uptimes_us in proptest::collection::vec(edge_biased_u64(), 1..8),
    ) {
        let c = crate::scenarios::cyclic_task_container(interval_us);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        for uptime_us in uptimes_us {
            prop_assert!(vm.run_round(uptime_us).is_ok());
        }
    }
}

proptest! {
    #[test]
    fn execute_when_add_i32_with_zero_then_identity(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (0)
            0x20,              // ADD_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, 0]);
        prop_assert_eq!(result, a);
    }

    #[test]
    fn execute_when_mul_i32_with_one_then_identity(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (1)
            0x28,              // MUL_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, 1]);
        prop_assert_eq!(result, a);
    }

    #[test]
    fn execute_when_sub_i32_self_then_zero(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x24,              // SUB_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a]);
        prop_assert_eq!(result, 0);
    }
}

// Cross-check the i32 arithmetic opcodes against plain Rust arithmetic over a
// wide range of operands. Inputs are constrained so the result cannot overflow
// i32, which keeps the oracle unambiguous (plain `a + b` / `a - b` / `a * b`)
// regardless of the VM's implementation-defined overflow behaviour. This
// complements the identity tests above, which only exercise `a op {0,1,a}`.
proptest! {
    #[test]
    fn execute_when_add_i32_no_overflow_then_matches_rust(
        a in -1_000_000_000_i32..=1_000_000_000,
        b in -1_000_000_000_i32..=1_000_000_000,
    ) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (b)
            0x20,              // ADD_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, a + b);
    }

    #[test]
    fn execute_when_sub_i32_no_overflow_then_matches_rust(
        a in -1_000_000_000_i32..=1_000_000_000,
        b in -1_000_000_000_i32..=1_000_000_000,
    ) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (b)
            0x24,              // SUB_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, a - b);
    }

    #[test]
    fn execute_when_mul_i32_no_overflow_then_matches_rust(
        // 46340^2 < i32::MAX, so the product always fits.
        a in -46_340_i32..=46_340,
        b in -46_340_i32..=46_340,
    ) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (b)
            0x28,              // MUL_I32
            0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0x8C,              // RET_VOID
        ];
        let result = crate::common::run_and_read_i32(&bytecode, 1, &[a, b]);
        prop_assert_eq!(result, a * b);
    }
}
