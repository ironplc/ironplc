//! VM error types with V-codes for user-facing error reporting.

use std::fmt;

use ironplc_vm::error::Trap;

/// V6xxx code constants for file system / IO errors.
pub const FILE_OPEN: &str = "V6001";
pub const CONTAINER_READ: &str = "V6002";
pub const SIGNAL_HANDLER: &str = "V6003";
pub const DUMP_CREATE: &str = "V6004";
pub const VAR_READ: &str = "V6005";
pub const DUMP_WRITE: &str = "V6006";
pub const LOG_CONFIG: &str = "V6007";

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
    pub fn from_trap(trap: &Trap, task_id: u16, instance_id: u16) -> Self {
        VmError {
            v_code: trap.v_code(),
            exit_code: trap.exit_code(),
            message: format!("VM trap: {trap} (task {task_id}, instance {instance_id})"),
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
        let err = VmError::from_trap(&Trap::DivideByZero, 0, 0);
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
        let err = VmError::from_trap(&Trap::DivideByZero, 2, 5);
        assert_eq!(
            err.to_string(),
            "V4001 - VM trap: divide by zero (task 2, instance 5)"
        );
    }
}
