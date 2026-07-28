//! Integration tests for the VM debug engine (Phase 3): the pause-capable
//! [`DebugHook`], call/return depth callbacks, and the re-entrant
//! `run_round_debug` driver.

use crate::common::{single_function_container, VmBuffers};
use ironplc_container::{opcode, ContainerBuilder, FunctionId, VarIndex};
use ironplc_vm::{
    BreakpointTable, DebugHook, DebuggerHook, HookAction, PauseReason, Phase, RoundOutcome,
};

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
        // Call tests nest SCAN -> callee (2 frames); depth 2 fits every
        // debug scenario including the single-level CALL cases.
        .max_call_depth(2)
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

/// Steel-thread scan: x := 10; y := x + 32. Used for breakpoint tests.
/// Offsets: LOAD_CONST\@0 STORE_VAR x\@3 LOAD_VAR x\@6 LOAD_CONST\@9
/// ADD\@12 STORE_VAR y\@13 RET_VOID\@16.
fn steel_thread_scan() -> Vec<u8> {
    #[rustfmt::skip]
    let bytecode = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::STORE_VAR_I32,  0x00, 0x00,
        opcode::LOAD_VAR_I32,   0x00, 0x00,
        opcode::LOAD_CONST_I32, 0x01, 0x00,
        opcode::ADD_I32,
        opcode::STORE_VAR_I32,  0x01, 0x00,
        opcode::RET_VOID,
    ];
    bytecode
}

#[test]
fn run_round_debug_when_breakpoint_in_entry_then_pauses_there_and_resumes_to_completion() {
    let c = single_function_container(&steel_thread_scan(), 2, &[10, 32]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let mut table = BreakpointTable::new();
    let id = table.add(FunctionId::SCAN, 6); // LOAD_VAR x, after x := 10
    let mut hook = DebuggerHook::new(&table);

    // First round pauses at the breakpoint.
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Paused(PauseReason::Breakpoint(id)));
    assert_eq!(vm.phase(), Phase::PausedAt(PauseReason::Breakpoint(id)));

    // The top frame sits exactly on the breakpoint instruction.
    let frames = vm.debug_frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].function_id, FunctionId::SCAN);
    assert_eq!(frames[0].pc, 6);

    // x has been assigned (offset 3 executed), y not yet.
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 0);

    // Resuming completes the scan; final state matches an unhooked run.
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Completed);
    assert_eq!(vm.phase(), Phase::CompletedScan);
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 42);
}

#[test]
fn run_round_debug_when_breakpoint_in_callee_then_frame_stack_shows_caller_beneath() {
    // Same doubling function as the callback test.
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::ADD_I32,
        opcode::RET,
    ];
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

    let func2 = FunctionId::new(2);
    let mut table = BreakpointTable::new();
    let id = table.add(func2, 3); // second LOAD_VAR inside the callee
    let mut hook = DebuggerHook::new(&table);

    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Paused(PauseReason::Breakpoint(id)));

    // Two frames: SCAN beneath, callee on top at the breakpoint offset.
    let frames = vm.debug_frames();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].function_id, FunctionId::SCAN);
    assert_eq!(frames[1].function_id, func2);
    assert_eq!(frames[1].pc, 3);

    // Resume runs to completion with the correct result.
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Completed);
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);
}

/// Pauses before every instruction, advancing exactly one instruction per
/// resume, so a full scan is replayed one boundary at a time.
#[derive(Default)]
struct PauseEachInstruction {
    skip: bool,
    instructions: usize,
}

impl DebugHook for PauseEachInstruction {
    fn before_instruction(&mut self, _f: FunctionId, _pc: usize, _op: u8) -> HookAction {
        if self.skip {
            self.skip = false;
            self.instructions += 1;
            HookAction::Continue
        } else {
            self.skip = true;
            HookAction::Pause(PauseReason::Step)
        }
    }
}

#[test]
fn run_round_debug_when_paused_at_every_instruction_then_final_state_matches_unhooked_run() {
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::ADD_I32,
        opcode::RET,
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::CALL, 0x02, 0x00, 0x02, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];

    // Reference: an ordinary unhooked scan.
    let c = call_container(&scan_bytecode, &[(&func_body, 4, 1, 1)], 3, &[21]);
    let mut rb = VmBuffers::from_container(&c);
    let mut reference = crate::common::load_and_start(&c, &mut rb).unwrap();
    reference.run_round(0).unwrap();
    let ref_vars: Vec<i32> = (0..3)
        .map(|i| reference.read_variable(VarIndex::new(i)).unwrap())
        .collect();
    let ref_data = reference.data_region().to_vec();

    // Hooked: resume one instruction at a time until the scan completes.
    let mut db = VmBuffers::from_container(&c);
    let mut debugged = crate::common::load_and_start(&c, &mut db).unwrap();
    let mut hook = PauseEachInstruction::default();
    let mut rounds = 0;
    loop {
        rounds += 1;
        assert!(rounds < 1000, "pause/resume did not converge");
        match debugged.run_round_debug(0, &mut hook).unwrap() {
            RoundOutcome::Paused(_) => continue,
            RoundOutcome::Completed | RoundOutcome::PausedAfterScan => break,
        }
    }

    assert!(hook.instructions > 0);
    let got_vars: Vec<i32> = (0..3)
        .map(|i| debugged.read_variable(VarIndex::new(i)).unwrap())
        .collect();
    assert_eq!(got_vars, ref_vars);
    assert_eq!(debugged.data_region(), ref_data.as_slice());
}

#[test]
fn run_round_debug_when_trap_then_returns_fault_and_phase_faulted() {
    // A single invalid opcode traps immediately.
    let c = single_function_container(&[0xFF], 0, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let table = BreakpointTable::new();
    let mut hook = DebuggerHook::new(&table);
    let err = vm.run_round_debug(0, &mut hook).unwrap_err();
    assert_eq!(err.trap, ironplc_vm::error::Trap::InvalidInstruction(0xFF));
    assert_eq!(vm.phase(), Phase::Faulted);
}

/// Container whose scan calls a doubling function, with fixed offsets used by
/// the stepping tests: LOAD_CONST\@0, CALL\@3, STORE\@8, RET_VOID\@11; the
/// callee runs LOAD_VAR\@0, LOAD_VAR\@3, ADD\@6, RET\@7.
fn stepping_container() -> ironplc_container::Container {
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::LOAD_VAR_I32, 0x02, 0x00,
        opcode::ADD_I32,
        opcode::RET,
    ];
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,   // @0
        opcode::CALL, 0x02, 0x00, 0x02, 0x00, // @3
        opcode::STORE_VAR_I32, 0x00, 0x00,    // @8
        opcode::RET_VOID,                     // @11
    ];
    call_container(&scan_bytecode, &[(&func_body, 4, 1, 1)], 3, &[21])
}

#[test]
fn run_round_debug_when_step_over_call_then_lands_after_call_in_caller() {
    let c = stepping_container();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let mut table = BreakpointTable::new();
    table.add(FunctionId::SCAN, 3); // the CALL
    let mut hook = DebuggerHook::new(&table);

    // Pause on the CALL.
    assert!(matches!(
        vm.run_round_debug(0, &mut hook).unwrap(),
        RoundOutcome::Paused(PauseReason::Breakpoint(_))
    ));
    assert_eq!(vm.debug_frames().last().unwrap().pc, 3);

    // Step over the call: land on the next instruction in SCAN, not inside
    // the callee.
    hook.step_over();
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Paused(PauseReason::Step));
    let frames = vm.debug_frames();
    assert_eq!(frames.len(), 1, "must be back in the caller only");
    assert_eq!(frames[0].function_id, FunctionId::SCAN);
    assert_eq!(frames[0].pc, 8); // STORE, immediately after the CALL
}

#[test]
fn run_round_debug_when_step_in_call_then_lands_on_first_callee_instruction() {
    let c = stepping_container();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let mut table = BreakpointTable::new();
    table.add(FunctionId::SCAN, 3); // the CALL
    let mut hook = DebuggerHook::new(&table);

    assert!(matches!(
        vm.run_round_debug(0, &mut hook).unwrap(),
        RoundOutcome::Paused(PauseReason::Breakpoint(_))
    ));

    // Step in: descend into the callee, landing on its first instruction.
    hook.step_in();
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Paused(PauseReason::Step));
    let frames = vm.debug_frames();
    assert_eq!(frames.len(), 2, "must have descended one frame");
    assert_eq!(frames[0].function_id, FunctionId::SCAN);
    assert_eq!(frames[1].function_id, FunctionId::new(2));
    assert_eq!(frames[1].pc, 0);
}

#[test]
fn run_round_debug_when_step_out_of_callee_then_lands_in_caller_after_call() {
    let c = stepping_container();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    let func2 = FunctionId::new(2);
    let mut table = BreakpointTable::new();
    table.add(func2, 0); // first instruction inside the callee
    let mut hook = DebuggerHook::new(&table);

    // Pause inside the callee.
    assert!(matches!(
        vm.run_round_debug(0, &mut hook).unwrap(),
        RoundOutcome::Paused(PauseReason::Breakpoint(_))
    ));
    assert_eq!(vm.debug_frames().len(), 2);

    // Step out: run the callee to its return, landing back in SCAN just
    // after the CALL.
    hook.step_out();
    let outcome = vm.run_round_debug(0, &mut hook).unwrap();
    assert_eq!(outcome, RoundOutcome::Paused(PauseReason::Step));
    let frames = vm.debug_frames();
    assert_eq!(frames.len(), 1, "must have returned to the caller");
    assert_eq!(frames[0].function_id, FunctionId::SCAN);
    assert_eq!(frames[0].pc, 8);
}
