use std::fmt;

use ironplc_container::ContainerError;

/// Runtime traps that halt VM execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trap {
    DivideByZero,
    StackOverflow,
    StackUnderflow,
    InvalidInstruction(u8),
    InvalidConstantIndex(u16),
    InvalidVariableIndex(u16),
    InvalidFunctionId(u16),
}

impl fmt::Display for Trap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trap::DivideByZero => write!(f, "divide by zero"),
            Trap::StackOverflow => write!(f, "stack overflow"),
            Trap::StackUnderflow => write!(f, "stack underflow"),
            Trap::InvalidInstruction(op) => write!(f, "invalid instruction: 0x{op:02X}"),
            Trap::InvalidConstantIndex(i) => write!(f, "invalid constant index: {i}"),
            Trap::InvalidVariableIndex(i) => write!(f, "invalid variable index: {i}"),
            Trap::InvalidFunctionId(id) => write!(f, "invalid function ID: {id}"),
        }
    }
}

/// Errors produced by VM operations.
#[derive(Debug)]
pub enum VmError {
    /// A runtime trap occurred during execution.
    Trap(Trap),
    /// An error occurred while loading or reading the container.
    Container(ContainerError),
    /// The VM is not in the correct state for the requested operation.
    InvalidState(&'static str),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::Trap(t) => write!(f, "trap: {t}"),
            VmError::Container(e) => write!(f, "container error: {e}"),
            VmError::InvalidState(msg) => write!(f, "invalid VM state: {msg}"),
        }
    }
}

impl std::error::Error for VmError {}

impl From<Trap> for VmError {
    fn from(t: Trap) -> Self {
        VmError::Trap(t)
    }
}

impl From<ContainerError> for VmError {
    fn from(e: ContainerError) -> Self {
        VmError::Container(e)
    }
}
