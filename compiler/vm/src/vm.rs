use ironplc_container::{Container, InstanceId, TaskId, TaskType, VarIndex};
#[cfg(test)]
use ironplc_container::{ConstantIndex, FunctionId};

use crate::buffers::VmBuffers;
use crate::dispatch::execute;
use crate::error::Trap;
#[cfg(feature = "profiling")]
use crate::profile::InstructionProfile;
use crate::scheduler::{ProgramInstanceState, TaskScheduler, TaskState};
use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Maximum depth of nested CALL / user-FB_CALL frames before the VM
/// traps with [`Trap::CallStackOverflow`].
///
/// The VM currently runs callee bytecode by recursively invoking
/// `execute_with_hook`, so the Rust thread stack is consumed per frame.
/// This bound keeps the VM well clear of the thread-stack limit
/// (including under test harnesses with smaller stacks) while leaving
/// plenty of headroom for realistic IEC-61131 programs, which typically
/// nest only a handful of levels.
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

        // Execute init functions once before entering scan mode.
        for pi in 0..self.program_instances.len() {
            let init_fn = self.program_instances[pi].init_function_id;
            let instance_id = self.program_instances[pi].instance_id;
            let task_id = self.program_instances[pi].task_id;
            let var_table_offset = self.program_instances[pi].var_table_offset;
            let var_table_count = self.program_instances[pi].var_table_count;

            let bytecode =
                self.container
                    .code
                    .get_function_bytecode(init_fn)
                    .ok_or(FaultContext {
                        trap: Trap::InvalidFunctionId(init_fn),
                        task_id,
                        instance_id,
                    })?;

            let scope = VariableScope {
                shared_globals_size,
                instance_offset: var_table_offset,
                instance_count: var_table_count,
            };

            execute(
                bytecode,
                self.container,
                &mut self.stack,
                &mut self.variables,
                self.data_region,
                self.temp_buf,
                self.max_temp_buf_bytes,
                &scope,
                0, // init functions don't need real time
                0, // top-of-chain call: depth starts at zero
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
            shared_globals_size,
            scan_count: 0,
            stop_requested: false,
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
            shared_globals_size,
            scan_count: initial_scan_count,
            stop_requested: false,
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
    shared_globals_size: u16,
    scan_count: u64,
    stop_requested: bool,
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
        if self.container.header.flags & ironplc_container::FLAG_HAS_SYSTEM_UPTIME != 0 {
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

        // Stub: INPUT_FREEZE (no-op)

        for ri in 0..ready_count {
            let task_idx = self.ready_buf[ri];
            let task_id = self.task_states[task_idx].task_id;

            #[cfg(not(target_arch = "wasm32"))]
            let start = Instant::now();
            let mut last_instance_id = InstanceId::DEFAULT;

            // Iterate over program instances for this task.
            // Copy fields to locals before calling execute() to satisfy borrow checker.
            for pi in 0..self.program_instances.len() {
                if self.program_instances[pi].task_id != task_id {
                    continue;
                }
                let instance_id = self.program_instances[pi].instance_id;
                last_instance_id = instance_id;
                let entry_function_id = self.program_instances[pi].entry_function_id;
                let var_table_offset = self.program_instances[pi].var_table_offset;
                let var_table_count = self.program_instances[pi].var_table_count;

                let bytecode = self
                    .container
                    .code
                    .get_function_bytecode(entry_function_id)
                    .ok_or(FaultContext {
                        trap: Trap::InvalidFunctionId(entry_function_id),
                        task_id,
                        instance_id,
                    })?;

                let scope = VariableScope {
                    shared_globals_size: self.shared_globals_size,
                    instance_offset: var_table_offset,
                    instance_count: var_table_count,
                };

                execute(
                    bytecode,
                    self.container,
                    &mut self.stack,
                    &mut self.variables,
                    self.data_region,
                    self.temp_buf,
                    self.max_temp_buf_bytes,
                    &scope,
                    current_time_us,
                    0, // top-of-chain call: depth starts at zero
                    #[cfg(feature = "profiling")]
                    &mut self.profile,
                )
                .map_err(|trap| FaultContext {
                    trap,
                    task_id,
                    instance_id,
                })?;
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
            .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0) // init: RET_VOID
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
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x30,                   // ADD_I32
            0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
            0xB5,                   // RET_VOID
        ];

        ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(FunctionId::INIT, &[0xB5], 0, 2, 0) // init: RET_VOID
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
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]
        ];
        let c = ContainerBuilder::new()
            .num_variables(0)
            .add_i32_constant(1)
            .add_i32_constant(2)
            .add_function(FunctionId::INIT, &[0xB5], 0, 0, 0) // init: RET_VOID
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
        let c = single_function_container(&[0x30], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new().load(&c, &mut b).start().unwrap();

        assert_trap(&mut vm, Trap::StackUnderflow);
    }

    #[test]
    fn execute_when_invalid_constant_index_then_trap() {
        // 0 constants in pool, but bytecode references pool[0]
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
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
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x18, 0x05, 0x00,  // STORE_VAR_I32 var[5]
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
            0x10, 0x05, 0x00,  // LOAD_VAR_I32 var[5]
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
            0x01, 0x00, 0x00,        // LOAD_CONST_I32 pool[0] (3)
            0x01, 0x01, 0x00,        // LOAD_CONST_I32 pool[1] (7)
            0xB3, 0x02, 0x00, 0x01, 0x00,  // CALL function 2, var_offset=1
            0x18, 0x00, 0x00,        // STORE_VAR_I32 var[0] (result)
            0xB5,                    // RET_VOID
        ];
        #[rustfmt::skip]
        let func_bytecode: Vec<u8> = vec![
            0x10, 0x01, 0x00,  // LOAD_VAR_I32 var[1] (A - absolute index)
            0x10, 0x02, 0x00,  // LOAD_VAR_I32 var[2] (B - absolute index)
            0x30,              // ADD_I32
            0x18, 0x03, 0x00,  // STORE_VAR_I32 var[3] (return slot - absolute index)
            0x10, 0x03, 0x00,  // LOAD_VAR_I32 var[3]
            0xB4,              // RET
        ];

        let c = ContainerBuilder::new()
            .num_variables(4) // 1 program var + 3 function vars
            .add_i32_constant(3)
            .add_i32_constant(7)
            .add_function(FunctionId::INIT, &[0xB5], 0, 1, 0) // init
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
            .add_function(FunctionId::INIT, &[0xB5], 0, 0, 0)
            .add_function(FunctionId::SCAN, &[0xB5], 0, 0, 0)
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
