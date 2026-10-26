//! The layout: which variable-table slot and which run of the data region
//! each variable's storage takes, decided before any instruction that reads
//! it is emitted (`specs/design/lowered-program.md`, "Existing Structures").
//!
//! The layout is built from the lowered declarations (`declarations.rs`), in
//! the order the variable table numbers them, and gives each body where
//! its variables are (`bodies.rs`). `limits.rs` checks each declaration
//! within the limits of the bytecode format first, and `var_table.rs`
//! checks each slot handed out against the variable table's limit.

mod bodies;
mod calls;
mod debug_vars;
pub(crate) mod declarations;
mod descriptors;
mod entries;
mod limits;
pub(crate) mod names;
mod region;
pub(crate) mod var_table;

pub(crate) use bodies::{BlockLayout, FunctionLayout, MethodLayout};
pub(crate) use region::{elements, slots, string_shape};
