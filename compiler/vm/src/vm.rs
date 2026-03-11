use ironplc_container::{Container, STRING_HEADER_BYTES};

use crate::builtin;
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
    /// data region, temporary buffers, task states, program instance states,
    /// and ready-task buffer.
    ///
    /// Populates `task_states` and `program_instances` from `container.task_table`.
    /// Consumes the empty VM and returns a ready VM.
    #[allow(clippy::too_many_arguments)]
    pub fn load<'a>(
        self,
        container: &'a Container,
        stack_buf: &'a mut [Slot],
        var_buf: &'a mut [Slot],
        data_region_buf: &'a mut [u8],
        temp_buf: &'a mut [u8],
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
                    init_function_id: p.init_function_id,
                };
            }
        }

        let stack = OperandStack::new(stack_buf);
        let variables = VariableTable::new(var_buf);
        let max_temp_buf_bytes = container.header.max_temp_buf_bytes as usize;

        VmReady {
            container,
            stack,
            variables,
            data_region: data_region_buf,
            temp_buf,
            max_temp_buf_bytes,
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
    data_region: &'a mut [u8],
    temp_buf: &'a mut [u8],
    max_temp_buf_bytes: usize,
    task_states: &'a mut [TaskState],
    program_instances: &'a mut [ProgramInstanceState],
    ready_buf: &'a mut [usize],
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
        }
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: u16) -> Result<u64, Trap> {
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
                    self.data_region,
                    self.temp_buf,
                    self.max_temp_buf_bytes,
                    &scope,
                    current_time_us,
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

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: u16) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }

    /// Reads a variable value as an i64.
    pub fn read_variable_i64(&self, index: u16) -> Result<i64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i64())
    }

    /// Writes a variable value as an i32.
    pub fn write_variable(&mut self, index: u16, value: i32) -> Result<(), Trap> {
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

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: u16) -> Result<u64, Trap> {
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

    /// Reads a variable's raw 64-bit slot value.
    pub fn read_variable_raw(&self, index: u16) -> Result<u64, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_u64())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.variables.len()
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
        let index = read_u16_le($bytecode, &mut $pc);
        let value = $container
            .constant_pool
            .$get(index)
            .map_err(|_| Trap::InvalidConstantIndex(index))?;
        $stack.push(Slot::$from(value))?;
    }};
}

/// Executes bytecode until RET_VOID or a trap.
///
/// This is a free function so that the borrow checker can see
/// independent borrows of container (immutable) vs stack/variables
/// (mutable).
#[allow(clippy::too_many_arguments)]
fn execute(
    bytecode: &[u8],
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    scope: &VariableScope,
    current_time_us: u64,
) -> Result<(), Trap> {
    let mut pc: usize = 0;
    let mut next_temp_buf: u16 = 0;

    while pc < bytecode.len() {
        let op = bytecode[pc];
        pc += 1;

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
                let index = read_u16_le(bytecode, &mut pc);
                scope.check_access(index)?;
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32
            | opcode::STORE_VAR_I64
            | opcode::STORE_VAR_F32
            | opcode::STORE_VAR_F64 => {
                let index = read_u16_le(bytecode, &mut pc);
                scope.check_access(index)?;
                let slot = stack.pop()?;
                variables.store(index, slot)?;
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
                let offset = read_i16_le(bytecode, &mut pc);
                pc = (pc as isize + offset as isize) as usize;
            }
            opcode::JMP_IF_NOT => {
                let offset = read_i16_le(bytecode, &mut pc);
                let cond = stack.pop()?.as_i32();
                if cond == 0 {
                    pc = (pc as isize + offset as isize) as usize;
                }
            }
            opcode::BUILTIN => {
                let func_id = read_u16_le(bytecode, &mut pc);
                builtin::dispatch(func_id, stack)?;
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
            //      buffer holds the data. A bump allocator (`next_temp_buf`)
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
                let data_offset = read_u16_le(bytecode, &mut pc) as usize;
                let max_length = read_u16_le(bytecode, &mut pc);

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
                }
                str_write_header(data_region, data_offset, max_length, 0);
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
                let index = read_u16_le(bytecode, &mut pc);
                let str_bytes = container
                    .constant_pool
                    .get_str(index)
                    .map_err(|_| Trap::InvalidConstantIndex(index))?;

                let (buf_idx, buf_start) =
                    str_alloc_temp(&mut next_temp_buf, max_temp_buf_bytes, temp_buf.len())?;

                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (str_bytes.len() as u16).min(max_len);
                str_write_header(temp_buf, buf_start, max_len, cur_len);
                temp_buf[buf_start + STRING_HEADER_BYTES
                    ..buf_start + STRING_HEADER_BYTES + cur_len as usize]
                    .copy_from_slice(&str_bytes[..cur_len as usize]);

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
                let data_offset = read_u16_le(bytecode, &mut pc) as usize;
                let buf_idx = stack.pop()?.as_i32() as usize;

                let buf_start = buf_idx * max_temp_buf_bytes;
                if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                let src_cur_len = str_read_cur_len(temp_buf, buf_start);

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
                }
                let dest_max_len = str_read_max_len(data_region, data_offset);

                // Copy character data, truncating if source exceeds destination capacity.
                let copy_len = src_cur_len.min(dest_max_len) as usize;
                if data_offset + STRING_HEADER_BYTES + copy_len > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
                }
                let dst_start = data_offset + STRING_HEADER_BYTES;
                let src_start = buf_start + STRING_HEADER_BYTES;
                data_region[dst_start..dst_start + copy_len]
                    .copy_from_slice(&temp_buf[src_start..src_start + copy_len]);

                // Update destination cur_length.
                data_region[data_offset + 2..data_offset + STRING_HEADER_BYTES]
                    .copy_from_slice(&(copy_len as u16).to_le_bytes());
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
                let data_offset = read_u16_le(bytecode, &mut pc) as usize;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
                }
                let src_max_len = str_read_max_len(data_region, data_offset);
                let src_cur_len = str_read_cur_len(data_region, data_offset);
                // Defensive: never read more than max_length bytes.
                let read_len = src_cur_len.min(src_max_len) as usize;

                let (buf_idx, buf_start) =
                    str_alloc_temp(&mut next_temp_buf, max_temp_buf_bytes, temp_buf.len())?;

                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (read_len as u16).min(max_len);
                str_write_header(temp_buf, buf_start, max_len, cur_len);
                if data_offset + STRING_HEADER_BYTES + cur_len as usize > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
                }
                let dst_start = buf_start + STRING_HEADER_BYTES;
                let src_start = data_offset + STRING_HEADER_BYTES;
                temp_buf[dst_start..dst_start + cur_len as usize]
                    .copy_from_slice(&data_region[src_start..src_start + cur_len as usize]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // --- String function opcodes ---
            //
            // LEN_STR reads the cur_length field from a STRING variable's
            // header in the data region and pushes it as an i32.
            opcode::LEN_STR => {
                let data_offset = read_u16_le(bytecode, &mut pc) as usize;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u16));
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
                let in1_offset = read_u16_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u16_le(bytecode, &mut pc) as usize;

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u16));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                // Read IN2's current length.
                if in2_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in2_offset as u16));
                }
                let in2_len =
                    u16::from_le_bytes([data_region[in2_offset + 2], data_region[in2_offset + 3]])
                        as usize;

                let result = if in2_len == 0 || in2_len > in1_len {
                    // Empty search string or search string longer than haystack: not found.
                    0i32
                } else {
                    let in1_start = in1_offset + STRING_HEADER_BYTES;
                    let in2_start = in2_offset + STRING_HEADER_BYTES;
                    let in1_data = &data_region[in1_start..in1_start + in1_len];
                    let in2_data = &data_region[in2_start..in2_start + in2_len];

                    // Linear search for the first occurrence.
                    let mut found = 0i32;
                    for i in 0..=(in1_len - in2_len) {
                        if in1_data[i..i + in2_len] == *in2_data {
                            found = (i + 1) as i32; // 1-based position
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
                let in1_offset = read_u16_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u16_le(bytecode, &mut pc) as usize;

                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u16));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                // Read IN2's current length.
                if in2_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in2_offset as u16));
                }
                let in2_len =
                    u16::from_le_bytes([data_region[in2_offset + 2], data_region[in2_offset + 3]])
                        as usize;

                let in1_start = in1_offset + STRING_HEADER_BYTES;
                let in2_start = in2_offset + STRING_HEADER_BYTES;

                // Clamp P to valid range (1-based, minimum 1).
                let p = if p_val < 1 { 1usize } else { p_val as usize };
                // Clamp L to non-negative.
                let l = if l_val < 0 { 0usize } else { l_val as usize };

                // Convert P from 1-based to 0-based index.
                let start_idx = (p - 1).min(in1_len);
                // Number of characters to delete, clamped to remaining length.
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN2 + IN1[start_idx+delete_len..]
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + in2_len + suffix_len;

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Write result data: prefix + IN2 + suffix.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let mut write_pos = 0usize;

                // Copy prefix (IN1[0..start_idx]).
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }

                // Copy IN2.
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }

                // Copy suffix (IN1[suffix_start..]).
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + suffix_start + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // INSERT_STR: Insert IN2 into IN1 after position P.
            // Pops P from stack, pushes buf_idx.
            opcode::INSERT_STR => {
                let in1_offset = read_u16_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u16_le(bytecode, &mut pc) as usize;

                let p_val = stack.pop()?.as_i32();

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u16));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                // Read IN2's current length.
                if in2_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in2_offset as u16));
                }
                let in2_len =
                    u16::from_le_bytes([data_region[in2_offset + 2], data_region[in2_offset + 3]])
                        as usize;

                let in1_start = in1_offset + STRING_HEADER_BYTES;
                let in2_start = in2_offset + STRING_HEADER_BYTES;

                // Clamp P to valid range (1-based, minimum 0 means insert at start).
                let p = if p_val < 0 { 0usize } else { p_val as usize };

                // Insert point: after position P (0-based index = P).
                let insert_idx = p.min(in1_len);

                // Result = IN1[0..insert_idx] + IN2 + IN1[insert_idx..]
                let prefix_len = insert_idx;
                let suffix_len = in1_len - insert_idx;
                let result_len = prefix_len + in2_len + suffix_len;

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Write result data: prefix + IN2 + suffix.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let mut write_pos = 0usize;

                // Copy prefix (IN1[0..insert_idx]).
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }

                // Copy IN2.
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }

                // Copy suffix (IN1[insert_idx..]).
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + insert_idx + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // DELETE_STR: Delete L characters from IN1 starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::DELETE_STR => {
                let in1_offset = read_u16_le(bytecode, &mut pc) as usize;

                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u16));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                let in1_start = in1_offset + STRING_HEADER_BYTES;

                // Clamp P to valid range (1-based, minimum 1).
                let p = if p_val < 1 { 1usize } else { p_val as usize };
                // Clamp L to non-negative.
                let l = if l_val < 0 { 0usize } else { l_val as usize };

                // Convert P from 1-based to 0-based index.
                let start_idx = (p - 1).min(in1_len);
                // Number of characters to delete, clamped to remaining length.
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN1[start_idx+delete_len..]
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + suffix_len;

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Write result data: prefix + suffix.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let mut write_pos = 0usize;

                // Copy prefix (IN1[0..start_idx]).
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }

                // Copy suffix (IN1[suffix_start..]).
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + suffix_start + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // LEFT_STR: Return the leftmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::LEFT_STR => {
                let in_offset = read_u16_le(bytecode, &mut pc) as usize;

                let l_val = stack.pop()?.as_i32();

                // Read IN's current length.
                if in_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in_offset as u16));
                }
                let in_len =
                    u16::from_le_bytes([data_region[in_offset + 2], data_region[in_offset + 3]])
                        as usize;

                let in_start = in_offset + STRING_HEADER_BYTES;

                // Clamp L to non-negative.
                let l = if l_val < 0 { 0usize } else { l_val as usize };

                // Result length is min(L, in_len).
                let result_len = l.min(in_len);

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Copy leftmost characters from IN.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let copy_len = cur_len as usize;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[in_start..in_start + copy_len]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // RIGHT_STR: Return the rightmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::RIGHT_STR => {
                let in_offset = read_u16_le(bytecode, &mut pc) as usize;

                let l_val = stack.pop()?.as_i32();

                // Read IN's current length.
                if in_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in_offset as u16));
                }
                let in_len =
                    u16::from_le_bytes([data_region[in_offset + 2], data_region[in_offset + 3]])
                        as usize;

                let in_start = in_offset + STRING_HEADER_BYTES;

                // Clamp L to non-negative.
                let l = if l_val < 0 { 0usize } else { l_val as usize };

                // Result length is min(L, in_len).
                let result_len = l.min(in_len);

                // Start index within IN for the rightmost characters.
                let src_start = in_len - result_len;

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Copy rightmost characters from IN.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let copy_len = cur_len as usize;
                let src = in_start + src_start;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[src..src + copy_len]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // MID_STR: Return L characters from IN starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::MID_STR => {
                let in_offset = read_u16_le(bytecode, &mut pc) as usize;

                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                // Read IN's current length.
                if in_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in_offset as u16));
                }
                let in_len =
                    u16::from_le_bytes([data_region[in_offset + 2], data_region[in_offset + 3]])
                        as usize;

                let in_start = in_offset + STRING_HEADER_BYTES;

                // Clamp P to valid range (1-based, minimum 1).
                let p = if p_val < 1 { 1usize } else { p_val as usize };
                // Clamp L to non-negative.
                let l = if l_val < 0 { 0usize } else { l_val as usize };

                // Convert P from 1-based to 0-based index.
                let start_idx = (p - 1).min(in_len);
                // Number of characters to extract, clamped to remaining length.
                let result_len = l.min(in_len - start_idx);

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Copy characters from IN starting at start_idx.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let copy_len = cur_len as usize;
                let src = in_start + start_idx;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[src..src + copy_len]);

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            // CONCAT_STR: Concatenate IN1 and IN2.
            // Pushes buf_idx.
            opcode::CONCAT_STR => {
                let in1_offset = read_u16_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u16_le(bytecode, &mut pc) as usize;

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u16));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                // Read IN2's current length.
                if in2_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in2_offset as u16));
                }
                let in2_len =
                    u16::from_le_bytes([data_region[in2_offset + 2], data_region[in2_offset + 3]])
                        as usize;

                let in1_start = in1_offset + STRING_HEADER_BYTES;
                let in2_start = in2_offset + STRING_HEADER_BYTES;

                let result_len = in1_len + in2_len;

                // Allocate a temp buffer.
                if max_temp_buf_bytes == 0 {
                    return Err(Trap::TempBufferExhausted);
                }
                let buf_idx = next_temp_buf;
                let buf_start = buf_idx as usize * max_temp_buf_bytes;
                let buf_end = buf_start + max_temp_buf_bytes;
                if buf_end > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                next_temp_buf = next_temp_buf.wrapping_add(1);

                // Clamp result to temp buffer capacity.
                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (result_len as u16).min(max_len);

                // Write header.
                temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
                temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
                    .copy_from_slice(&cur_len.to_le_bytes());

                // Write result data: IN1 + IN2.
                let data_start = buf_start + STRING_HEADER_BYTES;
                let mut write_pos = 0usize;

                // Copy IN1.
                let in1_copy = in1_len.min(cur_len as usize);
                for i in 0..in1_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }

                // Copy IN2.
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(buf_idx as i32))?;
            }
            opcode::POP => {
                stack.pop()?;
            }
            // --- Function block opcodes ---
            opcode::FB_LOAD_INSTANCE => {
                let var_index = read_u16_le(bytecode, &mut pc);
                scope.check_access(var_index)?;
                let slot = variables.load(var_index)?;
                stack.push(slot)?;
            }
            opcode::FB_STORE_PARAM => {
                let field = bytecode[pc] as u16;
                pc += 1;
                let value = stack.pop()?;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u16));
                }
                data_region[offset..offset + 8].copy_from_slice(&value.as_i64().to_le_bytes());
            }
            opcode::FB_LOAD_PARAM => {
                let field = bytecode[pc] as u16;
                pc += 1;
                let fb_ref = stack.peek()?.as_i32() as u32;
                let offset = fb_ref as usize + field as usize * 8;
                if offset + 8 > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(offset as u16));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&data_region[offset..offset + 8]);
                stack.push(Slot::from_i64(i64::from_le_bytes(buf)))?;
            }
            opcode::FB_CALL => {
                let type_id = read_u16_le(bytecode, &mut pc);
                let fb_ref = stack.peek()?.as_i32() as u32;
                let instance_start = fb_ref as usize;
                match type_id {
                    opcode::fb_type::TON | opcode::fb_type::TOF | opcode::fb_type::TP => {
                        let instance_size = crate::intrinsic::TIMER_INSTANCE_FIELDS * 8;
                        let instance_end = instance_start + instance_size;
                        if instance_end > data_region.len() {
                            return Err(Trap::DataRegionOutOfBounds(instance_start as u16));
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
                    _ => return Err(Trap::InvalidFbTypeId(type_id)),
                }
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

/// Reads a little-endian i16 from bytecode at pc, advancing pc by 2.
fn read_i16_le(bytecode: &[u8], pc: &mut usize) -> i16 {
    let value = i16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc += 2;
    value
}

/// Read max_length from a string header at `offset` in `buf`.
fn str_read_max_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

/// Read cur_length from a string header at `offset` in `buf`.
fn str_read_cur_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset + 2], buf[offset + 3]])
}

/// Write a string header (max_length, cur_length) at `offset` in `buf`.
fn str_write_header(buf: &mut [u8], offset: usize, max_len: u16, cur_len: u16) {
    buf[offset..offset + 2].copy_from_slice(&max_len.to_le_bytes());
    buf[offset + 2..offset + STRING_HEADER_BYTES].copy_from_slice(&cur_len.to_le_bytes());
}

/// Allocate the next temp buffer slot. Returns (buf_idx, buf_start).
fn str_alloc_temp(
    next_temp_buf: &mut u16,
    max_temp_buf_bytes: usize,
    temp_buf_len: usize,
) -> Result<(usize, usize), Trap> {
    if max_temp_buf_bytes == 0 {
        return Err(Trap::TempBufferExhausted);
    }
    let buf_idx = *next_temp_buf as usize;
    let buf_start = buf_idx * max_temp_buf_bytes;
    if buf_start + max_temp_buf_bytes > temp_buf_len {
        return Err(Trap::TempBufferExhausted);
    }
    *next_temp_buf = next_temp_buf.wrapping_add(1);
    Ok((buf_idx, buf_start))
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
            .add_function(0, &[0xB5], 0, num_vars) // init: RET_VOID
            .add_function(1, bytecode, 16, num_vars) // scan: test bytecode
            .init_function_id(0)
            .entry_function_id(1)
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
            .add_function(0, &[0xB5], 0, 2) // init: RET_VOID
            .add_function(1, &bytecode, 2, 2) // scan: program body
            .init_function_id(0)
            .entry_function_id(1)
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
            &mut b.data_region,
            &mut b.temp_buf,
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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
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
            .add_function(0, &[0xB5], 0, 0) // init: RET_VOID
            .add_function(1, &bytecode, 1, 0) // scan: triggers overflow
            .init_function_id(0)
            .entry_function_id(1)
            .build();

        let mut b = VmBuffers::from_container(&c);
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

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
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();

        assert!(vm.run_round(0).is_ok());
    }
}
