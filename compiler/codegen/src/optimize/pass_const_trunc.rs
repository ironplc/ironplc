//! Resolves `LOAD_CONST_I32 p; TRUNC_*` at compile time.
//!
//! Sub-32-bit integer types are promoted to 32 bits on load, and codegen
//! emits an explicit `TRUNC_*` before any store back to a narrow slot
//! (ADR-0001). `emit_truncation` cannot tell whether the value on the stack
//! came from a constant, because it is handed an `OpType` — a width and a
//! signedness — while the `storage_bits` that select the truncation width
//! live one level up in `VarTypeInfo`. So a narrow constant store emits the
//! constant at 32 bits and narrows it with an instruction the compiler could
//! have evaluated itself.
//!
//! When the constant already fits the narrow type the `TRUNC_*` is a no-op
//! and is dropped. When it does not, the truncated value is interned and the
//! load's pool operand rewritten, which is what the instruction would have
//! computed anyway: the VM is unconditionally wrapping, so `x : USINT := 300`
//! stores `44` either way.
//!
//! Only `LOAD_CONST_I32` can precede a `TRUNC_*` with a foldable value —
//! `TRUNC_*` takes an I32 operand, and `BOOL` (the one type reached by
//! `LOAD_TRUE`/`LOAD_FALSE`) has `storage_bits: 1`, for which
//! `emit_truncation` already emits nothing. A `DUP` left by the emitter's
//! consecutive-load peephole is not matched, so that `TRUNC_*` survives.

use std::collections::HashSet;

use ironplc_container::opcode;

use super::rewrite::{apply_peephole, Action, Instruction};
use super::OffsetMap;
use crate::compile::{intern_i32_constant, PoolConstant};

pub(super) fn apply(
    bytecode: &[u8],
    protected: &HashSet<usize>,
    constants: &mut Vec<PoolConstant>,
) -> (Vec<u8>, OffsetMap) {
    apply_peephole(bytecode, protected, |a, b| {
        match_const_trunc(a, b, constants)
    })
}

/// Applies a `TRUNC_*` opcode to `value`, or `None` if `op` is not one.
///
/// Mirrors the four truncation arms of the VM's dispatch loop exactly; a
/// differential test in `optimize/tests.rs` pins the two together.
fn trunc_fold_value(op: u8, value: i32) -> Option<i32> {
    match op {
        opcode::TRUNC_I8 => Some((value as i8) as i32),
        opcode::TRUNC_U8 => Some((value as u8) as i32),
        opcode::TRUNC_I16 => Some((value as i16) as i32),
        opcode::TRUNC_U16 => Some((value as u16) as i32),
        _ => None,
    }
}

fn match_const_trunc(
    a: &Instruction,
    b: &Instruction,
    constants: &mut Vec<PoolConstant>,
) -> Option<[Action; 2]> {
    if a.opcode() != opcode::LOAD_CONST_I32 || a.bytes.len() != 3 {
        return None;
    }
    // A pool index that is out of bounds, or names a constant of another
    // type, is left alone rather than folded: this pass is not the place to
    // diagnose a malformed pool reference.
    let Some(&PoolConstant::I32(value)) = constants.get(a.u16_operand() as usize) else {
        return None;
    };
    let folded = trunc_fold_value(b.opcode(), value)?;

    if folded == value {
        return Some([Action::Keep, Action::Remove]);
    }
    let pool_index = intern_i32_constant(constants, folded);
    Some([Action::RewriteOperand(pool_index), Action::Remove])
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn load_const_i32(idx: u16) -> Vec<u8> {
        let mut v = vec![opcode::LOAD_CONST_I32];
        v.extend_from_slice(&idx.to_le_bytes());
        v
    }

    /// `LOAD_CONST_I32 pool:0; <trunc_op>; RET_VOID`.
    fn const_then_trunc(trunc_op: u8) -> Vec<u8> {
        let mut bytecode = load_const_i32(0);
        bytecode.push(trunc_op);
        bytecode.push(opcode::RET_VOID);
        bytecode
    }

    #[test]
    fn apply_when_constant_in_range_then_removes_trunc_and_keeps_load() {
        let mut constants = vec![PoolConstant::I32(42)];
        let (result, _) = apply(
            &const_then_trunc(opcode::TRUNC_I8),
            &HashSet::new(),
            &mut constants,
        );

        let mut expected = load_const_i32(0);
        expected.push(opcode::RET_VOID);
        assert_eq!(result, expected);
        assert_eq!(constants, vec![PoolConstant::I32(42)]);
    }

    #[test]
    fn apply_when_constant_out_of_range_then_rewrites_operand_to_folded_value() {
        // 300 does not fit u8; the truncated value 44 is appended to the pool
        // and the load rewritten to point at it.
        let mut constants = vec![PoolConstant::I32(300)];
        let (result, _) = apply(
            &const_then_trunc(opcode::TRUNC_U8),
            &HashSet::new(),
            &mut constants,
        );

        let mut expected = load_const_i32(1);
        expected.push(opcode::RET_VOID);
        assert_eq!(result, expected);
        assert_eq!(
            constants,
            vec![PoolConstant::I32(300), PoolConstant::I32(44)]
        );
    }

    #[test]
    fn apply_when_folded_value_already_in_pool_then_reuses_the_entry() {
        let mut constants = vec![PoolConstant::I32(44), PoolConstant::I32(300)];
        let mut bytecode = load_const_i32(1);
        bytecode.push(opcode::TRUNC_U8);
        bytecode.push(opcode::RET_VOID);

        let (result, _) = apply(&bytecode, &HashSet::new(), &mut constants);

        let mut expected = load_const_i32(0);
        expected.push(opcode::RET_VOID);
        assert_eq!(result, expected);
        assert_eq!(
            constants,
            vec![PoolConstant::I32(44), PoolConstant::I32(300)],
            "the pool must not grow when the folded value is already present"
        );
    }

    #[test]
    fn apply_when_pool_entry_is_not_i32_then_no_change() {
        let mut constants = vec![PoolConstant::I64(42)];
        let bytecode = const_then_trunc(opcode::TRUNC_I8);
        let (result, _) = apply(&bytecode, &HashSet::new(), &mut constants);

        assert_eq!(result, bytecode);
    }

    #[test]
    fn apply_when_pool_index_out_of_bounds_then_no_change() {
        let mut constants = Vec::new();
        let bytecode = const_then_trunc(opcode::TRUNC_I8);
        let (result, _) = apply(&bytecode, &HashSet::new(), &mut constants);

        assert_eq!(result, bytecode);
    }

    #[test]
    fn apply_when_dup_precedes_trunc_then_no_change() {
        // The emitter's consecutive-load peephole can leave a DUP where a
        // second load would have been. The value behind it is not visible to
        // this pass, so the TRUNC has to stay.
        let mut constants = vec![PoolConstant::I32(300)];
        let mut bytecode = load_const_i32(0);
        bytecode.push(opcode::DUP);
        bytecode.push(opcode::TRUNC_U8);
        bytecode.push(opcode::RET_VOID);

        let (result, _) = apply(&bytecode, &HashSet::new(), &mut constants);

        assert_eq!(result, bytecode);
    }

    #[test]
    fn apply_when_second_instruction_is_not_trunc_then_no_change() {
        let mut constants = vec![PoolConstant::I32(42)];
        let mut bytecode = load_const_i32(0);
        bytecode.push(opcode::NEG_I32);
        bytecode.push(opcode::RET_VOID);

        let (result, _) = apply(&bytecode, &HashSet::new(), &mut constants);

        assert_eq!(result, bytecode);
    }

    #[test]
    fn trunc_fold_value_when_not_a_trunc_opcode_then_none() {
        assert!(trunc_fold_value(opcode::NEG_I32, 1).is_none());
    }

    proptest! {
        /// The fold must agree with the VM for every `i32`, or a program's
        /// meaning changes depending on whether its value happened to be a
        /// constant. These mirror the four `TRUNC_*` arms of `vm.rs`; the
        /// paired end-to-end tests in `codegen/tests/it/end_to_end_const_trunc.rs`
        /// run the same values through the real VM.
        #[test]
        fn trunc_fold_value_when_any_i32_then_matches_vm_semantics(value: i32) {
            prop_assert_eq!(
                trunc_fold_value(opcode::TRUNC_I8, value),
                Some((value as i8) as i32)
            );
            prop_assert_eq!(
                trunc_fold_value(opcode::TRUNC_U8, value),
                Some((value as u8) as i32)
            );
            prop_assert_eq!(
                trunc_fold_value(opcode::TRUNC_I16, value),
                Some((value as i16) as i32)
            );
            prop_assert_eq!(
                trunc_fold_value(opcode::TRUNC_U16, value),
                Some((value as u16) as i32)
            );
        }

        /// Folding never changes what a narrow store observes: either the
        /// constant already fits and the `TRUNC` is dropped, or the rewritten
        /// pool entry holds exactly what the `TRUNC` would have produced.
        #[test]
        fn apply_when_any_constant_then_value_reaching_the_store_is_unchanged(
            value: i32,
            op_index in 0usize..4,
        ) {
            let trunc_op = [
                opcode::TRUNC_I8,
                opcode::TRUNC_U8,
                opcode::TRUNC_I16,
                opcode::TRUNC_U16,
            ][op_index];
            let expected = trunc_fold_value(trunc_op, value).unwrap();

            let mut constants = vec![PoolConstant::I32(value)];
            let (result, _) = apply(&const_then_trunc(trunc_op), &HashSet::new(), &mut constants);

            prop_assert_eq!(result[0], opcode::LOAD_CONST_I32);
            prop_assert!(!result.contains(&trunc_op));
            let pool_index = u16::from_le_bytes([result[1], result[2]]);
            prop_assert_eq!(&constants[pool_index as usize], &PoolConstant::I32(expected));
        }
    }
}
