// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

pub mod disassemble;
pub mod project;
pub mod tokenizer;

pub use project::{FileBackedProject, Project};
