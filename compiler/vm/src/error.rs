use core::fmt;

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
    WatchdogTimeout(u16),
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
            Trap::WatchdogTimeout(id) => write!(f, "watchdog timeout on task {id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_display_when_watchdog_timeout_then_includes_task_id() {
        let trap = Trap::WatchdogTimeout(3);
        assert_eq!(format!("{trap}"), "watchdog timeout on task 3");
    }
}
