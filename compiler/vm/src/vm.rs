use ironplc_container::Container;

use crate::error::Trap;
use crate::scheduler::{ProgramInstanceState, TaskScheduler, TaskState};
use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use ironplc_container::opcode;

/// Context for a fault that occurred during task execution.
#[derive(Debug)]
pub struct FaultContext {
    pub trap: Trap,
    pub task_id: u16,
    pub instance_id: u16,
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

    /// Loads a container, using caller-provided buffers for stack, variables,
    /// task states, program instance states, and ready-task buffer.
    ///
    /// Populates `task_states` and `program_instances` from `container.task_table`.
    /// Consumes the empty VM and returns a ready VM.
    pub fn load<'a>(
        self,
        container: &'a Container,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
        task_states: &'a mut [TaskState],
        program_instances: &'a mut [ProgramInstanceState],
        ready_buf: &'a mut [usize],
    ) -> VmReady<'a> {
        // Populate task_states from the container's task table.
        for (i, t) in container.task_table.tasks.iter().enumerate() {
            if i < task_states.len() {
                task_states[i] = TaskState {
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
            if i < program_instances.len() {
                program_instances[i] = ProgramInstanceState {
                    instance_id: p.instance_id,
                    task_id: p.task_id,
                    entry_function_id: p.entry_function_id,
                    var_table_offset: p.var_table_offset,
                    var_table_count: p.var_table_count,
                };
            }
        }

        let stack = OperandStack::new(stack_buf);
        let variables = VariableTable::new(var_buf);

        VmReady {
            container,
            stack,
            variables,
            task_states,
            program_instances,
            ready_buf,
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
    task_states: &'a mut [TaskState],
    program_instances: &'a mut [ProgramInstanceState],
    ready_buf: &'a mut [usize],
}

impl<'a> VmReady<'a> {
    /// Starts the VM for scan execution.
    /// Consumes the ready VM and returns a running VM.
    pub fn start(self) -> VmRunning<'a> {
        let shared_globals_size = self.container.task_table.shared_globals_size;
        VmRunning {
            container: self.container,
            stack: self.stack,
            variables: self.variables,
            task_states: self.task_states,
            program_instances: self.program_instances,
            ready_buf: self.ready_buf,
            shared_globals_size,
            scan_count: 0,
            stop_requested: false,
        }
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
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
    task_states: &'a mut [TaskState],
    program_instances: &'a mut [ProgramInstanceState],
    ready_buf: &'a mut [usize],
    shared_globals_size: u16,
    scan_count: u64,
    stop_requested: bool,
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

        // Stub: INPUT_FREEZE (no-op)

        for ri in 0..ready_count {
            let task_idx = self.ready_buf[ri];
            let task_id = self.task_states[task_idx].task_id;

            // Iterate over program instances for this task.
            // Copy fields to locals before calling execute() to satisfy borrow checker.
            for pi in 0..self.program_instances.len() {
                if self.program_instances[pi].task_id != task_id {
                    continue;
                }
                let instance_id = self.program_instances[pi].instance_id;
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
                    &scope,
                )
                .map_err(|trap| FaultContext {
                    trap,
                    task_id,
                    instance_id,
                })?;
            }

            // Watchdog check stub: without Instant-based sub-round timing,
            // elapsed is always 0, so the watchdog never fires here.
            // Phase 4 will add real elapsed tracking.

            // Record execution with 0 elapsed (caller manages timing).
            let mut scheduler = TaskScheduler::new(self.task_states);
            scheduler.record_execution(task_idx, 0, current_time_us);
        }

        // Stub: OUTPUT_FLUSH (no-op)

        self.scan_count += 1;
        Ok(())
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots in the loaded container.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }

    /// Returns the number of completed scan cycles.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Requests the VM to stop after the current round.
    pub fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    /// Returns true if a stop has been requested.
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// Transitions to the stopped state (clean shutdown).
    pub fn stop(self) -> VmStopped<'a> {
        VmStopped {
            variables: self.variables,
            scan_count: self.scan_count,
        }
    }

    /// Transitions to the faulted state (trap occurred).
    pub fn fault(self, ctx: FaultContext) -> VmFaulted<'a> {
        VmFaulted {
            trap: ctx.trap,
            task_id: ctx.task_id,
            instance_id: ctx.instance_id,
            variables: self.variables,
        }
    }
}

/// A VM that has been cleanly stopped.
pub struct VmStopped<'a> {
    variables: VariableTable<'a>,
    scan_count: u64,
}

impl<'a> VmStopped<'a> {
    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }

    /// Returns the total number of completed scheduling rounds.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }
}

/// A VM that has stopped due to a trap.
pub struct VmFaulted<'a> {
    trap: Trap,
    task_id: u16,
    instance_id: u16,
    variables: VariableTable<'a>,
}

impl<'a> VmFaulted<'a> {
    /// Returns the trap that caused the fault.
    pub fn trap(&self) -> &Trap {
        &self.trap
    }

    /// Returns the task that was executing when the trap occurred.
    pub fn task_id(&self) -> u16 {
        self.task_id
    }

    /// Returns the program instance that was executing when the trap occurred.
    pub fn instance_id(&self) -> u16 {
        self.instance_id
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
    }
}

/// Executes bytecode until RET_VOID or a trap.
///
/// This is a free function so that the borrow checker can see
/// independent borrows of container (immutable) vs stack/variables
/// (mutable).
fn execute(
    bytecode: &[u8],
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    scope: &VariableScope,
) -> Result<(), Trap> {
    let mut pc: usize = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        pc += 1;

        match op {
            opcode::LOAD_CONST_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                let value = container
                    .constant_pool
                    .get_i32(index)
                    .map_err(|_| Trap::InvalidConstantIndex(index))?;
                stack.push(Slot::from_i32(value))?;
            }
            opcode::LOAD_VAR_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                scope.check_access(index)?;
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32 => {
                let index = read_u16_le(bytecode, &mut pc);
                scope.check_access(index)?;
                let slot = stack.pop()?;
                variables.store(index, slot)?;
            }
            opcode::ADD_I32 => {
                let b = stack.pop()?.as_i32();
                let a = stack.pop()?.as_i32();
                stack.push(Slot::from_i32(a.wrapping_add(b)))?;
            }
            opcode::SUB_I32 => {
                let b = stack.pop()?.as_i32();
                let a = stack.pop()?.as_i32();
                stack.push(Slot::from_i32(a.wrapping_sub(b)))?;
            }
            opcode::MUL_I32 => {
                let b = stack.pop()?.as_i32();
                let a = stack.pop()?.as_i32();
                stack.push(Slot::from_i32(a.wrapping_mul(b)))?;
            }
            opcode::RET_VOID => {
                return Ok(());
            }
            _ => {
                return Err(Trap::InvalidInstruction(op));
            }
        }
    }

    Ok(())
}

/// Reads a little-endian u16 from bytecode at pc, advancing pc by 2.
fn read_u16_le(bytecode: &[u8], pc: &mut usize) -> u16 {
    let value = u16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc += 2;
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::ContainerBuilder;

    /// Helper struct that allocates Vec-backed buffers for VM usage.
    struct VmBuffers {
        stack: Vec<Slot>,
        vars: Vec<Slot>,
        tasks: Vec<TaskState>,
        programs: Vec<ProgramInstanceState>,
        ready: Vec<usize>,
    }

    impl VmBuffers {
        fn from_container(container: &Container) -> Self {
            let header = &container.header;
            let task_count = container.task_table.tasks.len();
            let program_count = container.task_table.programs.len();
            VmBuffers {
                stack: vec![Slot::default(); header.max_stack_depth as usize],
                vars: vec![Slot::default(); header.num_variables as usize],
                tasks: vec![TaskState::default(); task_count],
                programs: vec![ProgramInstanceState::default(); program_count],
                ready: vec![0usize; task_count.max(1)],
            }
        }
    }

    /// Builds a container with one function from the given bytecode,
    /// with `num_vars` variables and the given constants.
    /// Uses a generous max_stack_depth (16) suitable for most tests.
    fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
        let mut builder = ContainerBuilder::new().num_variables(num_vars);
        for &c in constants {
            builder = builder.add_i32_constant(c);
        }
        builder.add_function(0, bytecode, 16, num_vars).build()
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
            .add_function(0, &bytecode, 2, 2)
            .build()
    }

    #[test]
    fn vm_load_when_valid_container_then_returns_ready() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let ready = Vm::new().load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        );

        // If this compiles, the VM is in the Ready state.
        // Verify we can read the initial variable values.
        assert_eq!(ready.read_variable(0).unwrap(), 0);
    }

    #[test]
    fn vm_run_round_when_steel_thread_then_x_is_10_y_is_42() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        vm.run_round(0).unwrap();

        assert_eq!(vm.read_variable(0).unwrap(), 10);
        assert_eq!(vm.read_variable(1).unwrap(), 42);
    }

    #[test]
    fn vm_run_round_when_invalid_opcode_then_trap() {
        let c = single_function_container(&[0xFF], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert_trap(&mut vm, Trap::InvalidInstruction(0xFF));
    }

    #[test]
    fn vm_request_stop_when_called_then_stop_requested() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert!(!vm.stop_requested());
        vm.request_stop();
        assert!(vm.stop_requested());
    }

    #[test]
    fn vm_stop_when_called_then_returns_stopped() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        let stopped = vm.stop();
        assert_eq!(stopped.read_variable(0).unwrap(), 0); // not yet executed
    }

    #[test]
    fn vm_fault_when_called_then_returns_faulted_with_context() {
        let c = steel_thread_container();
        let mut b = VmBuffers::from_container(&c);
        let vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        let ctx = FaultContext {
            trap: Trap::WatchdogTimeout(3),
            task_id: 3,
            instance_id: 1,
        };
        let faulted = vm.fault(ctx);
        assert_eq!(*faulted.trap(), Trap::WatchdogTimeout(3));
        assert_eq!(faulted.task_id(), 3);
        assert_eq!(faulted.instance_id(), 1);
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
            .add_function(0, &bytecode, 1, 0)
            .build();

        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        assert_trap(&mut vm, Trap::StackOverflow);
    }

    #[test]
    fn execute_when_stack_underflow_then_trap() {
        // ADD_I32 tries to pop 2 values from an empty stack
        let c = single_function_container(&[0x30], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

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
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert_trap(&mut vm, Trap::InvalidConstantIndex(0));
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
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert_trap(&mut vm, Trap::InvalidVariableIndex(5));
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
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert_trap(&mut vm, Trap::InvalidVariableIndex(5));
    }

    // Phase 1, Step 1.2: Execute edge-case tests

    #[test]
    fn execute_when_empty_bytecode_then_ok() {
        let c = single_function_container(&[], 0, &[]);
        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();

        assert!(vm.run_round(0).is_ok());
    }
}
