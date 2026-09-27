//! Intermediate representation of the WebAssembly target: its definition
//! ([`ir`]) and the IR validator ([`validate`]). The crate has no
//! dependency: a front end builds a [`Module`] (`ironplc-wasm` does it for an
//! analysed IronPLC program) and `ironplc-wasm-codegen` turns it into a logic
//! module (`compiler/wasm-symbols/docs/backend.md`).
#![forbid(unsafe_code)]

pub mod ir;
mod validate;

pub use ir::*;
pub use validate::validate;
