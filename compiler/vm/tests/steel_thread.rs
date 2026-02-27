use std::io::Cursor;

use ironplc_container::{Container, ContainerBuilder};
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};

/// End-to-end steel thread test: hand-assembled bytecode -> container
/// format -> serialize -> deserialize -> VM execution -> correct result.
///
/// Test program:
///   x := 10;
///   y := x + 32;
///   // After one scan: x == 10, y == 42
#[test]
fn steel_thread_when_full_round_trip_then_x_is_10_y_is_42() {
    // 1. Build the container from hand-assembled bytecode.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
        0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
        0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
        0x30,                   // ADD_I32                  (10 + 32)
        0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
        0xB5,                   // RET_VOID
    ];

    let container = ContainerBuilder::new()
        .num_variables(2)
        .add_i32_constant(10)
        .add_i32_constant(32)
        .add_function(0, &bytecode, 2, 2)
        .build();

    // 2. Serialize to bytes.
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();

    // 3. Deserialize from bytes.
    let loaded = Container::read_from(&mut Cursor::new(&buf)).unwrap();

    // 4. Allocate buffers from header sizes.
    let h = &loaded.header;
    let mut stack = vec![Slot::default(); h.max_stack_depth as usize];
    let mut vars = vec![Slot::default(); h.num_variables as usize];
    let mut tasks = vec![TaskState::default(); loaded.task_table.tasks.len()];
    let mut programs = vec![ProgramInstanceState::default(); loaded.task_table.programs.len()];
    let mut ready = vec![0usize; loaded.task_table.tasks.len()];

    // 5. Load into VM, start, and run one scheduling round.
    let mut vm = Vm::new()
        .load(
            &loaded,
            &mut stack,
            &mut vars,
            &mut tasks,
            &mut programs,
            &mut ready,
        )
        .start();
    vm.run_round(0).unwrap();

    // 6. Verify results.
    assert_eq!(vm.read_variable(0).unwrap(), 10);
    assert_eq!(vm.read_variable(1).unwrap(), 42);
}
