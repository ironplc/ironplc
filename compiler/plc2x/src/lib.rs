// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

pub mod cli;
pub mod logger;
pub mod lsp;
pub mod lsp_project;
pub mod project;
pub mod tokenizer;

#[cfg(test)]
mod test_helpers;
