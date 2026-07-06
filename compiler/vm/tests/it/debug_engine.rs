//! Integration tests for the VM debug engine (Phase 3): the pause-capable
//! [`DebugHook`], call/return depth callbacks, and the re-entrant
//! `run_round_debug` driver.

use crate::common::VmBuffers;
use ironplc_container::{opcode, ContainerBuilder, FunctionId, VarIndex};
use ironplc_vm::{DebugHook, HookAction, RoundOutcome};

/// Builds a container with init (RET_VOID), a scan entry function, and
/// additional user functions starting at function id 2.
fn call_container(
    scan_bytecode: &[u8],
    user_functions: &[(&[u8], u16, u16, u16)],
    num_vars: u16,
    i32_constants: &[i32],
) -> ironplc_container::Container {
    let init_bytecode = [opcode::RET_VOID];
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    builder = builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, scan_bytecode, 16, num_vars, 0);
    for (i, (bytecode, max_stack, num_locals, num_params)) in user_functions.iter().enumerate() {
        builder = builder.add_function(
            FunctionId::new((i + 2) as u16),
            bytecode,
            *max_stack,
            *num_locals,
            *num_params,
        );
    }
    builder
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

/// A hook that records the sequence of instruction / call / return events
/// and never pauses.
#[derive(Default)]
struct RecordingHook {
    events: Vec<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Instr(FunctionId, usize),
    Call(FunctionId),
    Return(Option<FunctionId>),
}

impl DebugHook for RecordingHook {
    fn before_instruction(&mut self, function_id: FunctionId, pc: usize, _op: u8) -> HookAction {
        self.events.push(Event::Instr(function_id, pc));
        HookAction::Continue
    }
    fn before_call(&mut self, callee: FunctionId) {
        self.events.push(Event::Call(callee));
    }
    fn after_return(&mut self, returning_to: Option<FunctionId>) {
        self.events.push(Event::Return(returning_to));
    }
}

/// Scan calls a doubling function, so the debug driver should observe a
/// `before_call` bracketing the callee's instructions and `after_return`
/// callbacks that mirror each frame pop.
#[test]
fn run_round_debug_when_call_then_call_and_return_callbacks_bracket_callee() {
    // Function 2 (var_offset=2, one param at var[2]): doubles its argument.
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::ADD_I32,
        opcode::RET,
    ];
    // Scan: push 21, call func 2, store result to var[0], return.
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::CALL, 0x02, 0x00, 0x02, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];

    let c = call_container(&scan_bytecode, &[(&func_body, 4, 1, 1)], 3, &[21]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let mut hook = RecordingHook::default();
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Completed);
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);

    let func2 = FunctionId::new(2);

    // Exactly one call, to func 2.
    let calls: Vec<FunctionId> = hook
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Call(f) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(calls, vec![func2]);

    // Two returns: func2 -> SCAN, then SCAN -> None (outermost).
    let returns: Vec<Option<FunctionId>> = hook
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Return(r) => Some(*r),
            _ => None,
        })
        .collect();
    assert_eq!(returns, vec![Some(FunctionId::SCAN), None]);

    // The call event precedes every func2 instruction, and the first return
    // (to SCAN) follows them — the callee's instructions are bracketed.
    let call_idx = hook
        .events
        .iter()
        .position(|e| matches!(e, Event::Call(_)))
        .unwrap();
    let return_to_scan_idx = hook
        .events
        .iter()
        .position(|e| matches!(e, Event::Return(Some(_))))
        .unwrap();
    let func2_instr_idxs: Vec<usize> = hook
        .events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Event::Instr(f, _) if *f == func2 => Some(i),
            _ => None,
        })
        .collect();
    assert!(!func2_instr_idxs.is_empty(), "callee ran no instructions");
    assert!(func2_instr_idxs.iter().all(|&i| i > call_idx));
    assert!(func2_instr_idxs.iter().all(|&i| i < return_to_scan_idx));
}
