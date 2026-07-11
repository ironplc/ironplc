use ironplc_container::{
    CharWidth, ConstantIndex, Container, FbTypeId, FunctionId, InstanceId, TaskId, TaskType,
    VarIndex, STRING_HEADER_BYTES,
};

use crate::buffers::VmBuffers;
use crate::builtin;
use crate::debug::PauseReason;
use crate::debug_hook::{DebugHook, HookAction, NoopDebugHook};
use crate::error::Trap;
use crate::frame_stack::{FbCallReturn, Frame, FrameStack};
#[cfg(feature = "profiling")]
use crate::profile::InstructionProfile;
use crate::scheduler::{ProgramInstanceState, TaskScheduler, TaskState};
use crate::stack::OperandStack;
use crate::string_ops;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use core::fmt::Write as FmtWrite;
use ironplc_container::opcode;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Maximum depth of nested CALL / user-FB_CALL frames before the VM
/// traps with [`Trap::CallStackOverflow`].
///
/// Sized at the embedder side via [`VmBuffers::frames`]. The dispatch
/// loop pushes one [`Frame`] per CALL / user-FB_CALL onto an explicit
/// frame stack and the Rust call stack is no longer consumed
/// proportionally to PLC call depth. The bound here matches what the
/// previous recursion-based check enforced, so no program that runs
/// today will trap that did not before.
pub(crate) const MAX_CALL_DEPTH: u32 = 32;

/// Context for a fault that occurred during task execution.
#[derive(Debug)]
pub struct FaultContext {
    pub trap: Trap,
    pub task_id: TaskId,
    pub instance_id: InstanceId,
}

/// A newly created VM with no loaded program.
///
/// The only valid operation is [`load`](Vm::load), which consumes
/// this value and produces a [`VmReady`].
pub struct Vm;

impl Vm {
    /// Creates a new VM.
    pub fn new() -> Self {
        Vm
    }

    /// Loads a container, using caller-provided buffers for execution state.
    ///
    /// Populates task states and program instances from `container.task_table`.
    /// Consumes the empty VM and returns a ready VM.
    pub fn load<'a>(self, container: &'a Container, bufs: &'a mut VmBuffers) -> VmReady<'a> {
        // Populate task_states from the container's task table.
        for (i, t) in container.task_table.tasks.iter().enumerate() {
            if i < bufs.tasks.len() {
                bufs.tasks[i] = TaskState {
                    task_id: t.task_id,
                    priority: t.priority,
                    task_type: t.task_type,
                    interval_us: t.interval_us,
                    watchdog_us: t.watchdog_us,
                    enabled: (t.flags & 0x01) != 0,
                    next_due_us: 0,
                    scan_count: 0,
                    last_execute_us: 0,
                    max_execute_us: 0,
                    overrun_count: 0,
                };
            }
        }

        // Populate program_instances from the container's task table.
        for (i, p) in container.task_table.programs.iter().enumerate() {
            if i < bufs.programs.len() {
                bufs.programs[i] = ProgramInstanceState {
                    instance_id: p.instance_id,
                    task_id: p.task_id,
                    entry_function_id: p.entry_function_id,
                    var_table_offset: p.var_table_offset,
                    var_table_count: p.var_table_count,
                    init_function_id: p.init_function_id,
                };
            }
        }

        let stack = OperandStack::new(&mut bufs.stack);
        let variables = VariableTable::new(&mut bufs.vars);
        let max_temp_buf_bytes = container.header.max_temp_buf_bytes as usize;

        VmReady {
            container,
            stack,
            variables,
            data_region: &mut bufs.data_region,
            temp_buf: &mut bufs.temp_buf,
            max_temp_buf_bytes,
            task_states: &mut bufs.tasks,
            program_instances: &mut bufs.programs,
            ready_buf: &mut bufs.ready,
            frames: &mut bufs.frames,
            #[cfg(feature = "profiling")]
            profile: InstructionProfile::new(),
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

/// A VM with a loaded program, ready to start execution.
///
/// Call [`start`](VmReady::start) to begin scan execution.
pub struct VmReady<'a> {
    container: &'a Container,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
    data_region: &'a mut [u8],
    temp_buf: &'a mut [u8],
    max_temp_buf_bytes: usize,
    task_states: &'a mut [TaskState],
    program_instances: &'a mut [ProgramInstanceState],
    ready_buf: &'a mut [usize],
    frames: &'a mut [Frame],
    #[cfg(feature = "profiling")]
    profile: InstructionProfile,
}

impl<'a> VmReady<'a> {
    /// Starts the VM for scan execution.
    ///
    /// Executes the init function for each program instance to apply
    /// variable initial values. Returns `Err(FaultContext)` if an init
    /// function traps. On success, returns a running VM ready for scan
    /// cycles.
    ///
    /// Use [`resume`](VmReady::resume) instead when variable buffers
    /// already contain initialized values.
    pub fn start(mut self) -> Result<VmRunning<'a>, FaultContext> {
        let shared_globals_size = self.container.task_table.shared_globals_size;

        // Reject containers whose declared call depth would not fit in
        // the embedder's frame buffer. Codegen populates
        // `max_call_depth` from the static call graph; a value of 0
        // means "not computed" (legacy or hand-built test containers)
        // and disables the check, preserving the prior behavior where
        // the runtime `Trap::CallStackOverflow` is the only signal.
        let declared = self.container.header.max_call_depth;
        let capacity = self.frames.len();
        if declared != 0 && declared as usize > capacity {
            return Err(FaultContext {
                trap: Trap::ProgramExceedsCallDepth {
                    required: declared,
                    capacity: capacity.min(u16::MAX as usize) as u16,
                },
                task_id: TaskId::DEFAULT,
                instance_id: InstanceId::DEFAULT,
            });
        }

        // Execute init functions once before entering scan mode.
        for pi in 0..self.program_instances.len() {
            let init_fn = self.program_instances[pi].init_function_id;
            let instance_id = self.program_instances[pi].instance_id;
            let task_id = self.program_instances[pi].task_id;
            let var_table_offset = self.program_instances[pi].var_table_offset;
            let var_table_count = self.program_instances[pi].var_table_count;

            let scope = VariableScope {
                shared_globals_size,
                instance_offset: var_table_offset,
                instance_count: var_table_count,
            };

            execute(
                self.container,
                &mut self.stack,
                &mut self.variables,
                self.data_region,
                self.temp_buf,
                self.max_temp_buf_bytes,
                self.frames,
                &scope,
                0, // init functions don't need real time
                init_fn,
                #[cfg(feature = "profiling")]
                &mut self.profile,
            )
            .map_err(|trap| FaultContext {
                trap,
                task_id,
                instance_id,
            })?;
        }

        Ok(VmRunning {
            container: self.container,
            stack: self.stack,
            variables: self.variables,
            data_region: self.data_region,
            temp_buf: self.temp_buf,
            max_temp_buf_bytes: self.max_temp_buf_bytes,
            task_states: self.task_states,
            program_instances: self.program_instances,
            ready_buf: self.ready_buf,
            frames: self.frames,
            shared_globals_size,
            scan_count: 0,
            stop_requested: false,
            phase: Phase::Ready,
            debug_frame_count: 0,
            debug_temp_alloc_next: 0,
            #[cfg(feature = "profiling")]
            profile: self.profile,
        })
    }

    /// Resumes execution without running init functions.
    ///
    /// Use this when variable buffers already contain initialized values
    /// (e.g., from a previous session). The `initial_scan_count` sets the
    /// starting scan counter so cycle tracking continues from where it
    /// left off.
    pub fn resume(self, initial_scan_count: u64) -> VmRunning<'a> {
        let shared_globals_size = self.container.task_table.shared_globals_size;
        VmRunning {
            container: self.container,
            stack: self.stack,
            variables: self.variables,
            data_region: self.data_region,
            temp_buf: self.temp_buf,
            max_temp_buf_bytes: self.max_temp_buf_bytes,
            task_states: self.task_states,
            program_instances: self.program_instances,
            ready_buf: self.ready_buf,
            frames: self.frames,
            shared_globals_size,
            scan_count: initial_scan_count,
            stop_requested: false,
            phase: Phase::Ready,
            debug_frame_count: 0,
            debug_temp_alloc_next: 0,
            #[cfg(feature = "profiling")]
            profile: self.profile,
        }
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: VarIndex) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: VarIndex) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }
}

/// Execution phase of the re-entrant debug driver
/// ([`run_round_debug`](VmRunning::run_round_debug)).
///
/// The non-debug [`run_round`](VmRunning::run_round) path never leaves
/// [`Phase::Ready`]; only the debug driver moves through the paused
/// sub-states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// No scan in flight; the next debug round starts fresh.
    Ready,
    /// A scan is in flight (transient; observed only mid-round).
    Running,
    /// Paused before an instruction with the frame stack preserved; the
    /// next debug round resumes the in-flight instance.
    PausedAt(PauseReason),
    /// A scan finished under the debug driver.
    CompletedScan,
    /// A trap ended the round; the VM should transition to [`VmFaulted`].
    Faulted,
}

/// Outcome of one [`run_round_debug`](VmRunning::run_round_debug) call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundOutcome {
    /// The scan completed; call again to run the next scan.
    Completed,
    /// A step spanned the scan boundary and stopped at the start of the
    /// next scan. Reserved for scan-stepping; not produced in the first
    /// debug phase (intra-scan steps report [`RoundOutcome::Paused`]).
    PausedAfterScan,
    /// The VM paused mid-scan; the frame stack is preserved for inspection
    /// and a later resume.
    Paused(PauseReason),
}

/// A VM that is actively executing scan cycles.
///
/// Call [`run_round`](VmRunning::run_round) repeatedly to execute tasks.
/// On a trap, the VM transitions to [`VmFaulted`].
pub struct VmRunning<'a> {
    container: &'a Container,
    stack: OperandStack<'a>,
    variables: VariableTable<'a>,
    data_region: &'a mut [u8],
    temp_buf: &'a mut [u8],
    max_temp_buf_bytes: usize,
    task_states: &'a mut [TaskState],
    program_instances: &'a mut [ProgramInstanceState],
    ready_buf: &'a mut [usize],
    frames: &'a mut [Frame],
    shared_globals_size: u16,
    scan_count: u64,
    stop_requested: bool,
    /// Debug driver phase. `Ready` for the non-debug path.
    phase: Phase,
    /// Live frame count preserved across a debug pause (0 when not mid-scan).
    debug_frame_count: usize,
    /// Temp-buffer allocator bump position preserved across a debug pause.
    debug_temp_alloc_next: u16,
    #[cfg(feature = "profiling")]
    profile: InstructionProfile,
}

impl<'a> VmRunning<'a> {
    /// Executes one scheduling round: collects ready tasks, executes them
    /// in priority order, and updates timing.
    ///
    /// The caller provides `current_time_us` (microseconds since VM start).
    /// Sleep logic is the caller's responsibility.
    ///
    /// Returns `Ok(())` if the round completes. Returns `Err(FaultContext)` if
    /// a trap occurs during execution. The caller should transition to
    /// `VmFaulted` on trap.
    pub fn run_round(&mut self, current_time_us: u64) -> Result<(), FaultContext> {
        // Build a scheduler temporarily borrowing task_states.
        // We need to collect ready task indices into ready_buf, then drop the scheduler
        // before iterating, so we can mutably borrow task_states during record_execution.
        let ready_count;
        {
            let scheduler = TaskScheduler::new(self.task_states);
            let ready = scheduler.collect_ready_tasks(current_time_us, self.ready_buf);
            ready_count = ready.len();
        }

        if ready_count == 0 {
            return Ok(());
        }

        // System variable injection: write monotonic uptime before task execution.
        self.inject_system_uptime(current_time_us);

        // Stub: INPUT_FREEZE (no-op)

        for ri in 0..ready_count {
            let task_idx = self.ready_buf[ri];
            let task_id = self.task_states[task_idx].task_id;

            #[cfg(not(target_arch = "wasm32"))]
            let start = Instant::now();
            let mut last_instance_id = InstanceId::DEFAULT;

            // Iterate over program instances for this task.
            for pi in 0..self.program_instances.len() {
                if self.program_instances[pi].task_id != task_id {
                    continue;
                }
                let instance_id = self.program_instances[pi].instance_id;
                last_instance_id = instance_id;

                // Production scan: run the instance to completion with the
                // zero-cost hook and fresh (non-resumable) frame state.
                let (outcome, _, _) = self
                    .run_instance(pi, current_time_us, 0, 0, &mut NoopDebugHook)
                    .map_err(|trap| FaultContext {
                        trap,
                        task_id,
                        instance_id,
                    })?;
                debug_assert!(
                    matches!(outcome, ExecuteOutcome::Completed),
                    "NoopDebugHook can never pause a scan"
                );
            }

            #[cfg(not(target_arch = "wasm32"))]
            let elapsed_us = start.elapsed().as_micros() as u64;
            #[cfg(target_arch = "wasm32")]
            let elapsed_us = 0u64;

            // Watchdog check: if the task has a watchdog configured and
            // execution exceeded the timeout, trap.
            let watchdog_us = self.task_states[task_idx].watchdog_us;
            if watchdog_us > 0 && elapsed_us > watchdog_us {
                return Err(FaultContext {
                    trap: Trap::WatchdogTimeout(task_id),
                    task_id,
                    instance_id: last_instance_id,
                });
            }

            let mut scheduler = TaskScheduler::new(self.task_states);
            scheduler.record_execution(task_idx, elapsed_us, current_time_us);
        }

        // Stub: OUTPUT_FLUSH (no-op)

        self.scan_count += 1;
        Ok(())
    }

    /// Re-entrant debug variant of [`run_round`](Self::run_round).
    ///
    /// Runs the single program instance (v1 is single-instance; the Phase 4
    /// DAP launch enforces exactly one instance) with `hook` observing every
    /// instruction and call/return. Unlike `run_round`, this can stop
    /// mid-scan: when the hook returns [`HookAction::Pause`], the frame
    /// stack, operand stack, and temp-buffer position are preserved and the
    /// method returns [`RoundOutcome::Paused`]. Calling it again resumes the
    /// paused instance from exactly where it stopped.
    ///
    /// A trap still surfaces through the fault path: the method returns
    /// `Err(FaultContext)` and sets the phase to [`Phase::Faulted`], exactly
    /// as `run_round` does.
    ///
    /// **Intentionally bypasses the scheduler and watchdog.** `run_round`
    /// consults the [`TaskScheduler`] to pick ready tasks, records execution
    /// time to re-arm cyclic timers, and traps on watchdog overrun. This
    /// method does none of that: it runs instance 0 unconditionally and never
    /// times the scan. That is deliberate — while a human controls the clock
    /// at a breakpoint, cyclic re-arming is meaningless and a watchdog would
    /// fire the moment execution paused. The shared per-instance execution
    /// core lives in [`run_instance`](Self::run_instance); only the
    /// scheduling/lifecycle policy around it differs between the two drivers.
    pub fn run_round_debug<H: DebugHook>(
        &mut self,
        current_time_us: u64,
        hook: &mut H,
    ) -> Result<RoundOutcome, FaultContext> {
        // No program instance → nothing to debug; the scan is a no-op.
        if self.program_instances.is_empty() {
            self.phase = Phase::CompletedScan;
            return Ok(RoundOutcome::Completed);
        }

        let resuming = matches!(self.phase, Phase::PausedAt(_));

        // Instance 0 is the single debuggee (v1 is single-instance).
        let instance_id = self.program_instances[0].instance_id;
        let task_id = self.program_instances[0].task_id;

        if !resuming {
            // Fresh scan: inject system uptime, then reset the resume state so
            // run_instance starts with an empty frame stack (the dispatch loop
            // pushes the entry frame). When resuming, the preserved frame count
            // is non-zero and the paused frames survive in place.
            self.inject_system_uptime(current_time_us);
            self.debug_frame_count = 0;
            self.debug_temp_alloc_next = 0;
        }

        self.phase = Phase::Running;

        let frame_count_in = self.debug_frame_count;
        let temp_alloc_next_in = self.debug_temp_alloc_next;
        let (outcome, frame_count, temp_alloc_next) = self
            .run_instance(0, current_time_us, frame_count_in, temp_alloc_next_in, hook)
            .map_err(|trap| {
                self.phase = Phase::Faulted;
                FaultContext {
                    trap,
                    task_id,
                    instance_id,
                }
            })?;
        self.debug_frame_count = frame_count;
        self.debug_temp_alloc_next = temp_alloc_next;

        match outcome {
            ExecuteOutcome::Paused(reason) => {
                self.phase = Phase::PausedAt(reason);
                Ok(RoundOutcome::Paused(reason))
            }
            ExecuteOutcome::Completed => {
                self.scan_count += 1;
                self.phase = Phase::CompletedScan;
                Ok(RoundOutcome::Completed)
            }
        }
    }

    /// Writes the monotonic uptime system variables before task execution,
    /// if the loaded container declares them. Shared by [`run_round`] and
    /// [`run_round_debug`] so the injection lives in exactly one place.
    fn inject_system_uptime(&mut self, current_time_us: u64) {
        if self.container.header.flags & ironplc_container::FLAG_HAS_SYSTEM_UPTIME == 0 {
            return;
        }
        let time_ms = (current_time_us / 1000) as i64;
        // __SYSTEM_UP_TIME at VarIndex(0): i32 milliseconds (wrapping)
        self.variables
            .store(VarIndex::new(0), Slot::from_i32(time_ms as i32))
            .expect("system uptime variable must exist at index 0");
        // __SYSTEM_UP_LTIME at VarIndex(1): i64 milliseconds (non-wrapping)
        self.variables
            .store(VarIndex::new(1), Slot::from_i64(time_ms))
            .expect("system uptime variable must exist at index 1");
    }

    /// Runs one program instance's entry function through the shared dispatch
    /// loop until it returns (`Completed`) or the hook pauses it (`Paused`).
    ///
    /// This is the single execution core behind both drivers:
    /// - [`run_round`](Self::run_round) calls it per ready instance with
    ///   [`NoopDebugHook`] and fresh `0` resume state (a production scan is
    ///   never resumable, so the returned frame state is discarded);
    /// - [`run_round_debug`](Self::run_round_debug) calls it once with a real
    ///   hook and the *persisted* resume state, so a paused instance can
    ///   continue on the next call.
    ///
    /// The caller owns all scheduling, watchdog, and instance-selection
    /// policy; this method only builds the instance's [`VariableScope`] and
    /// runs it. `frame_count` / `temp_alloc_next` are the resume state on
    /// entry; the returned pair is the state after this run (advanced only
    /// when the run paused mid-instance).
    fn run_instance<H: DebugHook>(
        &mut self,
        instance_index: usize,
        current_time_us: u64,
        frame_count: usize,
        temp_alloc_next: u16,
        hook: &mut H,
    ) -> Result<(ExecuteOutcome, usize, u16), Trap> {
        let entry_function_id = self.program_instances[instance_index].entry_function_id;
        let var_table_offset = self.program_instances[instance_index].var_table_offset;
        let var_table_count = self.program_instances[instance_index].var_table_count;

        let scope = VariableScope {
            shared_globals_size: self.shared_globals_size,
            instance_offset: var_table_offset,
            instance_count: var_table_count,
        };

        let mut frame_count = frame_count;
        let mut temp_alloc_next = temp_alloc_next;
        let outcome = execute_with_hook(
            self.container,
            &mut self.stack,
            &mut self.variables,
            self.data_region,
            self.temp_buf,
            self.max_temp_buf_bytes,
            self.frames,
            &scope,
            current_time_us,
            entry_function_id,
            &mut frame_count,
            &mut temp_alloc_next,
            #[cfg(feature = "profiling")]
            &mut self.profile,
            hook,
        )?;
        Ok((outcome, frame_count, temp_alloc_next))
    }

    /// Current debug-driver phase (see [`Phase`]).
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The live call frames of a paused instance, outermost first.
    ///
    /// Empty unless the VM is paused ([`Phase::PausedAt`]). Lets a debugger
    /// walk the stack — each [`Frame`] carries its `function_id` and `pc`
    /// so `(function_id, pc)` pairs can be resolved against debug info.
    pub fn debug_frames(&self) -> &[Frame] {
        &self.frames[..self.debug_frame_count]
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: VarIndex) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: VarIndex) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }

    /// Reads a variable value as an i64.
    pub fn read_variable_i64(&self, index: VarIndex) -> Result<i64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i64())
    }

    /// Writes a variable value as an i32.
    pub fn write_variable(&mut self, index: VarIndex, value: i32) -> Result<(), Trap> {
        self.variables.store(index, Slot::from_i32(value))
    }

    /// Returns a reference to the data region.
    pub fn data_region(&self) -> &[u8] {
        self.data_region
    }

    /// Returns the number of variable slots in the loaded container.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }

    /// Returns the number of completed scan cycles.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Returns the earliest `next_due_us` across all enabled cyclic tasks,
    /// or `None` if no cyclic tasks exist (e.g. freewheeling only).
    pub fn next_due_us(&self) -> Option<u64> {
        self.task_states
            .iter()
            .filter(|t| t.enabled && t.task_type == TaskType::Cyclic)
            .map(|t| t.next_due_us)
            .min()
    }

    /// Requests the VM to stop after the current round.
    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    /// Returns true if a stop has been requested.
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// Returns a reference to the instruction profile.
    #[cfg(feature = "profiling")]
    pub fn profile(&self) -> &InstructionProfile {
        &self.profile
    }

    /// Transitions to the stopped state (clean shutdown).
    pub fn stop(self) -> VmStopped<'a> {
        VmStopped {
            variables: self.variables,
            scan_count: self.scan_count,
            #[cfg(feature = "profiling")]
            profile: self.profile,
        }
    }

    /// Transitions to the faulted state (trap occurred).
    pub fn fault(self, ctx: FaultContext) -> VmFaulted<'a> {
        VmFaulted {
            trap: ctx.trap,
            task_id: ctx.task_id,
            instance_id: ctx.instance_id,
            variables: self.variables,
            #[cfg(feature = "profiling")]
            profile: self.profile,
        }
    }
}

/// A VM that has been cleanly stopped.
pub struct VmStopped<'a> {
    variables: VariableTable<'a>,
    scan_count: u64,
    #[cfg(feature = "profiling")]
    profile: InstructionProfile,
}

impl<'a> VmStopped<'a> {
    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: VarIndex) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: VarIndex) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }

    /// Returns the total number of completed scheduling rounds.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Returns a reference to the instruction profile.
    #[cfg(feature = "profiling")]
    pub fn profile(&self) -> &InstructionProfile {
        &self.profile
    }
}

/// A VM that has stopped due to a trap.
pub struct VmFaulted<'a> {
    trap: Trap,
    task_id: TaskId,
    instance_id: InstanceId,
    variables: VariableTable<'a>,
    #[cfg(feature = "profiling")]
    profile: InstructionProfile,
}

impl<'a> VmFaulted<'a> {
    /// Returns the trap that caused the fault.
    pub fn trap(&self) -> &Trap {
        &self.trap
    }

    /// Returns the task that was executing when the trap occurred.
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    /// Returns the program instance that was executing when the trap occurred.
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: VarIndex) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: VarIndex) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }

    /// Returns a reference to the instruction profile.
    #[cfg(feature = "profiling")]
    pub fn profile(&self) -> &InstructionProfile {
        &self.profile
    }
}

/// Binary operation: pop b then a, compute result, push.
macro_rules! binop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $a:ident, $b:ident, $result:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Comparison: pop b then a, compare, push i32 boolean.
macro_rules! cmpop {
    ($stack:expr, $as_ty:ident, $a:ident, $b:ident, $cond:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::from_i32(if $cond { 1 } else { 0 }))?;
    }};
}

/// Unary operation: pop one, compute, push.
macro_rules! unaryop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $a:ident, $result:expr) => {{
        let $a = $stack.pop()?.$as_ty();
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Checked division: pop b then a, check b != zero, compute, push.
macro_rules! checked_divop {
    ($stack:expr, $as_ty:ident, $from_ty:ident, $zero:expr, $a:ident, $b:ident, $result:expr) => {{
        let $b = $stack.pop()?.$as_ty();
        let $a = $stack.pop()?.$as_ty();
        if $b == $zero {
            return Err(Trap::DivideByZero);
        }
        $stack.push(Slot::$from_ty($result))?;
    }};
}

/// Load constant from pool: read index, look up, push.
macro_rules! load_const {
    ($bytecode:expr, $pc:expr, $container:expr, $stack:expr, $get:ident, $from:ident) => {{
        let index = read_u16_le($bytecode, &mut $pc)?;
        let value = $container
            .constant_pool
            .$get(ConstantIndex::new(index))
            .map_err(|_| Trap::InvalidConstantIndex(ConstantIndex::new(index)))?;
        $stack.push(Slot::$from(value))?;
    }};
}

/// Executes the given entry function until it returns (the frame stack
/// drains) or a trap, using a no-op debug hook.
///
/// This is a thin wrapper around [`execute_with_hook`] that supplies a
/// [`NoopDebugHook`]. Existing call sites use this entry point so that
/// the debug-hook plumbing imposes no overhead on VMs that do not need
/// instruction-level callbacks (the noop hook is a ZST and inlines away).
#[allow(clippy::too_many_arguments)]
fn execute(
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    frames: &mut [Frame],
    entry_scope: &VariableScope,
    current_time_us: u64,
    entry_function_id: FunctionId,
    #[cfg(feature = "profiling")] profile: &mut InstructionProfile,
) -> Result<(), Trap> {
    let mut hook = NoopDebugHook;
    // Fresh, non-resumable run: the frame stack starts empty (so the entry
    // frame is pushed) and no temp-buffer allocations carry over. The noop
    // hook can never pause, so `Completed` is the only reachable outcome.
    let mut frame_count = 0usize;
    let mut temp_alloc_next = 0u16;
    match execute_with_hook(
        container,
        stack,
        variables,
        data_region,
        temp_buf,
        max_temp_buf_bytes,
        frames,
        entry_scope,
        current_time_us,
        entry_function_id,
        &mut frame_count,
        &mut temp_alloc_next,
        #[cfg(feature = "profiling")]
        profile,
        &mut hook,
    )? {
        ExecuteOutcome::Completed => Ok(()),
        ExecuteOutcome::Paused(_) => {
            unreachable!("NoopDebugHook always returns HookAction::Continue")
        }
    }
}

/// Outcome of an [`execute_with_hook`] invocation.
///
/// `Completed` means the entry function's frame stack drained (the program
/// returned). `Paused` means the debug hook requested a stop before an
/// instruction executed; the frame stack, operand stack, and temp-buffer
/// allocator position are all preserved so a later call can resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecuteOutcome {
    /// The program returned normally.
    Completed,
    /// The hook paused execution before an instruction.
    Paused(PauseReason),
}

/// Executes the entry function until its frame stack drains (program
/// returns) or a trap, invoking `hook.before_instruction` before each
/// opcode.
///
/// The dispatch loop is iterative: a single [`FrameStack`] tracks every
/// in-flight PLC `CALL` / user-`FB_CALL`, so the Rust call stack is not
/// consumed proportionally to PLC call depth and an instruction-level
/// debugger can be added without restructuring the loop again.
///
/// `entry_function_id` identifies the topmost function pushed onto the
/// frame stack; subsequent CALLs push deeper frames and each frame
/// carries its own function id so `hook.before_instruction` can resolve
/// `(function_id, pc)` pairs across nested frames.
///
/// It is generic over the hook type so that the noop hook monomorphizes
/// to identical code as before; only callers that supply a real hook
/// pay any runtime cost.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_with_hook<H: DebugHook>(
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    frames: &mut [Frame],
    entry_scope: &VariableScope,
    current_time_us: u64,
    entry_function_id: FunctionId,
    frame_count: &mut usize,
    temp_alloc_next: &mut u16,
    #[cfg(feature = "profiling")] profile: &mut InstructionProfile,
    hook: &mut H,
) -> Result<ExecuteOutcome, Trap> {
    let mut temp_alloc = string_ops::TempBufAllocator::new(max_temp_buf_bytes);
    // Restore the temp-buffer bump position captured at the last pause. On a
    // fresh run this is 0 (nothing allocated yet).
    temp_alloc.rewind_to(*temp_alloc_next);
    let mut frame_stack = FrameStack::resume(frames, *frame_count);
    if frame_stack.is_empty() {
        // Fresh run: push the entry frame. When resuming a paused instance
        // the frame stack is non-empty and the entry frame (plus any deeper
        // frames) already survive in the backing slice.
        frame_stack.push(Frame {
            function_id: entry_function_id,
            pc: 0,
            scope: *entry_scope,
            temp_alloc_mark: 0,
            fb_return: None,
        })?;
    }

    while !frame_stack.is_empty() {
        // Snapshot the top frame's mutable state for this iteration.
        // `pc` is the local working copy; we write it back to the frame
        // at the end of the iteration unless dispatch pushed or popped
        // a frame (in which case the frame stack top changed mid-arm
        // and `pc` is not relevant to the new top).
        let (current_function_id, scope, mut pc) = {
            let top = frame_stack.top().expect("non-empty by loop condition");
            (top.function_id, top.scope, top.pc)
        };
        let bytecode = container
            .code
            .get_function_bytecode(current_function_id)
            .ok_or(Trap::InvalidFunctionId(current_function_id))?;

        if pc >= bytecode.len() {
            // Fell off the end of a function body without an explicit
            // RET — treat as RET_VOID.
            handle_frame_return(
                &mut temp_alloc,
                &mut frame_stack,
                data_region,
                variables,
                hook,
            )?;
            continue;
        }

        let op = bytecode[pc];
        // Notify the debug hook before advancing pc so the hook sees the
        // offset of the opcode itself, not its operand bytes. With
        // NoopDebugHook this call is inlined away to nothing.
        match hook.before_instruction(current_function_id, pc, op) {
            HookAction::Continue => {}
            HookAction::Pause(reason) => {
                // Write pc back to the top frame WITHOUT advancing, so the
                // paused instruction re-executes on resume. The frame stack
                // and temp-allocator position are preserved so the caller
                // can resume this instance later.
                frame_stack
                    .top_mut()
                    .expect("non-empty by loop condition")
                    .pc = pc;
                *frame_count = frame_stack.len();
                *temp_alloc_next = temp_alloc.next();
                return Ok(ExecuteOutcome::Paused(reason));
            }
        }
        pc += 1;

        #[cfg(feature = "profiling")]
        profile.record(op);

        // Set to false in arms that push or pop a frame; the post-match
        // pc write-back is skipped in those cases so we never clobber a
        // newly-pushed callee's `pc: 0` or a freshly-popped caller's
        // already-correct `pc`.
        let mut advance_pc = true;

        match op {
            // --- Load constants ---
            opcode::LOAD_CONST_I32 => {
                load_const!(bytecode, pc, container, stack, get_i32, from_i32)
            }
            opcode::LOAD_CONST_I64 => {
                load_const!(bytecode, pc, container, stack, get_i64, from_i64)
            }
            opcode::LOAD_CONST_F32 => {
                load_const!(bytecode, pc, container, stack, get_f32, from_f32)
            }
            opcode::LOAD_CONST_F64 => {
                load_const!(bytecode, pc, container, stack, get_f64, from_f64)
            }
            opcode::LOAD_TRUE => {
                stack.push(Slot::from_i32(1))?;
            }
            opcode::LOAD_FALSE => {
                stack.push(Slot::from_i32(0))?;
            }
            // --- Load/store variables (type-erased slots) ---
            opcode::LOAD_VAR_I32
            | opcode::LOAD_VAR_I64
            | opcode::LOAD_VAR_F32
            | opcode::LOAD_VAR_F64 => {
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(index)?;
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32
            | opcode::STORE_VAR_I64
            | opcode::STORE_VAR_F32
            | opcode::STORE_VAR_F64 => {
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(index)?;
                let slot = stack.pop()?;
                variables.store(index, slot)?;
            }
            // --- Indirect load/store (reference dereference) ---
            opcode::LOAD_INDIRECT => {
                let ref_slot = stack.pop()?;
                if ref_slot.is_null_ref() {
                    return Err(Trap::NullDereference);
                }
                let target_index = ref_slot
                    .as_var_index()
                    .ok_or(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))?;
                scope.check_access(target_index)?;
                let value = variables.load(target_index)?;
                stack.push(value)?;
            }
            opcode::STORE_INDIRECT => {
                let ref_slot = stack.pop()?;
                if ref_slot.is_null_ref() {
                    return Err(Trap::NullDereference);
                }
                let target_index = ref_slot
                    .as_var_index()
                    .ok_or(Trap::InvalidVariableIndex(VarIndex::new(u16::MAX)))?;
                scope.check_access(target_index)?;
                let value = stack.pop()?;
                variables.store(target_index, value)?;
            }
            // --- Integer arithmetic (wrapping) ---
            opcode::ADD_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_add(b)),
            opcode::SUB_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_sub(b)),
            opcode::MUL_I32 => binop!(stack, as_i32, from_i32, a, b, a.wrapping_mul(b)),
            opcode::ADD_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_add(b)),
            opcode::SUB_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_sub(b)),
            opcode::MUL_I64 => binop!(stack, as_i64, from_i64, a, b, a.wrapping_mul(b)),
            // --- Integer division (checked for zero) ---
            opcode::DIV_I32 => {
                checked_divop!(stack, as_i32, from_i32, 0i32, a, b, a.wrapping_div(b))
            }
            opcode::MOD_I32 => {
                checked_divop!(stack, as_i32, from_i32, 0i32, a, b, a.wrapping_rem(b))
            }
            opcode::DIV_I64 => {
                checked_divop!(stack, as_i64, from_i64, 0i64, a, b, a.wrapping_div(b))
            }
            opcode::MOD_I64 => {
                checked_divop!(stack, as_i64, from_i64, 0i64, a, b, a.wrapping_rem(b))
            }
            // --- Unsigned integer division (checked for zero) ---
            opcode::DIV_U32 => checked_divop!(
                stack,
                as_i32,
                from_i32,
                0i32,
                a,
                b,
                ((a as u32) / (b as u32)) as i32
            ),
            opcode::MOD_U32 => checked_divop!(
                stack,
                as_i32,
                from_i32,
                0i32,
                a,
                b,
                ((a as u32) % (b as u32)) as i32
            ),
            opcode::DIV_U64 => checked_divop!(
                stack,
                as_i64,
                from_i64,
                0i64,
                a,
                b,
                ((a as u64) / (b as u64)) as i64
            ),
            opcode::MOD_U64 => checked_divop!(
                stack,
                as_i64,
                from_i64,
                0i64,
                a,
                b,
                ((a as u64) % (b as u64)) as i64
            ),
            // --- Float arithmetic ---
            opcode::ADD_F32 => binop!(stack, as_f32, from_f32, a, b, a + b),
            opcode::SUB_F32 => binop!(stack, as_f32, from_f32, a, b, a - b),
            opcode::MUL_F32 => binop!(stack, as_f32, from_f32, a, b, a * b),
            opcode::DIV_F32 => binop!(stack, as_f32, from_f32, a, b, a / b),
            opcode::ADD_F64 => binop!(stack, as_f64, from_f64, a, b, a + b),
            opcode::SUB_F64 => binop!(stack, as_f64, from_f64, a, b, a - b),
            opcode::MUL_F64 => binop!(stack, as_f64, from_f64, a, b, a * b),
            opcode::DIV_F64 => binop!(stack, as_f64, from_f64, a, b, a / b),
            // --- Negation ---
            opcode::NEG_I32 => unaryop!(stack, as_i32, from_i32, a, a.wrapping_neg()),
            opcode::NEG_I64 => unaryop!(stack, as_i64, from_i64, a, a.wrapping_neg()),
            opcode::NEG_F32 => unaryop!(stack, as_f32, from_f32, a, -a),
            opcode::NEG_F64 => unaryop!(stack, as_f64, from_f64, a, -a),
            // --- Truncation ---
            opcode::TRUNC_I8 => unaryop!(stack, as_i32, from_i32, a, (a as i8) as i32),
            opcode::TRUNC_U8 => unaryop!(stack, as_i32, from_i32, a, (a as u8) as i32),
            opcode::TRUNC_I16 => unaryop!(stack, as_i32, from_i32, a, (a as i16) as i32),
            opcode::TRUNC_U16 => unaryop!(stack, as_i32, from_i32, a, (a as u16) as i32),
            // --- Signed comparison ---
            opcode::EQ_I32 => cmpop!(stack, as_i32, a, b, a == b),
            opcode::NE_I32 => cmpop!(stack, as_i32, a, b, a != b),
            opcode::LT_I32 => cmpop!(stack, as_i32, a, b, a < b),
            opcode::LE_I32 => cmpop!(stack, as_i32, a, b, a <= b),
            opcode::GT_I32 => cmpop!(stack, as_i32, a, b, a > b),
            opcode::GE_I32 => cmpop!(stack, as_i32, a, b, a >= b),
            opcode::EQ_I64 => cmpop!(stack, as_i64, a, b, a == b),
            opcode::NE_I64 => cmpop!(stack, as_i64, a, b, a != b),
            opcode::LT_I64 => cmpop!(stack, as_i64, a, b, a < b),
            opcode::LE_I64 => cmpop!(stack, as_i64, a, b, a <= b),
            opcode::GT_I64 => cmpop!(stack, as_i64, a, b, a > b),
            opcode::GE_I64 => cmpop!(stack, as_i64, a, b, a >= b),
            // --- Unsigned comparison ---
            opcode::LT_U32 => cmpop!(stack, as_i32, a, b, (a as u32) < (b as u32)),
            opcode::LE_U32 => cmpop!(stack, as_i32, a, b, (a as u32) <= (b as u32)),
            opcode::GT_U32 => cmpop!(stack, as_i32, a, b, (a as u32) > (b as u32)),
            opcode::GE_U32 => cmpop!(stack, as_i32, a, b, (a as u32) >= (b as u32)),
            opcode::LT_U64 => cmpop!(stack, as_i64, a, b, (a as u64) < (b as u64)),
            opcode::LE_U64 => cmpop!(stack, as_i64, a, b, (a as u64) <= (b as u64)),
            opcode::GT_U64 => cmpop!(stack, as_i64, a, b, (a as u64) > (b as u64)),
            opcode::GE_U64 => cmpop!(stack, as_i64, a, b, (a as u64) >= (b as u64)),
            // --- Float comparison ---
            opcode::EQ_F32 => cmpop!(stack, as_f32, a, b, a == b),
            opcode::NE_F32 => cmpop!(stack, as_f32, a, b, a != b),
            opcode::LT_F32 => cmpop!(stack, as_f32, a, b, a < b),
            opcode::LE_F32 => cmpop!(stack, as_f32, a, b, a <= b),
            opcode::GT_F32 => cmpop!(stack, as_f32, a, b, a > b),
            opcode::GE_F32 => cmpop!(stack, as_f32, a, b, a >= b),
            opcode::EQ_F64 => cmpop!(stack, as_f64, a, b, a == b),
            opcode::NE_F64 => cmpop!(stack, as_f64, a, b, a != b),
            opcode::LT_F64 => cmpop!(stack, as_f64, a, b, a < b),
            opcode::LE_F64 => cmpop!(stack, as_f64, a, b, a <= b),
            opcode::GT_F64 => cmpop!(stack, as_f64, a, b, a > b),
            opcode::GE_F64 => cmpop!(stack, as_f64, a, b, a >= b),
            // --- Boolean logic ---
            opcode::BOOL_AND => cmpop!(stack, as_i32, a, b, (a != 0) && (b != 0)),
            opcode::BOOL_OR => cmpop!(stack, as_i32, a, b, (a != 0) || (b != 0)),
            opcode::BOOL_XOR => cmpop!(stack, as_i32, a, b, (a != 0) != (b != 0)),
            opcode::BOOL_NOT => unaryop!(stack, as_i32, from_i32, a, if a == 0 { 1 } else { 0 }),
            // --- Bitwise (32-bit) ---
            opcode::BIT_AND_32 => binop!(stack, as_i32, from_i32, a, b, a & b),
            opcode::BIT_OR_32 => binop!(stack, as_i32, from_i32, a, b, a | b),
            opcode::BIT_XOR_32 => binop!(stack, as_i32, from_i32, a, b, a ^ b),
            opcode::BIT_NOT_32 => unaryop!(stack, as_i32, from_i32, a, !a),
            // --- Bitwise (64-bit) ---
            opcode::BIT_AND_64 => binop!(stack, as_i64, from_i64, a, b, a & b),
            opcode::BIT_OR_64 => binop!(stack, as_i64, from_i64, a, b, a | b),
            opcode::BIT_XOR_64 => binop!(stack, as_i64, from_i64, a, b, a ^ b),
            opcode::BIT_NOT_64 => unaryop!(stack, as_i64, from_i64, a, !a),
            // --- Control flow ---
            opcode::JMP => {
                let offset = read_i16_le(bytecode, &mut pc)?;
                pc = (pc as isize + offset as isize) as usize;
            }
            opcode::JMP_IF_NOT => {
                let offset = read_i16_le(bytecode, &mut pc)?;
                let cond = stack.pop()?.as_i32();
                if cond == 0 {
                    pc = (pc as isize + offset as isize) as usize;
                }
            }
            opcode::CMP_BR_I32 | opcode::CMP_BR_I64 => {
                let cmp_op_byte = read_u8(bytecode, &mut pc)?;
                let var_idx = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let const_idx = ConstantIndex::new(read_u16_le(bytecode, &mut pc)?);
                let offset = read_i16_le(bytecode, &mut pc)?;
                scope.check_access(var_idx)?;
                let truth = if op == opcode::CMP_BR_I32 {
                    let cur = variables.load(var_idx)?.as_i32();
                    let cnst = container
                        .constant_pool
                        .get_i32(const_idx)
                        .map_err(|_| Trap::InvalidConstantIndex(const_idx))?;
                    match cmp_op_byte {
                        opcode::cmp_op::EQ => cur == cnst,
                        opcode::cmp_op::NE => cur != cnst,
                        opcode::cmp_op::LT_S => cur < cnst,
                        opcode::cmp_op::LE_S => cur <= cnst,
                        opcode::cmp_op::GT_S => cur > cnst,
                        opcode::cmp_op::GE_S => cur >= cnst,
                        _ => return Err(Trap::InvalidCmpOp(cmp_op_byte)),
                    }
                } else {
                    let cur = variables.load(var_idx)?.as_i64();
                    let cnst = container
                        .constant_pool
                        .get_i64(const_idx)
                        .map_err(|_| Trap::InvalidConstantIndex(const_idx))?;
                    match cmp_op_byte {
                        opcode::cmp_op::EQ => cur == cnst,
                        opcode::cmp_op::NE => cur != cnst,
                        opcode::cmp_op::LT_S => cur < cnst,
                        opcode::cmp_op::LE_S => cur <= cnst,
                        opcode::cmp_op::GT_S => cur > cnst,
                        opcode::cmp_op::GE_S => cur >= cnst,
                        _ => return Err(Trap::InvalidCmpOp(cmp_op_byte)),
                    }
                };
                if truth {
                    pc = (pc as isize + offset as isize) as usize;
                }
            }
            opcode::BUILTIN => {
                let func_id = read_u16_le(bytecode, &mut pc)?;
                match func_id {
                    // --- Numeric ↔ STRING conversions ---
                    //
                    // These are handled inline (not via builtin::dispatch)
                    // because they need access to temp_buf and data_region.
                    opcode::builtin::CONV_I32_TO_STR => {
                        let val = stack.pop()?.as_i32();
                        let mut fmt_buf = StackFmtBuf::new();
                        let _ = write!(fmt_buf, "{}", val);
                        let bytes = fmt_buf.as_bytes();
                        // Numeric formatting always produces ASCII (narrow).
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len(), CharWidth::Narrow)?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(
                            temp_buf,
                            buf_start,
                            max_len,
                            cur_len,
                            CharWidth::Narrow,
                        );
                        temp_buf[buf_start + STRING_HEADER_BYTES
                            ..buf_start + STRING_HEADER_BYTES + cur_len as usize]
                            .copy_from_slice(&bytes[..cur_len as usize]);
                        stack.push(Slot::from_i32(buf_idx as i32))?;
                    }
                    opcode::builtin::CONV_U32_TO_STR => {
                        let val = stack.pop()?.as_i32() as u32;
                        let mut fmt_buf = StackFmtBuf::new();
                        let _ = write!(fmt_buf, "{}", val);
                        let bytes = fmt_buf.as_bytes();
                        // Numeric formatting always produces ASCII (narrow).
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len(), CharWidth::Narrow)?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(
                            temp_buf,
                            buf_start,
                            max_len,
                            cur_len,
                            CharWidth::Narrow,
                        );
                        temp_buf[buf_start + STRING_HEADER_BYTES
                            ..buf_start + STRING_HEADER_BYTES + cur_len as usize]
                            .copy_from_slice(&bytes[..cur_len as usize]);
                        stack.push(Slot::from_i32(buf_idx as i32))?;
                    }
                    opcode::builtin::CONV_F32_TO_STR => {
                        let val = stack.pop()?.as_f32();
                        let mut fmt_buf = StackFmtBuf::new();
                        let _ = write!(fmt_buf, "{}", val);
                        let bytes = fmt_buf.as_bytes();
                        // Numeric formatting always produces ASCII (narrow).
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len(), CharWidth::Narrow)?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(
                            temp_buf,
                            buf_start,
                            max_len,
                            cur_len,
                            CharWidth::Narrow,
                        );
                        temp_buf[buf_start + STRING_HEADER_BYTES
                            ..buf_start + STRING_HEADER_BYTES + cur_len as usize]
                            .copy_from_slice(&bytes[..cur_len as usize]);
                        stack.push(Slot::from_i32(buf_idx as i32))?;
                    }
                    opcode::builtin::CONV_STR_TO_I32 => {
                        let data_offset = stack.pop()?.as_i32() as usize;
                        if data_offset + STRING_HEADER_BYTES > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                        }
                        // STRING_TO_* parses Latin-1 digits; reject WSTRING input.
                        let width = string_ops::str_read_char_width(data_region, data_offset)?;
                        string_ops::verify_encoding(CharWidth::Narrow, width)?;
                        let cur_len =
                            string_ops::str_read_cur_len(data_region, data_offset) as usize;
                        let start = data_offset + STRING_HEADER_BYTES;
                        let end = (start + cur_len).min(data_region.len());
                        let result = core::str::from_utf8(&data_region[start..end])
                            .ok()
                            .and_then(|s| s.trim().parse::<i32>().ok())
                            .unwrap_or(0);
                        stack.push(Slot::from_i32(result))?;
                    }
                    opcode::builtin::CONV_STR_TO_F32 => {
                        let data_offset = stack.pop()?.as_i32() as usize;
                        if data_offset + STRING_HEADER_BYTES > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                        }
                        // STRING_TO_* parses Latin-1 digits; reject WSTRING input.
                        let width = string_ops::str_read_char_width(data_region, data_offset)?;
                        string_ops::verify_encoding(CharWidth::Narrow, width)?;
                        let cur_len =
                            string_ops::str_read_cur_len(data_region, data_offset) as usize;
                        let start = data_offset + STRING_HEADER_BYTES;
                        let end = (start + cur_len).min(data_region.len());
                        let result = core::str::from_utf8(&data_region[start..end])
                            .ok()
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .unwrap_or(0.0);
                        stack.push(Slot::from_f32(result))?;
                    }
                    opcode::builtin::CMP_STR => {
                        let right_offset = stack.pop()?.as_i32() as usize;
                        let left_offset = stack.pop()?.as_i32() as usize;

                        if left_offset + STRING_HEADER_BYTES > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(left_offset as u32));
                        }
                        let left_width = string_ops::str_read_char_width(data_region, left_offset)?;
                        if right_offset + STRING_HEADER_BYTES > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(right_offset as u32));
                        }
                        let right_width =
                            string_ops::str_read_char_width(data_region, right_offset)?;
                        string_ops::verify_encoding(left_width, right_width)?;
                        let w = left_width.as_usize();

                        // Lengths are code units; compare the full byte spans.
                        let left_len =
                            string_ops::str_read_cur_len(data_region, left_offset) as usize * w;
                        let left_start = left_offset + STRING_HEADER_BYTES;
                        let left_data = &data_region[left_start..left_start + left_len];

                        let right_len =
                            string_ops::str_read_cur_len(data_region, right_offset) as usize * w;
                        let right_start = right_offset + STRING_HEADER_BYTES;
                        let right_data = &data_region[right_start..right_start + right_len];

                        let cmp_val = match left_data.cmp(right_data) {
                            core::cmp::Ordering::Less => -1i32,
                            core::cmp::Ordering::Equal => 0i32,
                            core::cmp::Ordering::Greater => 1i32,
                        };
                        stack.push(Slot::from_i32(cmp_val))?;
                    }
                    _ => builtin::dispatch(func_id, stack)?,
                }
            }
            opcode::CALL => {
                let func_id_raw = read_u16_le(bytecode, &mut pc)?;
                let var_offset = read_u16_le(bytecode, &mut pc)?;
                let func_id = FunctionId::new(func_id_raw);
                let func = container
                    .code
                    .get_function(func_id)
                    .ok_or(Trap::InvalidFunctionId(func_id))?;

                let func_scope = VariableScope {
                    shared_globals_size: scope.shared_globals_size,
                    instance_offset: var_offset,
                    instance_count: func.num_locals,
                };

                // Pop arguments from stack into function's parameter slots
                // (reverse order so the leftmost argument lands in the lowest
                // slot).
                for i in (0..func.num_params).rev() {
                    let val = stack.pop()?;
                    variables.store(VarIndex::new(var_offset + i), val)?;
                }

                // Save the caller's pc (already advanced past the CALL's
                // operand bytes) on its frame, then push the callee's frame.
                // `FrameStack::push` returns Trap::CallStackOverflow on
                // capacity overflow — same trap as the old depth-counter
                // check.
                frame_stack
                    .top_mut()
                    .expect("non-empty: caller frame is still on stack")
                    .pc = pc;
                hook.before_call(func_id);
                frame_stack.push(Frame {
                    function_id: func_id,
                    pc: 0,
                    scope: func_scope,
                    temp_alloc_mark: temp_alloc.next(),
                    fb_return: None,
                })?;
                advance_pc = false;
            }
            opcode::RET => {
                // Return value is already on the operand stack; just unwind
                // this frame and let the caller frame (or outer-loop exit)
                // resume.
                handle_frame_return(
                    &mut temp_alloc,
                    &mut frame_stack,
                    data_region,
                    variables,
                    hook,
                )?;
                advance_pc = false;
            }
            // --- String opcodes ---
            //
            // Strings are variable-length and can't fit in the fixed-width
            // 64-bit stack slots. They live in two places:
            //
            //   1. **Data region**: persistent storage for STRING variables.
            //      Each string is laid out per ADR-0015 as:
            //        [max_length: u16][cur_length: u16][data: up to max_length bytes]
            //      The `data_offset` (byte offset into the data region) identifies
            //      each string and is baked into the bytecode operand.
            //
            //   2. **Temp buffers**: short-lived staging area for intermediate
            //      string values. Same [max][cur][data] layout. The temp buffer
            //      pool is a flat byte array divided into equal-sized slots; a
            //      `buf_idx` (which fits in one stack slot) identifies which temp
            //      buffer holds the data. A bump allocator (`TempBufAllocator`)
            //      hands out temp buffers within a single function call.
            //
            // The typical pattern for string assignment is:
            //   LOAD_CONST_STR pool[i]    -- copy literal → temp buf, push buf_idx
            //   STR_STORE_VAR  offset     -- pop buf_idx, copy temp buf → data region

            // STR_INIT: Initialize a string variable's header in the data region.
            //
            // Operands: data_offset (u16), max_length (u16)
            // Stack effect: none
            //
            // Sets max_length and zeros cur_length. This is emitted once per
            // STRING variable during program initialization, before any values
            // are stored. STR_STORE_VAR relies on max_length being set here
            // to enforce the capacity bound.
            opcode::STR_INIT => {
                let data_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let max_length = read_u16_le(bytecode, &mut pc)?;
                // The char_width operand (1 = narrow, 2 = wide) is emitted by
                // codegen from the variable's declared type (ADR-0034). Validate
                // it here so a tampered/garbage byte traps rather than corrupts.
                let char_width_byte = read_u8(bytecode, &mut pc)?;
                let char_width = CharWidth::from_u8(char_width_byte)
                    .map_err(|_| Trap::InvalidCharWidth(char_width_byte))?;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                string_ops::str_write_header(data_region, data_offset, max_length, 0, char_width);
            }

            // LOAD_CONST_STR: Load a string literal from the constant pool
            // into a temp buffer.
            //
            // Operands: pool_index (u16)
            // Stack effect: pushes buf_idx (the temp buffer holding the string)
            //
            // Steps:
            //   1. Look up raw bytes in the constant pool
            //   2. Claim the next temp buffer from the bump allocator
            //   3. Write the string into the temp buffer in [max][cur][data] format
            //   4. Push the buf_idx onto the stack so a subsequent opcode
            //      (e.g. STR_STORE_VAR) can find the data
            opcode::LOAD_CONST_STR => {
                let index = read_u16_le(bytecode, &mut pc)?;
                let cindex = ConstantIndex::new(index);
                // The entry's const_type determines the encoding (ADR-0034).
                let char_width = container
                    .constant_pool
                    .char_width(cindex)
                    .map_err(|_| Trap::InvalidConstantIndex(cindex))?;
                let str_bytes = container
                    .constant_pool
                    .get_str(cindex)
                    .map_err(|_| Trap::InvalidConstantIndex(cindex))?;

                let (buf_idx, buf_start, max_len) = {
                    let slot = temp_alloc.alloc(temp_buf.len(), char_width)?;
                    (slot.buf_idx as usize, slot.buf_start, slot.max_len)
                };

                // The pool stores raw bytes; its code-unit length is
                // bytes / char_width. Clamp to the slot capacity (units).
                let src_units = (str_bytes.len() / char_width.as_usize()) as u16;
                let cur_len = src_units.min(max_len);
                string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len, char_width);
                let copy_bytes = cur_len as usize * char_width.as_usize();
                temp_buf
                    [buf_start + STRING_HEADER_BYTES..buf_start + STRING_HEADER_BYTES + copy_bytes]
                    .copy_from_slice(&str_bytes[..copy_bytes]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }

            // STR_STORE_VAR: Copy a string from a temp buffer into the
            // data region (i.e., assign to a STRING variable).
            //
            // Operands: data_offset (u16) — where the destination string lives
            // Stack effect: pops buf_idx
            //
            // Steps:
            //   1. Pop buf_idx to locate the source temp buffer
            //   2. Read the source's cur_length from the temp buffer header
            //   3. Read the destination's max_length from the data region header
            //      (set earlier by STR_INIT)
            //   4. Copy min(src_cur_length, dest_max_length) bytes — this
            //      silently truncates if the source is longer than the
            //      destination can hold (IEC 61131-3 assignment semantics)
            //   5. Update the destination's cur_length
            opcode::STR_STORE_VAR => {
                let data_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let buf_idx = stack.pop()?.as_i32() as usize;

                let buf_start = buf_idx * max_temp_buf_bytes;
                if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                let src_width = string_ops::str_read_char_width(temp_buf, buf_start)?;
                let src_cur_len = string_ops::str_read_cur_len(temp_buf, buf_start);

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                // The source's encoding must match the destination variable's
                // (ADR-0034); a mismatch is a compiler bug or tampered bytecode.
                let dest_width = string_ops::str_read_char_width(data_region, data_offset)?;
                string_ops::verify_encoding(dest_width, src_width)?;
                let dest_max_len = string_ops::str_read_max_len(data_region, data_offset);

                // Lengths are code units; truncate to the destination capacity.
                let copy_units = src_cur_len.min(dest_max_len) as usize;
                let copy_bytes = copy_units * dest_width.as_usize();
                if data_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let dst_start = data_offset + STRING_HEADER_BYTES;
                let src_start = buf_start + STRING_HEADER_BYTES;
                data_region[dst_start..dst_start + copy_bytes]
                    .copy_from_slice(&temp_buf[src_start..src_start + copy_bytes]);

                // Update destination cur_length (code units).
                string_ops::str_write_cur_len(data_region, data_offset, copy_units as u16);
            }

            // STR_LOAD_VAR: Copy a string from the data region into a temp
            // buffer (i.e., read a STRING variable for use in an expression).
            //
            // Operands: data_offset (u16) — where the source string lives
            // Stack effect: pushes buf_idx
            //
            // This is the inverse of STR_STORE_VAR: it reads from the data
            // region and writes to a temp buffer so the string value can be
            // passed through the stack to another opcode.
            opcode::STR_LOAD_VAR => {
                let data_offset = read_u32_le(bytecode, &mut pc)? as usize;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let src_width = string_ops::str_read_char_width(data_region, data_offset)?;
                let src_max_len = string_ops::str_read_max_len(data_region, data_offset);
                let src_cur_len = string_ops::str_read_cur_len(data_region, data_offset);
                // Defensive: never read more than max_length code units.
                let read_units = src_cur_len.min(src_max_len) as usize;

                let (buf_idx, buf_start, max_len) = {
                    let slot = temp_alloc.alloc(temp_buf.len(), src_width)?;
                    (slot.buf_idx as usize, slot.buf_start, slot.max_len)
                };

                let cur_len = (read_units as u16).min(max_len);
                string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len, src_width);
                let copy_bytes = cur_len as usize * src_width.as_usize();
                if data_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let dst_start = buf_start + STRING_HEADER_BYTES;
                let src_start = data_offset + STRING_HEADER_BYTES;
                temp_buf[dst_start..dst_start + copy_bytes]
                    .copy_from_slice(&data_region[src_start..src_start + copy_bytes]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // --- String function opcodes ---
            //
            // LEN_STR reads the cur_length field from a STRING variable's
            // header in the data region and pushes it as an i32.
            opcode::LEN_STR => {
                let data_offset = read_u32_le(bytecode, &mut pc)? as usize;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let cur_len = u16::from_le_bytes([
                    data_region[data_offset + 2],
                    data_region[data_offset + 3],
                ]);
                stack.push(Slot::from_i32(cur_len as i32))?;
            }
            // FIND_STR: Find the first occurrence of IN2 within IN1.
            // Returns 1-based position or 0 if not found.
            opcode::FIND_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc)? as usize;

                let (in1_len, in1_start, w1) =
                    string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start, w2) =
                    string_ops::read_string_header(data_region, in2_offset)?;
                string_ops::verify_encoding(w1, w2)?;
                let w = w1.as_usize();

                let result = if in2_len == 0 || in2_len > in1_len {
                    // Empty search string or search string longer than haystack: not found.
                    0i32
                } else {
                    // Byte spans scale by char_width; matches must land on a
                    // code-unit boundary, so step a code unit at a time.
                    let in1_data = &data_region[in1_start..in1_start + in1_len * w];
                    let in2_data = &data_region[in2_start..in2_start + in2_len * w];

                    let mut found = 0i32;
                    for i in 0..=(in1_len - in2_len) {
                        if in1_data[i * w..(i + in2_len) * w] == *in2_data {
                            found = (i + 1) as i32; // 1-based code-unit position
                            break;
                        }
                    }
                    found
                };
                stack.push(Slot::from_i32(result))?;
            }
            // REPLACE_STR: Replace L characters starting at position P in IN1
            // with IN2. Pops P then L from stack, pushes buf_idx.
            opcode::REPLACE_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in1_len, in1_start, w1) =
                    string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start, w2) =
                    string_ops::read_string_header(data_region, in2_offset)?;
                string_ops::verify_encoding(w1, w2)?;
                let w = w1.as_usize();

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in1_len);
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN2 + IN1[start_idx+delete_len..]
                // (all indices/lengths are code units).
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + in2_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len(), w1)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    w1,
                );

                // Write result data: prefix + IN2 + suffix (byte spans scaled).
                let mut write_units = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    in1_start,
                    prefix_copy,
                    w1,
                );
                write_units += prefix_copy;
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_units));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + write_units * w,
                    data_region,
                    in2_start,
                    in2_copy,
                    w1,
                );
                write_units += in2_copy;
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_units));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + write_units * w,
                    data_region,
                    in1_start + suffix_start * w,
                    suffix_copy,
                    w1,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // INSERT_STR: Insert IN2 into IN1 after position P.
            // Pops P from stack, pushes buf_idx.
            opcode::INSERT_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let p_val = stack.pop()?.as_i32();

                let (in1_len, in1_start, w1) =
                    string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start, w2) =
                    string_ops::read_string_header(data_region, in2_offset)?;
                string_ops::verify_encoding(w1, w2)?;
                let w = w1.as_usize();

                let p = if p_val < 0 { 0usize } else { p_val as usize };
                let insert_idx = p.min(in1_len);

                // Result = IN1[0..insert_idx] + IN2 + IN1[insert_idx..]
                // (all indices/lengths are code units).
                let prefix_len = insert_idx;
                let suffix_len = in1_len - insert_idx;
                let result_len = prefix_len + in2_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len(), w1)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    w1,
                );

                // Write result data: prefix + IN2 + suffix (byte spans scaled).
                let mut write_units = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    in1_start,
                    prefix_copy,
                    w1,
                );
                write_units += prefix_copy;
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_units));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + write_units * w,
                    data_region,
                    in2_start,
                    in2_copy,
                    w1,
                );
                write_units += in2_copy;
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_units));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + write_units * w,
                    data_region,
                    in1_start + insert_idx * w,
                    suffix_copy,
                    w1,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // DELETE_STR: Delete L characters from IN1 starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::DELETE_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in1_len, in1_start, w1) =
                    string_ops::read_string_header(data_region, in1_offset)?;
                let w = w1.as_usize();

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in1_len);
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN1[start_idx+delete_len..]
                // (all indices/lengths are code units).
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len(), w1)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    w1,
                );

                // Write result data: prefix + suffix (byte spans scaled).
                let mut write_units = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    in1_start,
                    prefix_copy,
                    w1,
                );
                write_units += prefix_copy;
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_units));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + write_units * w,
                    data_region,
                    in1_start + suffix_start * w,
                    suffix_copy,
                    w1,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // LEFT_STR: Return the leftmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::LEFT_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start, in_width) =
                    string_ops::read_string_header(data_region, in_offset)?;

                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let result_len = l.min(in_len);

                let slot = temp_alloc.alloc(temp_buf.len(), in_width)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    in_width,
                );

                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    in_start,
                    cur_len as usize,
                    in_width,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // RIGHT_STR: Return the rightmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::RIGHT_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start, in_width) =
                    string_ops::read_string_header(data_region, in_offset)?;

                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let result_len = l.min(in_len);
                let src_offset_units = in_len - result_len;

                let slot = temp_alloc.alloc(temp_buf.len(), in_width)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    in_width,
                );

                let src = in_start + src_offset_units * in_width.as_usize();
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    src,
                    cur_len as usize,
                    in_width,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // MID_STR: Return L characters from IN starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::MID_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start, in_width) =
                    string_ops::read_string_header(data_region, in_offset)?;

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in_len);
                let result_len = l.min(in_len - start_idx);

                let slot = temp_alloc.alloc(temp_buf.len(), in_width)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    in_width,
                );

                let src = in_start + start_idx * in_width.as_usize();
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    src,
                    cur_len as usize,
                    in_width,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // CONCAT_STR: Concatenate IN1 and IN2.
            // Pushes buf_idx.
            opcode::CONCAT_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc)? as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc)? as usize;

                let (in1_len, in1_start, w1) =
                    string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start, w2) =
                    string_ops::read_string_header(data_region, in2_offset)?;
                string_ops::verify_encoding(w1, w2)?;
                let w = w1.as_usize();

                let result_len = in1_len + in2_len;

                let slot = temp_alloc.alloc(temp_buf.len(), w1)?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                    w1,
                );

                // Write result data: IN1 + IN2 (byte spans scaled by width).
                let in1_copy = in1_len.min(cur_len as usize);
                string_ops::copy_code_units(
                    temp_buf,
                    data_start,
                    data_region,
                    in1_start,
                    in1_copy,
                    w1,
                );
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(in1_copy));
                string_ops::copy_code_units(
                    temp_buf,
                    data_start + in1_copy * w,
                    data_region,
                    in2_start,
                    in2_copy,
                    w1,
                );

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // --- String array opcodes ---

            // STR_INIT_ARRAY: Initialize all string headers in an array of strings.
            //
            // Operands: var_index (u16), desc_index (u16)
            // Stack effect: none
            //
            // Reads element_extra from the array descriptor as max_string_length.
            // Element stride = STRING_HEADER_BYTES + max_string_length.
            // Loops through all elements writing [max_len][cur_len=0] headers.
            opcode::STR_INIT_ARRAY => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;

                let desc = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;
                let total_elements = desc.total_elements;
                let max_str_len = desc.element_extra;
                // Element width comes from the descriptor's element_type
                // (FieldType::String vs WString). The stride spans the header
                // plus max_str_len code units, each char_width bytes (ADR-0035).
                let char_width = desc.element_char_width();
                let stride = STRING_HEADER_BYTES + max_str_len as usize * char_width.as_usize();

                scope.check_access(var_index)?;
                let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;

                for i in 0..total_elements as usize {
                    let elem_offset = base_offset + i * stride;
                    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
                        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                    }
                    string_ops::str_write_header(
                        data_region,
                        elem_offset,
                        max_str_len,
                        0,
                        char_width,
                    );
                }
            }

            // STR_LOAD_ARRAY_ELEM: Load a string from an array element into a temp buffer.
            //
            // Operands: var_index (u16), desc_index (u16)
            // Stack effect: pops flat_index, pushes buf_idx (net 0)
            //
            // Computes element offset = base + flat_index * stride, then copies the
            // string from the data region into a temp buffer.
            opcode::STR_LOAD_ARRAY_ELEM => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;
                let index_i64 = index_slot.as_i64();

                let desc = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;
                let total_elements = desc.total_elements;
                let max_str_len = desc.element_extra;
                let stride = STRING_HEADER_BYTES
                    + max_str_len as usize * desc.element_char_width().as_usize();

                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as usize;

                scope.check_access(var_index)?;
                let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;
                let elem_offset = base_offset + index * stride;

                if elem_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                let elem_width = string_ops::str_read_char_width(data_region, elem_offset)?;
                let src_cur_len = string_ops::str_read_cur_len(data_region, elem_offset);
                let read_units = src_cur_len.min(max_str_len) as usize;

                let (buf_idx, buf_start, buf_max_len) = {
                    let slot = temp_alloc.alloc(temp_buf.len(), elem_width)?;
                    (slot.buf_idx as usize, slot.buf_start, slot.max_len)
                };

                let cur_len = (read_units as u16).min(buf_max_len);
                string_ops::str_write_header(temp_buf, buf_start, buf_max_len, cur_len, elem_width);

                let copy_bytes = cur_len as usize * elem_width.as_usize();
                if elem_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                string_ops::copy_code_units(
                    temp_buf,
                    buf_start + STRING_HEADER_BYTES,
                    data_region,
                    elem_offset + STRING_HEADER_BYTES,
                    cur_len as usize,
                    elem_width,
                );

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }

            // STR_STORE_ARRAY_ELEM: Store a temp buffer into a string array element.
            //
            // Operands: var_index (u16), desc_index (u16)
            // Stack effect: pops flat_index, pops buf_idx (net -2)
            //
            // Computes element offset = base + flat_index * stride, then copies
            // the temp buffer contents into the data region, truncating per IEC 61131-3.
            opcode::STR_STORE_ARRAY_ELEM => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;
                let value_slot = stack.pop()?;
                let index_i64 = index_slot.as_i64();
                let buf_idx = value_slot.as_i32() as usize;

                let desc = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;
                let total_elements = desc.total_elements;
                let max_str_len = desc.element_extra;
                let stride = STRING_HEADER_BYTES
                    + max_str_len as usize * desc.element_char_width().as_usize();

                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as usize;

                scope.check_access(var_index)?;
                let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;
                let elem_offset = base_offset + index * stride;

                // Read source from temp buffer.
                let buf_start = buf_idx * max_temp_buf_bytes;
                if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                let src_width = string_ops::str_read_char_width(temp_buf, buf_start)?;
                let src_cur_len = string_ops::str_read_cur_len(temp_buf, buf_start);

                if elem_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                // The source's encoding must match the array element's (ADR-0034).
                let dest_width = string_ops::str_read_char_width(data_region, elem_offset)?;
                string_ops::verify_encoding(dest_width, src_width)?;

                // Copy, truncating to max_str_len code units per IEC 61131-3.
                let copy_units = src_cur_len.min(max_str_len) as usize;
                let copy_bytes = copy_units * dest_width.as_usize();
                if elem_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                string_ops::copy_code_units(
                    data_region,
                    elem_offset + STRING_HEADER_BYTES,
                    temp_buf,
                    buf_start + STRING_HEADER_BYTES,
                    copy_units,
                    dest_width,
                );

                // Update destination cur_length (code units).
                string_ops::str_write_cur_len(data_region, elem_offset, copy_units as u16);
            }

            opcode::POP => {
                stack.pop()?;
            }
            opcode::DUP => {
                stack.dup()?;
            }
            opcode::SWAP => {
                stack.swap()?;
            }
            // --- Function block opcodes ---
            opcode::FB_LOAD_INSTANCE => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                scope.check_access(var_index)?;
                let slot = variables.load(var_index)?;
                stack.push(slot)?;
            }
            opcode::FB_STORE_PARAM => {
                let field = read_u8(bytecode, &mut pc)? as u16;
                let value = stack.pop()?;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                data_region[offset..offset + 8].copy_from_slice(&value.as_i64().to_le_bytes());
            }
            opcode::FB_LOAD_PARAM => {
                let field = read_u8(bytecode, &mut pc)? as u16;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[offset..offset + 8]);
                stack.push(Slot::from_i64(i64::from_le_bytes(buf)))?;
            }
            opcode::FB_CALL => {
                let type_id = read_u16_le(bytecode, &mut pc)?;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let instance_start = fb_ref as usize;
                match type_id {
                    opcode::fb_type::TON | opcode::fb_type::TOF | opcode::fb_type::TP => {
                        let instance_size = crate::intrinsic::TIMER_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        let time = current_time_us as i64;
                        match type_id {
                            opcode::fb_type::TON => crate::intrinsic::ton(slice, time)?,
                            opcode::fb_type::TOF => crate::intrinsic::tof(slice, time)?,
                            opcode::fb_type::TP => crate::intrinsic::tp(slice, time)?,
                            _ => unreachable!(),
                        }
                    }
                    opcode::fb_type::CTU => {
                        let instance_size = crate::intrinsic::CTU_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::ctu(slice)?;
                    }
                    opcode::fb_type::CTD => {
                        let instance_size = crate::intrinsic::CTD_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::ctd(slice)?;
                    }
                    opcode::fb_type::CTUD => {
                        let instance_size = crate::intrinsic::CTUD_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::ctud(slice)?;
                    }
                    opcode::fb_type::SR => {
                        let instance_size = crate::intrinsic::SR_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::sr(slice)?;
                    }
                    opcode::fb_type::RS => {
                        let instance_size = crate::intrinsic::RS_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::rs(slice)?;
                    }
                    opcode::fb_type::R_TRIG => {
                        let instance_size = crate::intrinsic::R_TRIG_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::r_trig(slice)?;
                    }
                    opcode::fb_type::F_TRIG => {
                        let instance_size = crate::intrinsic::F_TRIG_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u32));
                        }
                        let slice = &mut data_region[instance_start..instance_end];
                        crate::intrinsic::f_trig(slice)?;
                    }
                    _ => {
                        // User-defined FB: look up in the container's user FB table.
                        let fb_type_id = FbTypeId::new(type_id);
                        let user_fb = container
                            .type_section
                            .as_ref()
                            .and_then(|ts| {
                                ts.user_fb_types.iter().find(|d| d.type_id == fb_type_id)
                            })
                            .ok_or(Trap::InvalidFbTypeId(fb_type_id))?;

                        let func_id = user_fb.function_id;
                        let var_off = user_fb.var_offset;
                        let num_fields = user_fb.num_fields;

                        let func = container
                            .code
                            .get_function(func_id)
                            .ok_or(Trap::InvalidFunctionId(func_id))?;

                        // Copy-in: data region fields -> variable table slots.
                        for i in 0..num_fields as usize {
                            let offset = instance_start + i * 8;
                            if offset + 8 > data_region.len() {
                                return Err(Trap::DataRegionOutOfBounds(offset as u32));
                            }
                            let mut buf = [0u8; 8];
                            buf.copy_from_slice(&data_region[offset..offset + 8]);
                            variables.store(
                                VarIndex::new(var_off + i as u16),
                                Slot::from_i64(i64::from_le_bytes(buf)),
                            )?;
                        }

                        let func_scope = VariableScope {
                            shared_globals_size: scope.shared_globals_size,
                            instance_offset: var_off,
                            instance_count: func.num_locals,
                        };

                        // Save the caller's pc, then push the FB frame.
                        // `fb_return` records what `handle_frame_return`
                        // needs to copy variable slots back to the data
                        // region after the FB body returns.
                        frame_stack
                            .top_mut()
                            .expect("non-empty: caller frame is still on stack")
                            .pc = pc;
                        hook.before_call(func_id);
                        frame_stack.push(Frame {
                            function_id: func_id,
                            pc: 0,
                            scope: func_scope,
                            temp_alloc_mark: temp_alloc.next(),
                            fb_return: Some(FbCallReturn {
                                instance_start,
                                var_offset: var_off,
                                num_fields,
                            }),
                        })?;
                        advance_pc = false;
                    }
                }
            }
            // --- Array opcodes ---
            opcode::LOAD_ARRAY => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;

                // Read index as i64 to catch overflow from i64 flat-index arithmetic.
                let index_i64 = index_slot.as_i64();

                // Look up array descriptor by index (O(1) Vec access)
                let total_elements = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .map(|d| d.total_elements)
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;

                // Bounds check: 0 <= index < total_elements
                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as u32;

                scope.check_access(var_index)?;

                let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;
                let byte_offset = data_offset + index as usize * 8;

                if byte_offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
                }

                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
                let raw = i64::from_le_bytes(buf);
                stack.push(Slot::from_i64(raw))?;
            }
            opcode::STORE_ARRAY => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;
                let value_slot = stack.pop()?;

                let index_i64 = index_slot.as_i64();

                let total_elements = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .map(|d| d.total_elements)
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;

                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as u32;

                scope.check_access(var_index)?;

                let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;
                let byte_offset = data_offset + index as usize * 8;

                if byte_offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
                }

                data_region[byte_offset..byte_offset + 8]
                    .copy_from_slice(&value_slot.as_i64().to_le_bytes());
            }
            opcode::LOAD_ARRAY_DEREF => {
                let ref_var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;
                let index_i64 = index_slot.as_i64();

                // Resolve the reference: load the target variable index.
                scope.check_access(ref_var_index)?;
                let target_slot = variables.load(ref_var_index)?;
                let target_raw = target_slot.as_i64() as u64;
                if target_raw == u64::MAX {
                    return Err(Trap::NullDereference);
                }
                let target_var_index = VarIndex::new(target_raw as u16);

                // Bounds check via descriptor.
                let total_elements = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .map(|d| d.total_elements)
                    .ok_or(Trap::InvalidVariableIndex(ref_var_index))?;

                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index: target_var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as u32;

                // Double indirection: load data_offset from target array variable.
                // No scope check on target — it lives in the caller's scope,
                // matching LOAD_INDIRECT/STORE_INDIRECT behavior.
                let data_offset = variables.load(target_var_index)?.as_i32() as u32 as usize;
                let byte_offset = data_offset + index as usize * 8;

                if byte_offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
                }

                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
                let raw = i64::from_le_bytes(buf);
                stack.push(Slot::from_i64(raw))?;
            }
            opcode::STORE_ARRAY_DEREF => {
                let ref_var_index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
                let desc_index = read_u16_le(bytecode, &mut pc)?;
                let index_slot = stack.pop()?;
                let value_slot = stack.pop()?;
                let index_i64 = index_slot.as_i64();

                // Resolve the reference: load the target variable index.
                scope.check_access(ref_var_index)?;
                let target_slot = variables.load(ref_var_index)?;
                let target_raw = target_slot.as_i64() as u64;
                if target_raw == u64::MAX {
                    return Err(Trap::NullDereference);
                }
                let target_var_index = VarIndex::new(target_raw as u16);

                // Bounds check via descriptor.
                let total_elements = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .map(|d| d.total_elements)
                    .ok_or(Trap::InvalidVariableIndex(ref_var_index))?;

                if index_i64 < 0 || index_i64 >= total_elements as i64 {
                    return Err(Trap::ArrayIndexOutOfBounds {
                        var_index: target_var_index,
                        index: index_i64 as i32,
                        total_elements,
                    });
                }
                let index = index_i64 as u32;

                // Double indirection: load data_offset from target array variable.
                // No scope check on target — it lives in the caller's scope,
                // matching LOAD_INDIRECT/STORE_INDIRECT behavior.
                let data_offset = variables.load(target_var_index)?.as_i32() as u32 as usize;
                let byte_offset = data_offset + index as usize * 8;

                if byte_offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
                }

                data_region[byte_offset..byte_offset + 8]
                    .copy_from_slice(&value_slot.as_i64().to_le_bytes());
            }
            opcode::RET_VOID => {
                handle_frame_return(
                    &mut temp_alloc,
                    &mut frame_stack,
                    data_region,
                    variables,
                    hook,
                )?;
                advance_pc = false;
            }
            _ => {
                return Err(Trap::InvalidInstruction(op));
            }
        }

        if advance_pc {
            // Write the working `pc` back to the frame we entered this
            // iteration with. CALL / RET / FB_CALL-user-branch arms set
            // `advance_pc = false` because they have already updated
            // the stack and the top frame is no longer the one we
            // started with.
            frame_stack
                .top_mut()
                .expect("non-empty: advance_pc=true means no push/pop happened")
                .pc = pc;
        }
    }

    // The frame stack drained: the program returned. Record the (empty)
    // frame count and temp-allocator position so a subsequent fresh scan
    // starts from a clean slate.
    *frame_count = frame_stack.len();
    *temp_alloc_next = temp_alloc.next();
    Ok(ExecuteOutcome::Completed)
}

/// Pops the topmost frame, rewinds the temp-buffer allocator to the
/// frame's mark, and (for `FB_CALL` frames) copies variable slots back
/// into the FB instance's data-region fields.
///
/// Used by `RET`, `RET_VOID`, and the implicit-return path (running off
/// the end of a function body) inside the iterative dispatch loop. This is
/// the single choke point for frame pops, so [`DebugHook::after_return`]
/// fires here exactly once per pop — keeping a hook's depth counter in
/// lock-step with `frame_stack.len()`.
fn handle_frame_return<H: DebugHook>(
    temp_alloc: &mut string_ops::TempBufAllocator,
    frame_stack: &mut FrameStack,
    data_region: &mut [u8],
    variables: &mut VariableTable,
    hook: &mut H,
) -> Result<(), Trap> {
    let popped = frame_stack
        .pop()
        .expect("caller must hold the loop invariant: non-empty before return");
    temp_alloc.rewind_to(popped.temp_alloc_mark);

    if let Some(fbr) = popped.fb_return {
        fbr.copy_out(variables, data_region)?;
    }

    // Notify the hook after the pop, passing the function control returns
    // to — or `None` when the outermost frame just returned.
    let returning_to = frame_stack.top().map(|f| f.function_id);
    hook.after_return(returning_to);

    Ok(())
}

/// Reads a single byte from bytecode at pc, advancing pc by 1.
fn read_u8(bytecode: &[u8], pc: &mut usize) -> Result<u8, Trap> {
    if *pc >= bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = bytecode[*pc];
    *pc += 1;
    Ok(value)
}

/// Reads a little-endian u16 from bytecode at pc, advancing pc by 2.
fn read_u16_le(bytecode: &[u8], pc: &mut usize) -> Result<u16, Trap> {
    let end = *pc + 2;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = u16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc = end;
    Ok(value)
}

/// Reads a little-endian u32 from bytecode at pc, advancing pc by 4.
fn read_u32_le(bytecode: &[u8], pc: &mut usize) -> Result<u32, Trap> {
    let end = *pc + 4;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = u32::from_le_bytes([
        bytecode[*pc],
        bytecode[*pc + 1],
        bytecode[*pc + 2],
        bytecode[*pc + 3],
    ]);
    *pc = end;
    Ok(value)
}

/// Reads a little-endian i16 from bytecode at pc, advancing pc by 2.
fn read_i16_le(bytecode: &[u8], pc: &mut usize) -> Result<i16, Trap> {
    let end = *pc + 2;
    if end > bytecode.len() {
        return Err(Trap::UnexpectedEndOfBytecode);
    }
    let value = i16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc = end;
    Ok(value)
}

/// A small stack-allocated buffer for formatting numbers as strings.
///
/// Used by CONV_I32_TO_STR, CONV_U32_TO_STR, and CONV_F32_TO_STR to
/// avoid heap allocation. 48 bytes is enough for any i32, u32, or f32
/// decimal representation.
struct StackFmtBuf {
    buf: [u8; 48],
    len: usize,
}

impl StackFmtBuf {
    fn new() -> Self {
        Self {
            buf: [0u8; 48],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl core::fmt::Write for StackFmtBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.len;
        let to_copy = bytes.len().min(remaining);
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VmBuffers;
    use ironplc_container::ContainerBuilder;

    /// Builds a container with one function from the given bytecode,
    /// with `num_vars` variables and the given constants.
    /// Uses a generous max_stack_depth (16) suitable for most tests.
    fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
        let mut builder = ContainerBuilder::new().num_variables(num_vars);
        for &c in constants {
            builder = builder.add_i32_constant(c);
        }
        builder
            .add_function(FunctionId::INIT, &[0x8C], 0, num_vars, 0) // init: RET_VOID
            .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0) // scan: test bytecode
            .init_function_id(FunctionId::INIT)
            .entry_function_id(FunctionId::SCAN)
            .build()
    }

    /// Asserts that a run_round produces a specific trap.
    fn assert_trap(vm: &mut VmRunning, expected: Trap) {
        let result = vm.run_round(0);
        assert!(
            result.is_err(),
            "expected trap {expected} but run_round succeeded"
        );
        assert_eq!(result.unwrap_err().trap, expected);
    }

    fn steel_thread_container() -> Container {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
            0x0C, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
            0x00, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x20,                   // ADD_I32
            0x10, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
            0x8C,                   // RET_VOID
        ];

        ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(FunctionId::INIT, &[0x8C], 0, 2, 0) // init: RET_VOID
            .add_function(FunctionId::SCAN, &bytecode, 2, 2, 0) // scan: program body
            .init_function_id(FunctionId::INIT)
            .entry_function_id(FunctionId::SCAN)
            .build()
    }

    #[test]
    fn vm_load_when_valid_container_then_returns_ready() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let ready = Vm::new().load(&c, &mut b);

        // If this compiles, the VM is in the Ready state.
        // Verify we can read the initial variable values.
        assert_eq!(ready.read_variable(VarIndex::new(0)).unwrap(), 0);
    }

    #[test]
    fn vm_run_round_when_steel_thread_then_x_is_10_y_is_42() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        vm.run_round(0).unwrap();

        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
        assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 42);
    }

    #[test]
    fn vm_run_round_when_invalid_opcode_then_trap() {
        let c = single_function_container(&[0xFF], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::InvalidInstruction(0xFF));
    }

    #[test]
    fn vm_request_stop_when_called_then_stop_requested() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert!(!vm.stop_requested());
        vm.request_stop();
        assert!(vm.stop_requested());
    }

    #[test]
    fn vm_stop_when_called_then_returns_stopped() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let vm = Vm::new().load(&c, &mut b).start().unwrap();
        let stopped = vm.stop();
        assert_eq!(stopped.read_variable(VarIndex::new(0)).unwrap(), 0); // not yet executed
    }

    #[test]
    fn vm_fault_when_called_then_returns_faulted_with_context() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let vm = Vm::new().load(&c, &mut b).start().unwrap();
        let ctx = FaultContext {
            trap: Trap::WatchdogTimeout(ironplc_container::TaskId::new(3)),
            task_id: ironplc_container::TaskId::new(3),
            instance_id: ironplc_container::InstanceId::new(1),
        };
        let faulted = vm.fault(ctx);
        assert_eq!(
            *faulted.trap(),
            Trap::WatchdogTimeout(ironplc_container::TaskId::new(3))
        );
        assert_eq!(faulted.task_id(), ironplc_container::TaskId::new(3));
        assert_eq!(faulted.instance_id(), ironplc_container::InstanceId::new(1));
    }

    // Phase 1, Step 1.1: Execute error path tests
    // These verify that each Trap variant that can fire inside execute()
    // is triggered through the full Vm::new().load(c).start().run_round() path.

    #[test]
    fn execute_when_stack_overflow_then_trap() {
        // max_stack_depth: 1, but bytecode pushes two values
        // Cannot use single_function_container because it uses max_stack=16.
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]
        ];
        let c = ContainerBuilder::new()
            .num_variables(0)
            .add_i32_constant(1)
            .add_i32_constant(2)
            .add_function(FunctionId::INIT, &[0x8C], 0, 0, 0) // init: RET_VOID
            .add_function(FunctionId::SCAN, &bytecode, 1, 0, 0) // scan: triggers overflow
            .init_function_id(FunctionId::INIT)
            .entry_function_id(FunctionId::SCAN)
            .build();

        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();
        assert_trap(&mut vm, Trap::StackOverflow);
    }

    #[test]
    fn execute_when_stack_underflow_then_trap() {
        // ADD_I32 tries to pop 2 values from an empty stack
        let c = single_function_container(&[0x20], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::StackUnderflow);
    }

    #[test]
    fn execute_when_invalid_constant_index_then_trap() {
        // 0 constants in pool, but bytecode references pool[0]
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        ];
        let c = single_function_container(&bytecode, 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::InvalidConstantIndex(ConstantIndex::new(0)));
    }

    #[test]
    fn execute_when_invalid_variable_index_on_store_then_trap() {
        // 1 variable, but bytecode stores to var[5]
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x10, 0x05, 0x00,  // STORE_VAR_I32 var[5]
        ];
        let c = single_function_container(&bytecode, 1, &[42]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::InvalidVariableIndex(VarIndex::new(5)));
    }

    #[test]
    fn execute_when_invalid_variable_index_on_load_then_trap() {
        // 1 variable, but bytecode loads from var[5]
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x0C, 0x05, 0x00,  // LOAD_VAR_I32 var[5]
        ];
        let c = single_function_container(&bytecode, 1, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::InvalidVariableIndex(VarIndex::new(5)));
    }

    // Phase 1, Step 1.2: Execute edge-case tests

    #[test]
    fn execute_when_call_user_function_then_returns_value() {
        // Layout: var[0] = result (program), var[1] = A, var[2] = B, var[3] = return slot
        // Function 0 (init): RET_VOID
        // Function 1 (scan): push 3, push 7, CALL func 2 var_offset=1, store result, RET_VOID
        // Function 2 (add):  load A, load B, ADD, store return, load return, RET
        //   num_params=2, num_locals=3 (A, B, return_slot)
        #[rustfmt::skip]
        let scan_bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00,        // LOAD_CONST_I32 pool[0] (3)
            0x00, 0x01, 0x00,        // LOAD_CONST_I32 pool[1] (7)
            0x84, 0x02, 0x00, 0x01, 0x00,  // CALL function 2, var_offset=1
            0x10, 0x00, 0x00,        // STORE_VAR_I32 var[0] (result)
            0x8C,                    // RET_VOID
        ];
        #[rustfmt::skip]
        let func_bytecode: Vec<u8> = vec![
            0x0C, 0x01, 0x00,  // LOAD_VAR_I32 var[1] (A - absolute index)
            0x0C, 0x02, 0x00,  // LOAD_VAR_I32 var[2] (B - absolute index)
            0x20,              // ADD_I32
            0x10, 0x03, 0x00,  // STORE_VAR_I32 var[3] (return slot - absolute index)
            0x0C, 0x03, 0x00,  // LOAD_VAR_I32 var[3]
            0x88,              // RET
        ];

        let c = ContainerBuilder::new()
            .num_variables(4) // 1 program var + 3 function vars
            .add_i32_constant(3)
            .add_i32_constant(7)
            .add_function(FunctionId::INIT, &[0x8C], 0, 1, 0) // init
            .add_function(FunctionId::SCAN, &scan_bytecode, 2, 1, 0) // scan
            .add_function(FunctionId::new(2), &func_bytecode, 2, 3, 2) // add (num_params=2)
            .init_function_id(FunctionId::INIT)
            .entry_function_id(FunctionId::SCAN)
            .build();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();
        vm.run_round(0).unwrap();

        // result should be 3 + 7 = 10
        assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
    }

    #[test]
    fn execute_when_empty_bytecode_then_ok() {
        let c = single_function_container(&[], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert!(vm.run_round(0).is_ok());
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn vm_default_when_called_then_loads_container() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let ready = Vm::default().load(&c, &mut b);

        assert_eq!(ready.read_variable(VarIndex::new(0)).unwrap(), 0);
    }

    #[test]
    fn vm_ready_read_variable_raw_when_before_init_then_returns_zero_slot() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let ready = Vm::new().load(&c, &mut b);

        assert_eq!(ready.read_variable_raw(VarIndex::new(0)).unwrap(), 0u64);
        assert_eq!(ready.read_variable_raw(VarIndex::new(1)).unwrap(), 0u64);
    }

    #[test]
    fn vm_run_round_when_no_tasks_enabled_then_returns_ok_without_executing() {
        use ironplc_container::{ProgramInstanceEntry, TaskEntry};

        let c = ContainerBuilder::new()
            .num_variables(0)
            .add_function(FunctionId::INIT, &[0x8C], 0, 0, 0)
            .add_function(FunctionId::SCAN, &[0x8C], 0, 0, 0)
            .add_task(TaskEntry {
                task_id: TaskId::DEFAULT,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x00, // disabled
                interval_us: 0,
                single_var_index: VarIndex::NO_SINGLE_VAR,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            })
            .add_program_instance(ProgramInstanceEntry {
                instance_id: InstanceId::DEFAULT,
                task_id: TaskId::DEFAULT,
                entry_function_id: FunctionId::SCAN,
                var_table_offset: 0,
                var_table_count: 0,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: FunctionId::INIT,
            })
            .init_function_id(FunctionId::INIT)
            .entry_function_id(FunctionId::SCAN)
            .build();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert!(vm.run_round(0).is_ok());
    }
}
