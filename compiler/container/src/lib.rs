#![allow(clippy::result_large_err)]

mod builder;
mod code_section;
mod constant_pool;
mod container;
mod error;
mod header;
pub mod opcode;
mod task_table;

pub use builder::ContainerBuilder;
pub use code_section::{CodeSection, FuncEntry};
pub use constant_pool::{ConstEntry, ConstType, ConstantPool};
pub use container::Container;
pub use error::ContainerError;
pub use header::{FileHeader, FORMAT_VERSION, HEADER_SIZE, MAGIC};
pub use task_table::{ProgramInstanceEntry, TaskEntry, TaskTable, TaskType};
