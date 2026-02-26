#![no_std]
#![allow(clippy::result_large_err)]

#[cfg(feature = "std")]
extern crate std;

// Always available (no_std)
mod const_type;
mod error;
mod header;
pub mod opcode;
mod task_type;

// Only available with std
#[cfg(feature = "std")]
mod builder;
#[cfg(feature = "std")]
mod code_section;
#[cfg(feature = "std")]
mod constant_pool;
#[cfg(feature = "std")]
mod container;
#[cfg(feature = "std")]
mod task_table;

// Always-available re-exports
pub use const_type::ConstType;
pub use error::ContainerError;
pub use header::{FileHeader, FORMAT_VERSION, HEADER_SIZE, MAGIC};
pub use task_type::TaskType;

// std-only re-exports
#[cfg(feature = "std")]
pub use builder::ContainerBuilder;
#[cfg(feature = "std")]
pub use code_section::{CodeSection, FuncEntry};
#[cfg(feature = "std")]
pub use constant_pool::{ConstEntry, ConstantPool};
#[cfg(feature = "std")]
pub use container::Container;
#[cfg(feature = "std")]
pub use task_table::{ProgramInstanceEntry, TaskEntry, TaskTable};
