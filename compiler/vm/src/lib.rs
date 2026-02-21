// These modules are not yet consumed by the VM interpreter (coming next);
// suppress dead_code warnings until then.
#![allow(dead_code)]

pub mod error;
pub mod opcode;
pub(crate) mod stack;
pub(crate) mod value;
pub(crate) mod variable_table;

pub use value::Slot;
