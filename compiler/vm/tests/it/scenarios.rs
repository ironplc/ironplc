//! Scenario integration tests for the VM.
//!
//! Phase 2: Multi-scan state accumulation and fault handling.
//! Phase 3: Multi-task execution, variable scope isolation, and watchdog
//! timing against an injected clock.

use crate::common::{load_and_start, single_function_container, ManualClock, VmBuffers};
use ironplc_container::{
    opcode, ContainerBuilder, FunctionId, InstanceId, ProgramInstanceEntry, TaskEntry, TaskId,
    TaskType, VarIndex,
};
use ironplc_vm::error::Trap;
use ironplc_vm::{Clock, Vm};
use rstest::rstest;

/// Builds a container for a program that increments var[0] by 1 each scan.
///
/// Program logic: x := x + 1
/// Bytecode:
///   LOAD_VAR_I32 var[0]      // push current x
///   LOAD_CONST_I32 pool[0]   // push 1
///   ADD_I32                   // x + 1
///   STORE_VAR_I32 var[0]      // write back
///   RET_VOID
fn counter_container() -> ironplc_container::Container {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x0C, 0x00, 0x00,  // LOAD_VAR_I32 var[0]
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (1)
        0x20,              // ADD_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];

    ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 1, 0) // init: RET_VOID
        .add_function(ironplc_container::FunctionId::new(1), &bytecode, 2, 1, 0) // scan: counter
        .init_function_id(ironplc_container::FunctionId::new(0))
        .entry_function_id(ironplc_container::FunctionId::new(1))
        .max_call_depth(1)
        .build()
}

#[test]
fn scenario_when_counter_increments_each_scan_then_accumulates() {
    let c = counter_container();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();

    for _ in 0..10 {
        vm.run_round(0).unwrap();
    }

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
}

#[test]
fn scenario_when_stop_then_scan_count_reflects_completed_rounds() {
    let c = counter_container();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();

    for _ in 0..5 {
        vm.run_round(0).unwrap();
    }

    let stopped = vm.stop();

    assert_eq!(stopped.scan_count(), 5);
    assert_eq!(stopped.read_variable(VarIndex::new(0)).unwrap(), 5);
}

/// One task with two program instances: the counter runs first, then a
/// fault program traps. The counter's write is visible on the faulted VM.
///
/// Setup: one task with two program instances.
/// - Program instance 0: counter (increments var[0] each scan)
/// - Program instance 1: always faults (invalid opcode 0xFF)
///
/// On each scan, the counter executes first (storing x+1), then
/// the fault program executes and traps. After one round:
/// - var[0] == 1 (the counter ran before the fault)
/// - The VM reports InvalidInstruction(0xFF)
#[test]
fn scenario_when_fault_during_scan_then_prior_writes_visible() {
    // Function 0: counter program (x := x + 1)
    #[rustfmt::skip]
    let counter_bytecode: Vec<u8> = vec![
        0x0C, 0x00, 0x00,  // LOAD_VAR_I32 var[0]
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (1)
        0x20,              // ADD_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];

    // Function 1: always faults
    let fault_bytecode: Vec<u8> = vec![0xFF]; // invalid opcode

    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 1, 0) // init: RET_VOID
        .add_function(
            ironplc_container::FunctionId::new(1),
            &counter_bytecode,
            2,
            1,
            0,
        ) // scan: counter
        .add_function(
            ironplc_container::FunctionId::new(2),
            &fault_bytecode,
            1,
            0,
            0,
        ) // scan: fault
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(program_instance(0, 0, 1, 0, 1))
        .add_program_instance(program_instance(1, 0, 2, 0, 1))
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    let result = vm.run_round(0);

    // The counter ran successfully before the fault program trapped
    assert!(result.is_err());
    let ctx = result.unwrap_err();
    assert_eq!(ctx.trap, Trap::InvalidInstruction(0xFF));
    assert_eq!(ctx.instance_id, InstanceId::new(1)); // fault was in program instance 1

    // The counter's write (var[0] = 1) is visible despite the fault
    let faulted = vm.fault(ctx);
    assert_eq!(faulted.read_variable(VarIndex::new(0)).unwrap(), 1);
}

#[test]
fn scenario_when_variables_read_after_fault_then_accessible() {
    // A program that stores 42 to var[0] then hits an invalid opcode
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xFF,              // invalid opcode — triggers fault
    ];

    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(42)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 1, 0) // init: RET_VOID
        .add_function(ironplc_container::FunctionId::new(1), &bytecode, 1, 1, 0) // scan: stores then faults
        .init_function_id(ironplc_container::FunctionId::new(0))
        .entry_function_id(ironplc_container::FunctionId::new(1))
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    let result = vm.run_round(0);

    assert!(result.is_err());
    let ctx = result.unwrap_err();
    assert_eq!(ctx.trap, Trap::InvalidInstruction(0xFF));

    // The store before the fault is visible on the faulted VM
    let faulted = vm.fault(ctx);
    assert_eq!(faulted.read_variable(VarIndex::new(0)).unwrap(), 42);
}

// Phase 3: Multi-task and variable scope tests.

/// Helper to build a freewheeling task entry.
fn freewheeling_task(task_id: u16, priority: u16, watchdog_us: u64) -> TaskEntry {
    TaskEntry {
        task_id: TaskId::new(task_id),
        priority,
        task_type: TaskType::Freewheeling,
        flags: 0x01, // enabled
        interval_us: 0,
        single_var_index: VarIndex::NO_SINGLE_VAR,
        watchdog_us,
        input_image_offset: 0,
        output_image_offset: 0,
        reserved: [0; 4],
    }
}

/// Helper to build a program instance entry.
fn program_instance(
    instance_id: u16,
    task_id: u16,
    function_id: u16,
    var_offset: u16,
    var_count: u16,
) -> ProgramInstanceEntry {
    ProgramInstanceEntry {
        instance_id: InstanceId::new(instance_id),
        task_id: TaskId::new(task_id),
        entry_function_id: FunctionId::new(function_id),
        var_table_offset: var_offset,
        var_table_count: var_count,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        init_function_id: FunctionId::INIT,
    }
}

/// Two freewheeling tasks with separate variable partitions both execute
/// in a single round.
///
/// Layout:
///   4 variables, shared globals: 0
///   Task 0 (priority 0) -> function 0 -> stores 10 to var[0]
///   Task 1 (priority 1) -> function 1 -> stores 20 to var[2]
#[test]
fn scenario_when_two_freewheeling_tasks_then_both_execute() {
    #[rustfmt::skip]
    let fn0_bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (10)
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    #[rustfmt::skip]
    let fn1_bytecode: Vec<u8> = vec![
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (20)
        0x10, 0x02, 0x00,  // STORE_VAR_I32 var[2]
        0x8C,              // RET_VOID
    ];

    let c = ContainerBuilder::new()
        .num_variables(4)
        .add_i32_constant(10)
        .add_i32_constant(20)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 0, 0) // init: RET_VOID
        .add_function(
            ironplc_container::FunctionId::new(1),
            &fn0_bytecode,
            1,
            2,
            0,
        ) // scan: task 0
        .add_function(
            ironplc_container::FunctionId::new(2),
            &fn1_bytecode,
            1,
            2,
            0,
        ) // scan: task 1
        .add_task(freewheeling_task(0, 0, 0))
        .add_task(freewheeling_task(1, 1, 0))
        .add_program_instance(program_instance(0, 0, 1, 0, 2))
        .add_program_instance(program_instance(1, 1, 2, 2, 2))
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10); // set by task 0
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 20); // set by task 1
}

/// Two tasks communicate through a shared global variable.
///
/// Layout:
///   4 variables, shared globals: 1 (var[0] is global)
///   Task 0 (priority 0) -> writes 99 to var[0] (global)
///   Task 1 (priority 1) -> reads var[0] (global), stores to var[2] (private)
///
/// Task 0 runs first (lower priority number), so task 1 sees the value.
#[test]
fn scenario_when_tasks_share_global_then_communication_works() {
    // Function 0: store 99 to var[0]
    #[rustfmt::skip]
    let fn0_bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (99)
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    // Function 1: copy var[0] to var[2]
    #[rustfmt::skip]
    let fn1_bytecode: Vec<u8> = vec![
        0x0C, 0x00, 0x00,  // LOAD_VAR_I32 var[0]   (global)
        0x10, 0x02, 0x00,  // STORE_VAR_I32 var[2]  (private)
        0x8C,              // RET_VOID
    ];

    let c = ContainerBuilder::new()
        .num_variables(4)
        .shared_globals_size(1)
        .add_i32_constant(99)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 0, 0) // init: RET_VOID
        .add_function(
            ironplc_container::FunctionId::new(1),
            &fn0_bytecode,
            1,
            1,
            0,
        ) // scan: task 0
        .add_function(
            ironplc_container::FunctionId::new(2),
            &fn1_bytecode,
            1,
            2,
            0,
        ) // scan: task 1
        .add_task(freewheeling_task(0, 0, 0))
        .add_task(freewheeling_task(1, 1, 0))
        .add_program_instance(program_instance(0, 0, 1, 1, 1)) // task 0: private [1,2)
        .add_program_instance(program_instance(1, 1, 2, 2, 2)) // task 1: private [2,4)
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 99); // global, written by task 0
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 99); // task 1 read the global
}

/// A one-task container whose scan function is `RET_VOID` and whose task
/// has the given watchdog limit.
fn watchdog_container(watchdog_us: u64) -> ironplc_container::Container {
    ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[opcode::RET_VOID], 0, 1, 0) // init
        .add_function(FunctionId::new(1), &[opcode::RET_VOID], 0, 1, 0) // scan
        .add_task(freewheeling_task(0, 0, watchdog_us))
        .add_program_instance(program_instance(0, 0, 1, 0, 1))
        .max_call_depth(1)
        .build()
}

/// The watchdog trips when the clock measures a task as running longer than
/// `watchdog_us`, and not when it measures exactly `watchdog_us`.
///
/// The clock advances by `step_us` between the readings taken before and
/// after the task, so the measured time is exact whatever the host's speed.
#[rstest]
#[case(100, None)]
#[case(101, Some(Trap::WatchdogTimeout(TaskId::new(0))))]
fn scenario_when_clock_measures_task_against_watchdog_then_traps_only_past_limit(
    #[case] step_us: u64,
    #[case] expected: Option<Trap>,
) {
    let c = watchdog_container(100);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.clock = ManualClock::stepping(step_us);

    let result = vm.run_round(0);

    assert_eq!(result.err().map(|ctx| ctx.trap), expected);
}

/// The time the clock measures for a task is what the scheduler records.
#[test]
fn scenario_when_clock_measures_task_then_records_execution_time() {
    let c = watchdog_container(0);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.clock = ManualClock::stepping(250);
        vm.run_round(0).unwrap();
    }

    assert_eq!(b.tasks[0].last_execute_us, 250);
    assert_eq!(b.tasks[0].max_execute_us, 250);
}

/// A clock whose every reading is 1 ms earlier than the one before.
struct BackwardsClock {
    now_us: u64,
}

impl Clock for BackwardsClock {
    fn now_us(&mut self) -> u64 {
        let now = self.now_us;
        self.now_us = self.now_us.saturating_sub(1_000);
        now
    }
}

/// A clock that goes backwards measures the task as taking no time: it
/// neither panics nor trips a 1 µs watchdog.
#[test]
fn scenario_when_clock_goes_backwards_then_measures_zero_without_watchdog_timeout() {
    let c = watchdog_container(1);
    let mut b = VmBuffers::from_container(&c);
    let result = {
        let mut vm = Vm::new().load(&c, &mut b).unwrap().start().unwrap();
        vm.run_round(0, &mut BackwardsClock { now_us: 1_000_000 })
    };

    assert!(result.is_ok());
    assert_eq!(b.tasks[0].last_execute_us, 0);
}

/// A task with watchdog_us = 0 (disabled) never triggers a watchdog timeout.
#[test]
fn scenario_when_watchdog_disabled_then_no_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];

    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(42)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0) // init: RET_VOID
        .add_function(FunctionId::new(1), &bytecode, 1, 1, 0) // scan
        .add_task(freewheeling_task(0, 0, 0)) // watchdog_us = 0 (disabled)
        .add_program_instance(program_instance(0, 0, 1, 0, 1))
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    // However long the task measures, a disabled watchdog never trips.
    vm.clock = ManualClock::stepping(1_000_000);
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);
}

/// A program instance that accesses a variable outside its scope is trapped.
///
/// Layout:
///   4 variables, shared globals: 0
///   Program instance 0 scope: variables [2, 4)
///   Bytecode: LOAD_VAR_I32 var[0] — index 0 is outside [2, 4)
#[test]
fn scenario_when_scope_violation_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x0C, 0x00, 0x00,  // LOAD_VAR_I32 var[0]  (outside scope)
    ];

    let c = ContainerBuilder::new()
        .num_variables(4)
        .add_function(ironplc_container::FunctionId::new(0), &[0x8C], 0, 0, 0) // init: RET_VOID
        .add_function(ironplc_container::FunctionId::new(1), &bytecode, 1, 2, 0) // scan: scope violation
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(program_instance(0, 0, 1, 2, 2)) // scope [2, 4)
        .max_call_depth(1)
        .build();

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    let result = vm.run_round(0);

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().trap,
        Trap::InvalidVariableIndex(VarIndex::new(0))
    );
}

/// Builds a container with one enabled cyclic task (id 0, no watchdog) that
/// runs one program instance whose scan function is `RET_VOID`.
///
/// `interval_us` comes straight from the task table, as it would from a
/// container file on disk, so callers can drive the scheduler's arithmetic
/// with any `u64`.
pub(crate) fn cyclic_task_container(interval_us: u64) -> ironplc_container::Container {
    ContainerBuilder::new()
        .num_variables(0)
        .add_function(FunctionId::new(0), &[0x8C], 0, 0, 0) // init: RET_VOID
        .add_function(FunctionId::new(1), &[0x8C], 0, 0, 0) // scan: RET_VOID
        .add_task(TaskEntry {
            task_id: TaskId::new(0),
            priority: 0,
            task_type: TaskType::Cyclic,
            flags: 0x01, // enabled
            interval_us,
            single_var_index: VarIndex::NO_SINGLE_VAR,
            watchdog_us: 0,
            input_image_offset: 0,
            output_image_offset: 0,
            reserved: [0; 4],
        })
        .add_program_instance(program_instance(0, 0, 1, 0, 0))
        .max_call_depth(1)
        .build()
}

/// A cyclic task whose next due time would pass `u64::MAX` saturates there
/// instead of overflowing: it is not due again until uptime reaches
/// `u64::MAX`.
#[test]
fn scenario_when_cyclic_due_time_exceeds_u64_then_saturates_without_panic() {
    let c = cyclic_task_container(1 << 63);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();

    vm.run_round(1 << 63).unwrap();
    assert_eq!(vm.scan_count(), 1);

    vm.run_round(u64::MAX - 1).unwrap();
    assert_eq!(vm.scan_count(), 1);

    vm.run_round(u64::MAX).unwrap();
    assert_eq!(vm.scan_count(), 2);
}

/// A program instance whose variable range ends at the last index (`0xFFFF`)
/// is checked without overflowing `instance_offset + instance_count`.
///
/// The container is not one codegen produces: the instance's range lies far
/// past the variable buffer, so the access passes the scope check and then
/// traps on the buffer bound instead of panicking in the scope check.
#[test]
fn scenario_when_instance_range_ends_at_last_index_then_traps_without_overflow() {
    let bytecode = [opcode::LOAD_VAR_I32, 0xFF, 0xFF, opcode::RET_VOID];
    let mut c = single_function_container(&bytecode, 1, &[]);
    c.task_table.programs[0].var_table_offset = 0xFFFF;
    c.task_table.programs[0].var_table_count = 1;
    c.task_table.shared_globals_size = 0;

    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    let result = vm.run_round(0);

    assert_eq!(
        result.err().map(|ctx| ctx.trap),
        Some(Trap::InvalidVariableIndex(VarIndex::new(0xFFFF)))
    );
}
