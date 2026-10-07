//! Integration tests for the `FLAG_HAS_SYSTEM_UPTIME` validation performed
//! by `Vm::load`.
//!
//! A container with the flag set promises that `__SYSTEM_UP_TIME` and
//! `__SYSTEM_UP_LTIME` occupy variable slots 0 and 1, and every scan writes
//! them. The flag comes from disk, so `Vm::load` checks the promise before
//! any code runs rather than letting the first scan fail.

use ironplc_container::{opcode, VarIndex, FLAG_HAS_SYSTEM_UPTIME};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

use crate::common::{single_function_container, VmBuffers};

fn uptime_container(num_variables: u16) -> ironplc_container::Container {
    let mut c = single_function_container(&[opcode::RET_VOID], num_variables, &[]);
    c.header.flags |= FLAG_HAS_SYSTEM_UPTIME;
    c
}

#[test]
fn load_when_uptime_flag_set_and_no_variables_then_rejected() {
    let c = uptime_container(0);
    let mut b = VmBuffers::from_container(&c);

    let result = Vm::new().load(&c, &mut b);

    assert_eq!(result.err(), Some(Trap::SystemUptimeVariablesMissing));
}

#[test]
fn load_when_uptime_flag_set_and_one_variable_then_rejected() {
    let c = uptime_container(1);
    let mut b = VmBuffers::from_container(&c);

    let result = Vm::new().load(&c, &mut b);

    assert_eq!(result.err(), Some(Trap::SystemUptimeVariablesMissing));
}

#[test]
fn load_when_uptime_flag_set_and_variable_buffer_too_small_then_rejected() {
    // The container declares enough variables, but the embedder's buffer
    // backing them is smaller -- the buffer is what the scan writes to.
    let c = uptime_container(2);
    let mut b = VmBuffers::from_container(&c);
    b.vars.truncate(1);

    let result = Vm::new().load(&c, &mut b);

    assert_eq!(result.err(), Some(Trap::SystemUptimeVariablesMissing));
}

#[test]
fn load_when_uptime_flag_clear_and_no_variables_then_runs() {
    let c = single_function_container(&[opcode::RET_VOID], 0, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new().load(&c, &mut b).unwrap().start().unwrap();

    assert!(vm.run_round(1_000).is_ok());
}

#[test]
fn run_round_when_uptime_flag_set_and_two_variables_then_writes_uptime() {
    let c = uptime_container(2);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new().load(&c, &mut b).unwrap().start().unwrap();

    vm.run_round(2_500_000).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 2_500);
    assert_eq!(vm.read_variable_i64(VarIndex::new(1)).unwrap(), 2_500);
}
