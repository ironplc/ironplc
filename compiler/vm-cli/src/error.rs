//! VM error types with V-codes for user-facing error reporting.

use std::fmt;

use ironplc_container::{InstanceId, TaskId};
use ironplc_vm::error::Trap;

// V6xxx code constants are generated from resources/problem-codes.csv
include!(concat!(env!("OUT_DIR"), "/io_codes.rs"));

/// Exit code for file system / IO errors.
const IO_EXIT_CODE: u8 = 2;

/// A user-facing VM error with a V-code and category exit code.
pub struct VmError {
    v_code: &'static str,
    exit_code: u8,
    message: String,
}

impl VmError {
    /// Creates a `VmError` from a runtime trap with execution context.
    pub fn from_trap(trap: &Trap, task_id: TaskId, instance_id: InstanceId) -> Self {
        VmError {
            v_code: trap.v_code(),
            exit_code: trap.exit_code(),
            message: format!(
                "runtime error: {trap} (task {}, instance {})",
                task_id.raw(),
                instance_id.raw()
            ),
        }
    }

    /// Creates a `VmError` for a file system or IO error.
    pub fn io(v_code: &'static str, message: String) -> Self {
        VmError {
            v_code,
            exit_code: IO_EXIT_CODE,
            message,
        }
    }

    /// Returns the process exit code for this error's category.
    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.v_code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_trap_when_divide_by_zero_then_v4001_exit_1() {
        let err = VmError::from_trap(&Trap::DivideByZero, TaskId::new(0), InstanceId::new(0));
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().starts_with("V4001"));
    }

    #[test]
    fn io_when_file_open_then_v6001_exit_2() {
        let err = VmError::io(FILE_OPEN, "test".to_string());
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().starts_with("V6001"));
    }

    #[test]
    fn display_when_trap_then_includes_context() {
        let err = VmError::from_trap(&Trap::DivideByZero, TaskId::new(2), InstanceId::new(5));
        assert_eq!(
            err.to_string(),
            "V4001 - runtime error: divide by zero (task 2, instance 5)"
        );
    }
}
