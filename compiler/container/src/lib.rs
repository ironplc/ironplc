#![no_std]
#![allow(clippy::result_large_err)]

#[cfg(feature = "std")]
extern crate std;

// Always available (no_std)
mod const_type;
mod container_ref;
mod error;
mod header;
pub mod id_types;
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
pub mod debug_section;
#[cfg(feature = "std")]
pub mod task_table;
#[cfg(feature = "std")]
mod type_section;

// Always-available re-exports
pub use const_type::ConstType;
pub use container_ref::{ContainerRef, ProgramEntryRef, TaskEntryRef};
pub use error::ContainerError;
pub use header::{
    FileHeader, FLAG_HAS_SYSTEM_UPTIME, FORMAT_VERSION, HEADER_SIZE, MAGIC, STRING_HEADER_BYTES,
};
pub use id_types::{ConstantIndex, FbTypeId, FunctionId, InstanceId, SlotIndex, TaskId, VarIndex};
pub use opcode::Opcode;
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
pub use debug_section::{DebugSection, EnumDefEntry, FuncNameEntry, LineMapEntry, VarNameEntry};
#[cfg(feature = "std")]
pub use task_table::{ProgramInstanceEntry, TaskEntry, TaskTable};
#[cfg(feature = "std")]
pub use type_section::{
    ArrayDescriptor, FbTypeDescriptor, FieldEntry, FieldType, TypeSection, UserFbDescriptor,
};

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
