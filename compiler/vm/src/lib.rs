pub mod error;
pub mod opcode;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;
mod vm;

pub use value::Slot;
pub use vm::{Vm, VmReady, VmRunning};
