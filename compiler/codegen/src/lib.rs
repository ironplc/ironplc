#![allow(clippy::result_large_err)]
//! Code generation for IronPLC.
//!
//! This crate transforms a parsed and analyzed IEC 61131-3 AST (`Library`)
//! into bytecode that the IronPLC VM can execute, packaged as a `Container`.
//!
//! # Supported Subset
//!
//! The initial implementation supports a minimal subset of the language
//! sufficient for a steel-thread demonstration:
//!
//! - PROGRAM declarations
//! - INT variable declarations (`VAR`)
//! - Assignment statements
//! - Integer literal constants
//! - Binary Add operator
//! - Variable references (named symbolic variables)
//!
//! # Example
//!
//! ```ignore
//! use ironplc_codegen::compile;
//! use ironplc_parser::parse_program;
//!
//! let source = "PROGRAM main VAR x : INT; END_VAR x := 42; END_PROGRAM";
//! let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
//! let container = compile(&library).unwrap();
//! ```

mod compile;
mod emit;

pub use compile::compile;
