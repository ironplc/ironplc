//! Scenario integration tests for the VM.
//!
//! Phase 2: Multi-scan state accumulation and fault handling.
//! Phase 3: Multi-task execution, variable scope isolation, and watchdog.

use ironplc_container::{ContainerBuilder, ProgramInstanceEntry, TaskEntry, TaskType};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

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
        0x10, 0x00, 0x00,  // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (1)
        0x30,              // ADD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];

    ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(0, &bytecode, 2, 1)
        .build()
}

#[test]
fn scenario_when_counter_increments_each_scan_then_accumulates() {
    let mut vm = Vm::new().load(counter_container()).start();

    for _ in 0..10 {
        vm.run_round().unwrap();
    }

    assert_eq!(vm.read_variable(0).unwrap(), 10);
}

#[test]
fn scenario_when_stop_then_scan_count_reflects_completed_rounds() {
    let mut vm = Vm::new().load(counter_container()).start();

    for _ in 0..5 {
        vm.run_round().unwrap();
    }

    let stopped = vm.stop();

    assert_eq!(stopped.scan_count(), 5);
    assert_eq!(stopped.read_variable(0).unwrap(), 5);
}

/// A program that stores 42 to var[0] then faults. After the fault,
/// we run two successful scans of a counter first, then fault on scan 3.
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
        0x10, 0x00, 0x00,  // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (1)
        0x30,              // ADD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];

    // Function 1: always faults
    let fault_bytecode: Vec<u8> = vec![0xFF]; // invalid opcode

    let task = TaskEntry {
        task_id: 0,
        priority: 0,
        task_type: TaskType::Freewheeling,
        flags: 0x01, // enabled
        interval_us: 0,
        single_var_index: 0xFFFF,
        watchdog_us: 0,
        input_image_offset: 0,
        output_image_offset: 0,
        reserved: [0; 4],
    };

    // Program instance 0 runs the counter (function 0)
    let prog0 = ProgramInstanceEntry {
        instance_id: 0,
        task_id: 0,
        entry_function_id: 0,
        var_table_offset: 0,
        var_table_count: 1,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        reserved: 0,
    };

    // Program instance 1 runs the fault program (function 1)
    let prog1 = ProgramInstanceEntry {
        instance_id: 1,
        task_id: 0,
        entry_function_id: 1,
        var_table_offset: 0,
        var_table_count: 1,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        reserved: 0,
    };

    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(0, &counter_bytecode, 2, 1)
        .add_function(1, &fault_bytecode, 1, 0)
        .add_task(task)
        .add_program_instance(prog0)
        .add_program_instance(prog1)
        .build();

    let mut vm = Vm::new().load(container).start();
    let result = vm.run_round();

    // The counter ran successfully before the fault program trapped
    assert!(result.is_err());
    let ctx = result.unwrap_err();
    assert_eq!(ctx.trap, Trap::InvalidInstruction(0xFF));
    assert_eq!(ctx.instance_id, 1); // fault was in program instance 1

    // The counter's write (var[0] = 1) is visible despite the fault
    let faulted = vm.fault(ctx);
    assert_eq!(faulted.read_variable(0).unwrap(), 1);
}

#[test]
fn scenario_when_variables_read_after_fault_then_accessible() {
    // A program that stores 42 to var[0] then hits an invalid opcode
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xFF,              // invalid opcode — triggers fault
    ];

    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(42)
        .add_function(0, &bytecode, 1, 1)
        .build();

    let mut vm = Vm::new().load(container).start();
    let result = vm.run_round();

    assert!(result.is_err());
    let ctx = result.unwrap_err();
    assert_eq!(ctx.trap, Trap::InvalidInstruction(0xFF));

    // The store before the fault is visible on the faulted VM
    let faulted = vm.fault(ctx);
    assert_eq!(faulted.read_variable(0).unwrap(), 42);
}

// Phase 3: Multi-task and variable scope tests.

/// Helper to build a freewheeling task entry.
fn freewheeling_task(task_id: u16, priority: u16, watchdog_us: u64) -> TaskEntry {
    TaskEntry {
        task_id,
        priority,
        task_type: TaskType::Freewheeling,
        flags: 0x01, // enabled
        interval_us: 0,
        single_var_index: 0xFFFF,
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
        instance_id,
        task_id,
        entry_function_id: function_id,
        var_table_offset: var_offset,
        var_table_count: var_count,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        reserved: 0,
    }
}

/// Two freewheeling tasks with separate variable partitions both execute
/// in a single round.
///
/// Layout:
///   4 variables, shared globals: 0
///   Task 0 (priority 0) → function 0 → stores 10 to var[0]
///   Task 1 (priority 1) → function 1 → stores 20 to var[2]
#[test]
fn scenario_when_two_freewheeling_tasks_then_both_execute() {
    #[rustfmt::skip]
    let fn0_bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (10)
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    #[rustfmt::skip]
    let fn1_bytecode: Vec<u8> = vec![
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (20)
        0x18, 0x02, 0x00,  // STORE_VAR_I32 var[2]
        0xB5,              // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(4)
        .add_i32_constant(10)
        .add_i32_constant(20)
        .add_function(0, &fn0_bytecode, 1, 2)
        .add_function(1, &fn1_bytecode, 1, 2)
        .add_task(freewheeling_task(0, 0, 0))
        .add_task(freewheeling_task(1, 1, 0))
        .add_program_instance(program_instance(0, 0, 0, 0, 2))
        .add_program_instance(program_instance(1, 1, 1, 2, 2))
        .build();

    let mut vm = Vm::new().load(container).start();
    vm.run_round().unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 10); // set by task 0
    assert_eq!(vm.read_variable(2).unwrap(), 20); // set by task 1
}

/// Two tasks communicate through a shared global variable.
///
/// Layout:
///   4 variables, shared globals: 1 (var[0] is global)
///   Task 0 (priority 0) → writes 99 to var[0] (global)
///   Task 1 (priority 1) → reads var[0] (global), stores to var[2] (private)
///
/// Task 0 runs first (lower priority number), so task 1 sees the value.
#[test]
fn scenario_when_tasks_share_global_then_communication_works() {
    // Function 0: store 99 to var[0]
    #[rustfmt::skip]
    let fn0_bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (99)
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    // Function 1: copy var[0] to var[2]
    #[rustfmt::skip]
    let fn1_bytecode: Vec<u8> = vec![
        0x10, 0x00, 0x00,  // LOAD_VAR_I32 var[0]   (global)
        0x18, 0x02, 0x00,  // STORE_VAR_I32 var[2]  (private)
        0xB5,              // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(4)
        .shared_globals_size(1)
        .add_i32_constant(99)
        .add_function(0, &fn0_bytecode, 1, 1)
        .add_function(1, &fn1_bytecode, 1, 2)
        .add_task(freewheeling_task(0, 0, 0))
        .add_task(freewheeling_task(1, 1, 0))
        .add_program_instance(program_instance(0, 0, 0, 1, 1)) // task 0: private [1,2)
        .add_program_instance(program_instance(1, 1, 1, 2, 2)) // task 1: private [2,4)
        .build();

    let mut vm = Vm::new().load(container).start();
    vm.run_round().unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 99); // global, written by task 0
    assert_eq!(vm.read_variable(2).unwrap(), 99); // task 1 read the global
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
        0x10, 0x00, 0x00,  // LOAD_VAR_I32 var[0]  (outside scope)
    ];

    let container = ContainerBuilder::new()
        .num_variables(4)
        .add_function(0, &bytecode, 1, 2)
        .add_task(freewheeling_task(0, 0, 0))
        .add_program_instance(program_instance(0, 0, 0, 2, 2)) // scope [2, 4)
        .build();

    let mut vm = Vm::new().load(container).start();
    let result = vm.run_round();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().trap, Trap::InvalidVariableIndex(0));
}

/// A task whose execution exceeds its watchdog timeout is trapped.
///
/// Uses watchdog_us: 1 (1 microsecond) which is impossibly short for any
/// real execution. The program generates hundreds of instructions so that
/// the elapsed time (measured via `as_micros()`) is guaranteed > 1μs even
/// on fast machines.
#[test]
fn scenario_when_watchdog_exceeded_then_trap() {
    // Generate a long program: LOAD_CONST, then 500× (LOAD_CONST + ADD),
    // then STORE_VAR + RET_VOID. Total: ~1503 bytes, ~501 arithmetic ops.
    // This ensures execution takes well over 1μs.
    let mut bytecode: Vec<u8> = Vec::new();
    // Initial value on stack
    bytecode.extend_from_slice(&[0x01, 0x00, 0x00]); // LOAD_CONST_I32 pool[0]
    for _ in 0..500 {
        bytecode.extend_from_slice(&[0x01, 0x00, 0x00]); // LOAD_CONST_I32 pool[0]
        bytecode.push(0x30); // ADD_I32
    }
    bytecode.extend_from_slice(&[0x18, 0x00, 0x00]); // STORE_VAR_I32 var[0]
    bytecode.push(0xB5); // RET_VOID

    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(0, &bytecode, 2, 1)
        .add_task(freewheeling_task(0, 0, 1)) // watchdog_us: 1
        .add_program_instance(program_instance(0, 0, 0, 0, 1))
        .build();

    let mut vm = Vm::new().load(container).start();
    let result = vm.run_round();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().trap, Trap::WatchdogTimeout(0));
}
