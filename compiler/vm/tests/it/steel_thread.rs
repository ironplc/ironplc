use crate::common::{load_and_start, round_trip, steel_thread_container, VmBuffers};
use ironplc_container::VarIndex;

/// End-to-end steel thread test: hand-assembled bytecode -> container
/// format -> serialize -> deserialize -> VM execution -> correct result.
///
/// Test program:
///   x := 10;
///   y := x + 32;
///   // After one scan: x == 10, y == 42
#[test]
fn steel_thread_when_full_round_trip_then_x_is_10_y_is_42() {
    // 1. Build the container from hand-assembled bytecode, then serialize
    //    and deserialize it.
    let loaded = round_trip(&steel_thread_container());

    // 2. Allocate buffers from header sizes and run.
    let mut b = VmBuffers::from_container(&loaded);
    let mut vm = load_and_start(&loaded, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // 3. Verify results.
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 42);
}
