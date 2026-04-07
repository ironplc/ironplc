mod buffers;
pub(crate) mod builtin;
pub mod error;
pub(crate) mod intrinsic;
#[cfg(feature = "profiling")]
mod profile;
pub(crate) mod scheduler;
pub(crate) mod stack;
pub(crate) mod string_ops;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use buffers::VmBuffers;
#[cfg(feature = "profiling")]
pub use profile::InstructionProfile;
pub use scheduler::{ProgramInstanceState, TaskState};
pub use value::Slot;
pub use vm::{FaultContext, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
