#![no_std]
#![allow(clippy::result_large_err)]

#[cfg(feature = "std")]
extern crate std;

// Always available (no_std)
pub mod builtin;
mod char_width;
pub mod cmp_op;
mod const_type;
mod container_ref;
mod error;
pub mod fb_type;
mod header;
pub mod id_types;
mod instruction;
pub mod opcode;
pub mod policy;
mod string_layout;
mod task_type;

// Only available with std
#[cfg(feature = "std")]
mod builder;
mod code_section;
#[cfg(feature = "std")]
mod constant_pool;
#[cfg(feature = "std")]
mod container;
#[cfg(feature = "std")]
pub mod debug_format;
#[cfg(feature = "std")]
pub mod debug_section;
pub mod task_table;
// Shared container fixtures. Compiled for this crate's own tests, and for
// downstream test suites via the `test-support` feature.
#[cfg(all(feature = "std", any(test, feature = "test-support")))]
pub mod test_support;
mod type_section;
#[cfg(feature = "std")]
pub mod verify;

// Always-available re-exports
pub use char_width::CharWidth;
pub use code_section::FuncEntry;
pub use const_type::ConstType;
pub use container_ref::ContainerRef;
pub use error::ContainerError;
pub use header::{
    FileHeader, FLAG_HAS_DEBUG_SECTION, FLAG_HAS_SYSTEM_UPTIME, FLAG_HAS_TYPE_SECTION,
    FORMAT_VERSION, HEADER_SIZE, MAGIC,
};
pub use id_types::{
    ConstantIndex, FbTypeId, FunctionId, InstanceId, SlotIndex, SourceColumn, SourceFileId,
    SourceLine, TaskId, VarIndex,
};
pub use opcode::Opcode;
pub use string_layout::{string_region_size, DEFAULT_STRING_MAX_LENGTH, STRING_HEADER_BYTES};
pub use task_table::{ProgramInstanceEntry, TaskEntry};
pub use task_type::TaskType;
pub use type_section::{ArrayDescriptor, FieldEntry, FieldType, UserFbDescriptor};

// std-only re-exports
#[cfg(feature = "std")]
pub use builder::ContainerBuilder;
#[cfg(feature = "std")]
pub use code_section::CodeSection;
#[cfg(feature = "std")]
pub use constant_pool::{ConstEntry, ConstantPool};
#[cfg(feature = "std")]
pub use container::Container;
#[cfg(feature = "std")]
pub use debug_format::{
    RenderedValue, VarDebugInfo, VariableRenderer, VALUE_INVALID, VALUE_UNAVAILABLE,
};
#[cfg(feature = "std")]
pub use debug_section::{
    DebugSection, EnumDefEntry, FuncNameEntry, LineMapEntry, SourceFileEntry, StringLayoutEntry,
    VarNameEntry, SOURCE_FILE_HASH_LEN,
};
#[cfg(feature = "std")]
pub use task_table::TaskTable;
#[cfg(feature = "std")]
pub use type_section::{FbTypeDescriptor, TypeSection};
#[cfg(feature = "std")]
pub use verify::{verify_stack_balance, StackImbalance};

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
#[cfg(test)]
mod spec_conformance_behavior_policies;
