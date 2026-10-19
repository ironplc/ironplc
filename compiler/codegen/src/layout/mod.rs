//! The layout: which variable-table slot and which run of the data region
//! each variable's storage takes, decided before any instruction that reads
//! it is emitted (`specs/design/lowered-program.md`, "Existing Structures").
//!
//! The walk visits the declarations in the order the variable table numbers
//! them:
//!
//! 1. the compiler-provided globals, then the declared globals
//!    (`globals.rs`);
//! 2. once the function block types are known, the program's variables
//!    (`program.rs`);
//! 3. each reachable function, in library order (`function.rs`).
//!
//! The function blocks and methods are laid out as each is compiled. `storage.rs` lays out one variable's storage and records its
//! debug entry, whose type tag and name `debug_type.rs` gives.

mod debug_type;
mod function;
mod globals;
mod program;
mod storage;

pub(crate) use debug_type::{debug_type_for_decl, map_var_section};
pub(crate) use function::{lay_out_function, FunctionLayout};
pub(crate) use globals::lay_out_globals;
pub(crate) use program::lay_out_program;
