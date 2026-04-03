use ironplc_container::{
    ConstantIndex, Container, FbTypeId, FunctionId, InstanceId, TaskId, VarIndex,
    STRING_HEADER_BYTES,
};

use crate::buffers::VmBuffers;
use crate::builtin;
use crate::error::Trap;
use crate::scheduler::{ProgramInstanceState, TaskScheduler, TaskState};
use crate::stack::OperandStack;
use crate::string_ops;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use core::fmt::Write as FmtWrite;
use ironplc_container::opcode;

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
}

/// A VM that has stopped due to a trap.
pub struct VmFaulted<'a> {
    trap: Trap,
    task_id: TaskId,
    instance_id: InstanceId,
    variables: VariableTable<'a>,
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
            .$get(ConstantIndex::new(index))
            .map_err(|_| Trap::InvalidConstantIndex(ConstantIndex::new(index)))?;
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
    let mut temp_alloc = string_ops::TempBufAllocator::new(max_temp_buf_bytes);

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
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                scope.check_access(index)?;
                let slot = variables.load(index)?;
                stack.push(slot)?;
            }
            opcode::STORE_VAR_I32
            | opcode::STORE_VAR_I64
            | opcode::STORE_VAR_F32
            | opcode::STORE_VAR_F64 => {
                let index = VarIndex::new(read_u16_le(bytecode, &mut pc));
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
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len())?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len);
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
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len())?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len);
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
                        let (buf_idx, buf_start) = {
                            let slot = temp_alloc.alloc(temp_buf.len())?;
                            (slot.buf_idx as usize, slot.buf_start)
                        };
                        let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                        let cur_len = (bytes.len() as u16).min(max_len);
                        string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len);
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
                    _ => builtin::dispatch(func_id, stack)?,
                }
            }
            opcode::CALL => {
                let func_id = read_u16_le(bytecode, &mut pc);
                let var_offset = read_u16_le(bytecode, &mut pc);
                let func = container
                    .code
                    .get_function(FunctionId::new(func_id))
                    .ok_or(Trap::InvalidFunctionId(FunctionId::new(func_id)))?;
                let func_bytecode = container
                    .code
                    .get_function_bytecode(FunctionId::new(func_id))
                    .ok_or(Trap::InvalidFunctionId(FunctionId::new(func_id)))?;

                let func_scope = VariableScope {
                    shared_globals_size: scope.shared_globals_size,
                    instance_offset: var_offset,
                    instance_count: func.num_locals,
                };

                // Pop arguments from stack into function's parameter slots (reverse order).
                for i in (0..func.num_params).rev() {
                    let val = stack.pop()?;
                    variables.store(VarIndex::new(var_offset + i), val)?;
                }

                // TODO: Replace Rust-recursive execute() with an iterative dispatch
                // loop that manages its own return-address stack, so call depth is
                // controlled by the VM rather than Rust's thread stack.
                // Recursively execute the function body.
                execute(
                    func_bytecode,
                    container,
                    stack,
                    variables,
                    data_region,
                    temp_buf,
                    max_temp_buf_bytes,
                    &func_scope,
                    current_time_us,
                )?;
            }
            opcode::RET => {
                // Return value is already on the stack; just return from execute().
                return Ok(());
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
                let data_offset = read_u32_le(bytecode, &mut pc) as usize;
                let max_length = read_u16_le(bytecode, &mut pc);

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                string_ops::str_write_header(data_region, data_offset, max_length, 0);
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
                    .get_str(ConstantIndex::new(index))
                    .map_err(|_| Trap::InvalidConstantIndex(ConstantIndex::new(index)))?;

                let (buf_idx, buf_start) = {
                    let slot = temp_alloc.alloc(temp_buf.len())?;
                    (slot.buf_idx as usize, slot.buf_start)
                };

                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (str_bytes.len() as u16).min(max_len);
                string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len);
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
                let data_offset = read_u32_le(bytecode, &mut pc) as usize;
                let buf_idx = stack.pop()?.as_i32() as usize;

                let buf_start = buf_idx * max_temp_buf_bytes;
                if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
                    return Err(Trap::TempBufferExhausted);
                }
                let src_cur_len = string_ops::str_read_cur_len(temp_buf, buf_start);

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let dest_max_len = string_ops::str_read_max_len(data_region, data_offset);

                // Copy character data, truncating if source exceeds destination capacity.
                let copy_len = src_cur_len.min(dest_max_len) as usize;
                if data_offset + STRING_HEADER_BYTES + copy_len > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
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
                let data_offset = read_u32_le(bytecode, &mut pc) as usize;

                if data_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
                }
                let src_max_len = string_ops::str_read_max_len(data_region, data_offset);
                let src_cur_len = string_ops::str_read_cur_len(data_region, data_offset);
                // Defensive: never read more than max_length bytes.
                let read_len = src_cur_len.min(src_max_len) as usize;

                let (buf_idx, buf_start) = {
                    let slot = temp_alloc.alloc(temp_buf.len())?;
                    (slot.buf_idx as usize, slot.buf_start)
                };

                let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (read_len as u16).min(max_len);
                string_ops::str_write_header(temp_buf, buf_start, max_len, cur_len);
                if data_offset + STRING_HEADER_BYTES + cur_len as usize > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
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
                let data_offset = read_u32_le(bytecode, &mut pc) as usize;

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
                let in1_offset = read_u32_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc) as usize;

                // Read IN1's current length.
                if in1_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in1_offset as u32));
                }
                let in1_len =
                    u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]])
                        as usize;

                // Read IN2's current length.
                if in2_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(in2_offset as u32));
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
                let in1_offset = read_u32_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc) as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in1_len, in1_start) = string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start) = string_ops::read_string_header(data_region, in2_offset)?;

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in1_len);
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN2 + IN1[start_idx+delete_len..]
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + in2_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                // Write result data: prefix + IN2 + suffix.
                let mut write_pos = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + suffix_start + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // INSERT_STR: Insert IN2 into IN1 after position P.
            // Pops P from stack, pushes buf_idx.
            opcode::INSERT_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc) as usize;
                let p_val = stack.pop()?.as_i32();

                let (in1_len, in1_start) = string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start) = string_ops::read_string_header(data_region, in2_offset)?;

                let p = if p_val < 0 { 0usize } else { p_val as usize };
                let insert_idx = p.min(in1_len);

                // Result = IN1[0..insert_idx] + IN2 + IN1[insert_idx..]
                let prefix_len = insert_idx;
                let suffix_len = in1_len - insert_idx;
                let result_len = prefix_len + in2_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                // Write result data: prefix + IN2 + suffix.
                let mut write_pos = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + insert_idx + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // DELETE_STR: Delete L characters from IN1 starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::DELETE_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc) as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in1_len, in1_start) = string_ops::read_string_header(data_region, in1_offset)?;

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in1_len);
                let delete_len = l.min(in1_len - start_idx);

                // Result = IN1[0..start_idx] + IN1[start_idx+delete_len..]
                let prefix_len = start_idx;
                let suffix_start = start_idx + delete_len;
                let suffix_len = in1_len - suffix_start;
                let result_len = prefix_len + suffix_len;

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                // Write result data: prefix + suffix.
                let mut write_pos = 0usize;
                let prefix_copy = prefix_len.min(cur_len as usize);
                for i in 0..prefix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }
                let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..suffix_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + suffix_start + i];
                    write_pos += 1;
                }

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // LEFT_STR: Return the leftmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::LEFT_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc) as usize;
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start) = string_ops::read_string_header(data_region, in_offset)?;

                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let result_len = l.min(in_len);

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                let copy_len = cur_len as usize;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[in_start..in_start + copy_len]);

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // RIGHT_STR: Return the rightmost L characters of IN.
            // Pops L from stack, pushes buf_idx.
            opcode::RIGHT_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc) as usize;
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start) = string_ops::read_string_header(data_region, in_offset)?;

                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let result_len = l.min(in_len);
                let src_start = in_len - result_len;

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                let copy_len = cur_len as usize;
                let src = in_start + src_start;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[src..src + copy_len]);

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // MID_STR: Return L characters from IN starting at position P.
            // Pops P then L from stack, pushes buf_idx.
            opcode::MID_STR => {
                let in_offset = read_u32_le(bytecode, &mut pc) as usize;
                let p_val = stack.pop()?.as_i32();
                let l_val = stack.pop()?.as_i32();

                let (in_len, in_start) = string_ops::read_string_header(data_region, in_offset)?;

                let p = if p_val < 1 { 1usize } else { p_val as usize };
                let l = if l_val < 0 { 0usize } else { l_val as usize };
                let start_idx = (p - 1).min(in_len);
                let result_len = l.min(in_len - start_idx);

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                let copy_len = cur_len as usize;
                let src = in_start + start_idx;
                temp_buf[data_start..data_start + copy_len]
                    .copy_from_slice(&data_region[src..src + copy_len]);

                stack.push(Slot::from_i32(slot.buf_idx as i32))?;
            }
            // CONCAT_STR: Concatenate IN1 and IN2.
            // Pushes buf_idx.
            opcode::CONCAT_STR => {
                let in1_offset = read_u32_le(bytecode, &mut pc) as usize;
                let in2_offset = read_u32_le(bytecode, &mut pc) as usize;

                let (in1_len, in1_start) = string_ops::read_string_header(data_region, in1_offset)?;
                let (in2_len, in2_start) = string_ops::read_string_header(data_region, in2_offset)?;

                let result_len = in1_len + in2_len;

                let slot = temp_alloc.alloc(temp_buf.len())?;

                let (cur_len, data_start) = string_ops::write_string_header(
                    temp_buf,
                    slot.buf_start,
                    slot.max_len,
                    result_len,
                );

                // Write result data: IN1 + IN2.
                let mut write_pos = 0usize;
                let in1_copy = in1_len.min(cur_len as usize);
                for i in 0..in1_copy {
                    temp_buf[data_start + write_pos] = data_region[in1_start + i];
                    write_pos += 1;
                }
                let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
                for i in 0..in2_copy {
                    temp_buf[data_start + write_pos] = data_region[in2_start + i];
                    write_pos += 1;
                }

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
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);

                let desc = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;
                let total_elements = desc.total_elements;
                let max_str_len = desc.element_extra;
                let stride = STRING_HEADER_BYTES + max_str_len as usize;

                scope.check_access(var_index)?;
                let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;

                for i in 0..total_elements as usize {
                    let elem_offset = base_offset + i * stride;
                    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
                        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                    }
                    string_ops::str_write_header(data_region, elem_offset, max_str_len, 0);
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
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
                let index_slot = stack.pop()?;
                let index_i64 = index_slot.as_i64();

                let desc = container
                    .type_section
                    .as_ref()
                    .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
                    .ok_or(Trap::InvalidVariableIndex(var_index))?;
                let total_elements = desc.total_elements;
                let max_str_len = desc.element_extra;
                let stride = STRING_HEADER_BYTES + max_str_len as usize;

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
                let src_cur_len = string_ops::str_read_cur_len(data_region, elem_offset);
                let read_len = src_cur_len.min(max_str_len) as usize;

                let (buf_idx, buf_start) = {
                    let slot = temp_alloc.alloc(temp_buf.len())?;
                    (slot.buf_idx as usize, slot.buf_start)
                };

                let buf_max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
                let cur_len = (read_len as u16).min(buf_max_len);
                string_ops::str_write_header(temp_buf, buf_start, buf_max_len, cur_len);

                if elem_offset + STRING_HEADER_BYTES + cur_len as usize > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                let dst_start = buf_start + STRING_HEADER_BYTES;
                let src_start = elem_offset + STRING_HEADER_BYTES;
                temp_buf[dst_start..dst_start + cur_len as usize]
                    .copy_from_slice(&data_region[src_start..src_start + cur_len as usize]);

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
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
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
                let stride = STRING_HEADER_BYTES + max_str_len as usize;

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
                let src_cur_len = string_ops::str_read_cur_len(temp_buf, buf_start);

                if elem_offset + STRING_HEADER_BYTES > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }

                // Copy, truncating to max_str_len per IEC 61131-3 semantics.
                let copy_len = src_cur_len.min(max_str_len) as usize;
                if elem_offset + STRING_HEADER_BYTES + copy_len > data_region.len() {
                    return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
                }
                let dst_start = elem_offset + STRING_HEADER_BYTES;
                let src_start = buf_start + STRING_HEADER_BYTES;
                data_region[dst_start..dst_start + copy_len]
                    .copy_from_slice(&temp_buf[src_start..src_start + copy_len]);

                // Update destination cur_length.
                data_region[elem_offset + 2..elem_offset + STRING_HEADER_BYTES]
                    .copy_from_slice(&(copy_len as u16).to_le_bytes());
            }

            opcode::POP => {
                stack.pop()?;
            }
            // --- Function block opcodes ---
            opcode::FB_LOAD_INSTANCE => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
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
                    return Err(Trap::DataRegionOutOfBounds(offset as u32));
                }
                data_region[offset..offset + 8].copy_from_slice(&value.as_i64().to_le_bytes());
            }
            opcode::FB_LOAD_PARAM => {
                let field = bytecode[pc] as u16;
                pc += 1;
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
                let type_id = read_u16_le(bytecode, &mut pc);
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
                        let num_fields = user_fb.num_fields as usize;

                        let func = container
                            .code
                            .get_function(func_id)
                            .ok_or(Trap::InvalidFunctionId(func_id))?;
                        let func_bytecode = container
                            .code
                            .get_function_bytecode(func_id)
                            .ok_or(Trap::InvalidFunctionId(func_id))?;

                        // Copy-in: data region fields -> variable table slots.
                        for i in 0..num_fields {
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

                        // Execute the FB body.
                        let func_scope = VariableScope {
                            shared_globals_size: scope.shared_globals_size,
                            instance_offset: var_off,
                            instance_count: func.num_locals,
                        };
                        execute(
                            func_bytecode,
                            container,
                            stack,
                            variables,
                            data_region,
                            temp_buf,
                            max_temp_buf_bytes,
                            &func_scope,
                            current_time_us,
                        )?;

                        // Copy-out: variable table slots -> data region fields.
                        for i in 0..num_fields {
                            let offset = instance_start + i * 8;
                            let val = variables.load(VarIndex::new(var_off + i as u16))?;
                            data_region[offset..offset + 8]
                                .copy_from_slice(&val.as_i64().to_le_bytes());
                        }
                    }
                }
            }
            // --- Array opcodes ---
            opcode::LOAD_ARRAY => {
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
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
                let var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
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
                let ref_var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
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
                let ref_var_index = VarIndex::new(read_u16_le(bytecode, &mut pc));
                let desc_index = read_u16_le(bytecode, &mut pc);
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

/// Reads a little-endian u32 from bytecode at pc, advancing pc by 4.
fn read_u32_le(bytecode: &[u8], pc: &mut usize) -> u32 {
    let value = u32::from_le_bytes([
        bytecode[*pc],
        bytecode[*pc + 1],
        bytecode[*pc + 2],
        bytecode[*pc + 3],
    ]);
    *pc += 4;
    value
}

/// Reads a little-endian i16 from bytecode at pc, advancing pc by 2.
fn read_i16_le(bytecode: &[u8], pc: &mut usize) -> i16 {
    let value = i16::from_le_bytes([bytecode[*pc], bytecode[*pc + 1]]);
    *pc += 2;
    value
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
}
