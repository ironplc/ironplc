//! The layout: which variable-table slot and which run of the data region
//! each variable's storage takes, decided before any instruction that reads
//! it is emitted (`specs/design/lowered-program.md`, "Existing Structures").
//!
//! The walk visits the declarations in the order the variable table numbers
//! them:
//!
//! 1. the compiler-provided globals, then the declared globals
//!    (`globals.rs`);
//! 2. once the function block types are registered, the program's variables
//!    (`program.rs`);
//! 3. each reachable function, in library order (`function.rs`);
//! 4. each reachable function block, in library order, then its methods
//!    (`block.rs`).
//!
//! The function block types are registered first, before the program's
//! variables, since an instance of one needs its fields' indices. `storage.rs` lays out one variable's storage and records its
//! debug entry, whose type tag and name `debug_type.rs` gives.

mod block;
mod debug_type;
mod descriptors;
mod entries;
mod function;
mod globals;
pub(crate) mod ids;
pub(crate) mod lowered;
pub(crate) mod names;
mod program;
mod region;
mod storage;

pub(crate) use block::{lay_out_block, lay_out_block_types, BlockLayout, MethodLayout};
pub(crate) use debug_type::{debug_type_for_decl, map_var_section};
pub(crate) use function::{lay_out_function, FunctionLayout};
pub(crate) use globals::lay_out_globals;
pub(crate) use program::lay_out_program;
