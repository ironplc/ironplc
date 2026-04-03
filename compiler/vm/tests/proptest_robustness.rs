//! Property-based tests for VM robustness.
//!
//! These tests verify that the VM never panics on arbitrary input
//! and that arithmetic identities hold across the full value range.

mod common;

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
        let c = common::single_function_container(&bytecode, 256, &constants);
        let mut b = VmBuffers::from_container(&c);
        if let Ok(mut vm) = common::load_and_start(&c, &mut b) {
            // We don't care whether it succeeds or traps --
            // only that it doesn't panic.
            let _ = vm.run_round(0);
        }
    }
}

proptest! {
    #[test]
    fn execute_when_add_i32_with_zero_then_identity(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (0)
            0x30,              // ADD_I32
            0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0xB5,              // RET_VOID
        ];
        let result = common::run_and_read_i32(&bytecode, 1, &[a, 0]);
        prop_assert_eq!(result, a);
    }

    #[test]
    fn execute_when_mul_i32_with_one_then_identity(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (1)
            0x32,              // MUL_I32
            0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0xB5,              // RET_VOID
        ];
        let result = common::run_and_read_i32(&bytecode, 1, &[a, 1]);
        prop_assert_eq!(result, a);
    }

    #[test]
    fn execute_when_sub_i32_self_then_zero(a in any::<i32>()) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (a)
            0x31,              // SUB_I32
            0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
            0xB5,              // RET_VOID
        ];
        let result = common::run_and_read_i32(&bytecode, 1, &[a]);
        prop_assert_eq!(result, 0);
    }
}
