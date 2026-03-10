mod buffers;
pub(crate) mod builtin;
pub mod error;
pub(crate) mod intrinsic;
pub(crate) mod scheduler;
pub(crate) mod stack;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use buffers::VmBuffers;
pub use scheduler::{ProgramInstanceState, TaskState};
pub use value::Slot;
pub use vm::{FaultContext, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
