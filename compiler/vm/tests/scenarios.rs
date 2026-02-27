//! Phase 2: Multi-scan scenario tests.
//!
//! These verify that programs accumulate state correctly across scan cycles
//! and that the VM lifecycle transitions work with repeated execution.

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
