pub mod error;
pub(crate) mod scheduler;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use value::Slot;
pub use vm::{FaultContext, StopHandle, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
