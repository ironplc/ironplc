#![allow(clippy::result_large_err)]
// A match over a lowered enum names every variant, so a variant added later
// fails to compile wherever it is not handled (REQ-LOW-lowering-082).
#![deny(clippy::wildcard_enum_match_arm)]
//! Code generation for IronPLC.
//!
//! This crate compiles a lowered program (`ironplc_ir::lowered::Program`)
//! into bytecode that the IronPLC VM can execute, packaged as a `Container`.
//! Lowering builds the program from an analyzed library
//! (`ironplc_lowering::lower`); see `specs/design/lowered-program.md`.
//!
//! # Example
//!
//! ```ignore
//! use ironplc_analyzer::CleanAnalysis;
//! use ironplc_codegen::{compile, EmptyLookup};
//! use ironplc_parser::parse_program;
//!
//! let source = "PROGRAM main VAR x : INT; END_VAR x := 42; END_PROGRAM";
//! let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
//! let (analyzed, ctx) = ironplc_analyzer::stages::analyze(&[&library], &CompilerOptions::default()).unwrap();
//! let analysis = CleanAnalysis::new(&analyzed, &ctx).unwrap();
//! let program = ironplc_lowering::lower(&analysis).unwrap();
//! let container = compile(&program, &EmptyLookup).unwrap();
//! ```

mod access;
mod body;
mod call_graph;
mod compile;
mod compile_arith;
mod compile_array;
mod compile_array_struct;
mod compile_call;
mod compile_enum;
mod compile_expr;
mod compile_fb_call;
mod compile_fn;
mod compile_method;
mod compile_partial_access;
mod compile_place;
mod compile_reference;
mod compile_setup;
mod compile_stmt;
mod compile_string;
mod compile_string_place;
mod compile_struct;
mod compile_struct_init;
mod compile_subscript;
mod compile_user_call;
mod emit;
mod execution;
mod held_instances;
mod layout;
mod optimize;
mod source_lookup;
mod stack_balance;
#[cfg(test)]
mod test_support;

pub use compile::compile;
pub use ironplc_ir::source::{EmptyLookup, SourceLookup};

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
#[cfg(test)]
mod spec_conformance_adr;
#[cfg(test)]
mod spec_conformance_arithmetic_operator_overloads;
#[cfg(test)]
mod spec_conformance_behavior_policies;
#[cfg(test)]
mod spec_conformance_container_format;
#[cfg(test)]
mod spec_conformance_inline_enumeration;
#[cfg(test)]
mod spec_conformance_keyword_function_forms;
