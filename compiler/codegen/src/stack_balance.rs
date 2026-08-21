//! Compile-time enforcement of the operand-stack balance invariant.
//!
//! Every function codegen emits must leave the VM's operand stack exactly
//! as it found it: a body that returns without consuming a value it pushed
//! (or that pops one it never pushed) is a codegen bug. The VM cannot
//! recover from it — `compiler/vm/src/stack.rs` has no `clear`, and
//! `run_round` never truncates — so a single leaked slot survives every
//! subsequent scan round and accumulates until the operand stack overflows,
//! with a `Trap::StackOverflow` pointing nowhere near the instruction
//! responsible.
//!
//! This module is the choke point that stops such bytecode from reaching a
//! container. The analysis itself lives in [`ironplc_container::verify`],
//! beside the opcode tables its stack-effect model must agree with; this
//! module supplies the codegen-facing entry point and turns a violation
//! into a compiler diagnostic.

use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

/// Verifies operand-stack discipline across every function in `container`.
///
/// A violation is reported as [`Problem::InternalError`] (P9998), not as a
/// user-facing program error: reaching this point means codegen emitted
/// bytecode the VM cannot run correctly, which is a bug in the compiler
/// rather than a mistake in the program being compiled.
///
/// [`Problem::InternalError`]: ironplc_problems::Problem::InternalError
pub(crate) fn verify_container(container: &Container) -> Result<(), Diagnostic> {
    ironplc_container::verify_stack_balance(&container.code).map_err(|imbalance| {
        let rule = imbalance
            .rule()
            .map(|r| format!(" (verifier rule {r})"))
            .unwrap_or_default();
        Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            format!("codegen emitted bytecode that is not stack-balanced{rule}: {imbalance}"),
        ))
    })
}

#[cfg(test)]
mod tests {
    use ironplc_container::{
        opcode, verify_stack_balance, ContainerBuilder, FunctionId, StackImbalance, VarIndex,
    };

    use crate::emit::Emitter;

    /// Wraps an emitter's finished bytecode in a runnable single-function
    /// container, declaring the stack depth the emitter itself computed.
    ///
    /// This is the same sequence `compile_program_with_functions` performs,
    /// reduced to one function — so what these tests feed the verifier is
    /// what codegen would have shipped.
    fn container_from(emitter: &mut Emitter, num_params: u16) -> ironplc_container::Container {
        let max_stack_depth = {
            let _ = emitter.bytecode();
            emitter.max_stack_depth()
        };
        let bytecode = emitter.bytecode().to_vec();
        ContainerBuilder::new()
            .num_variables(4)
            .max_call_depth(1)
            .add_function(FunctionId::INIT, &bytecode, max_stack_depth, 4, num_params)
            .build()
    }

    #[test]
    fn verify_container_when_emitter_leaks_a_slot_then_unbalanced_return() {
        // A spare push with no matching pop: the value loaded here is never
        // stored, consumed by an operator, or popped.
        let mut emitter = Emitter::new();
        emitter.emit_load_const_i32(0);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter, 0);
        let result = verify_stack_balance(&container.code);

        assert!(matches!(
            result,
            Err(StackImbalance::UnbalancedReturn {
                expected: 0,
                actual: 1,
                ..
            })
        ));
    }

    #[test]
    fn verify_container_when_emitter_leaks_a_slot_then_diagnostic_is_internal_error() {
        let mut emitter = Emitter::new();
        emitter.emit_load_const_i32(0);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter, 0);
        let diagnostic = super::verify_container(&container).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(diagnostic.primary.message.contains("not stack-balanced"));
    }

    #[test]
    fn verify_container_when_emitter_over_pops_then_underflow() {
        // A store with nothing on the stack. `Emitter::pop_stack` uses
        // `saturating_sub`, so the emitter's own counter clamps at zero and
        // never notices.
        let mut emitter = Emitter::new();
        emitter.emit_store_var_i32(VarIndex::new(0));
        emitter.emit_ret_void();

        let container = container_from(&mut emitter, 0);
        let result = verify_stack_balance(&container.code);

        assert!(matches!(
            result,
            Err(StackImbalance::Underflow {
                opcode: opcode::STORE_VAR_I32,
                needs: 1,
                has: 0,
                ..
            })
        ));
    }

    #[test]
    fn emitter_max_stack_depth_when_over_pop_then_reports_zero_and_hides_the_defect() {
        // The pre-existing machinery's view of the over-popping function:
        // a completely ordinary function that needs no stack at all. This
        // is what "silently accepted before" means concretely -- there was
        // no signal to act on.
        let mut emitter = Emitter::new();
        emitter.emit_store_var_i32(VarIndex::new(0));
        emitter.emit_ret_void();
        let _ = emitter.bytecode();

        assert_eq!(emitter.max_stack_depth(), 0);
    }

    #[test]
    fn container_builder_when_bytecode_unbalanced_then_still_builds() {
        // The other half of "silently accepted before": neither the emitter
        // nor the container builder rejects either defect. Both produce a
        // well-formed container that the VM will happily load and run.
        let mut leaky = Emitter::new();
        leaky.emit_load_const_i32(0);
        leaky.emit_ret_void();
        let leak_container = container_from(&mut leaky, 0);

        let mut over_popping = Emitter::new();
        over_popping.emit_store_var_i32(VarIndex::new(0));
        over_popping.emit_ret_void();
        let over_pop_container = container_from(&mut over_popping, 0);

        assert_eq!(leak_container.header.num_functions, 1);
        assert_eq!(over_pop_container.header.num_functions, 1);
    }

    #[test]
    fn vm_when_leaked_slot_then_operand_stack_retains_it_across_the_scan_boundary() {
        // The runtime consequence the compile-time check prevents. The
        // leaking body runs as the init function, so this observes the
        // operand stack at the first scan boundary: the slot is still
        // there, and nothing in the VM will ever remove it.
        let mut emitter = Emitter::new();
        emitter.emit_load_const_i32(0);
        emitter.emit_ret_void();

        let mut container = container_from(&mut emitter, 0);
        // The container needs a constant for LOAD_CONST_I32 pool index 0 and
        // headroom on the operand stack, neither of which the leaking body
        // declares for itself.
        container
            .constant_pool
            .push(ironplc_container::ConstEntry::primitive_le(
                ironplc_container::ConstType::I32,
                &7i32.to_le_bytes(),
            ));
        container.header.max_stack_depth = 4;

        let mut bufs = ironplc_vm::VmBuffers::from_container(&container);
        let vm = ironplc_vm::Vm::new().load(&container, &mut bufs).start();

        assert!(vm.is_ok());
        assert_eq!(vm.unwrap().operand_stack_depth(), 1);
    }

    #[test]
    fn verify_container_when_branching_body_is_balanced_then_ok() {
        // The false-positive guard, built through the same API: a
        // conditional whose arms both leave the stack empty. A linear
        // end-of-function counter cannot distinguish this from the leaking
        // body above; walking the control-flow graph can.
        let mut emitter = Emitter::new();
        let else_label = emitter.create_label();
        let end_label = emitter.create_label();

        emitter.emit_load_var_i32(VarIndex::new(0));
        emitter.emit_jmp_if_not(else_label);
        emitter.emit_load_const_i32(0);
        emitter.emit_store_var_i32(VarIndex::new(1));
        emitter.emit_jmp(end_label);
        emitter.bind_label(else_label);
        emitter.emit_load_const_i32(1);
        emitter.emit_store_var_i32(VarIndex::new(1));
        emitter.bind_label(end_label);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter, 0);

        assert_eq!(verify_stack_balance(&container.code), Ok(()));
    }

    #[test]
    fn verify_container_when_early_ret_carries_a_value_then_ok() {
        // The specific shape the task warned a naive `== 0` assert would
        // reject: a value-returning body with an early RET. Both exits leave
        // exactly one value, but the emitter's linear counter finishes at 2.
        let mut emitter = Emitter::new();
        let skip = emitter.create_label();

        emitter.emit_load_var_i32(VarIndex::new(0));
        emitter.emit_jmp_if_not(skip);
        emitter.emit_load_const_i32(0);
        emitter.emit_ret();
        emitter.bind_label(skip);
        emitter.emit_load_const_i32(1);
        emitter.emit_ret();

        let container = container_from(&mut emitter, 0);

        assert_eq!(verify_stack_balance(&container.code), Ok(()));
        // The linear counter that a cheaper check would have asserted on is
        // non-zero here, on a body that is in fact perfectly balanced.
        assert_ne!(emitter.max_stack_depth(), 0);
    }
}
