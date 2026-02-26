pub mod cli;
pub mod error;
pub mod logger;
pub(crate) mod scheduler;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use value::Slot;
pub use vm::{Vm, VmReady, VmRunning};
