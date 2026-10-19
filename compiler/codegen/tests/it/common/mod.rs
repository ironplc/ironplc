//! Shared test helpers for codegen integration tests.
//!
//! - [`bc`]: per-instruction bytecode builders and `assert_bytecode!`
//! - `backend`: the backends a program runs on, and their agreement
//! - `vm`: the bytecode backend on the VM
//! - `run`: parse, compile and run a program, and drive function blocks
//! - `assert`: single-scan assertion helpers and the `e2e!` / `e2e_*!` macros
//! - `snapshot`, `session`: reading (and, across scans, writing) a program's
//!   variables by name
//! - `variables`: resolving a name to a variable, and to its VM slot
//! - `value`: the [`Value`] a test observes, and typed expectations
//! - `slot_value`: decoding a VM slot into a [`Value`], and encoding one
//! - [`scalar`]: the elementary types as numbers, for tests with a Rust oracle
//!
//! Everything tests use is re-exported here, so they name it as
//! `crate::common::<item>`.

#![allow(dead_code)]
#![allow(unused_macros)]
#![allow(clippy::result_large_err)]

#[macro_use]
pub mod bc;
#[macro_use]
mod assert;
mod backend;
mod run;
pub mod scalar;
mod session;
mod slot_value;
mod snapshot;
mod value;
mod variables;
mod vm;

pub use assert::*;
pub use backend::{assert_backends_agree, Reference};
pub use ironplc_vm::VmBuffers;
pub use run::*;
pub use snapshot::Snapshot;
pub use value::{FromValue, Value};
pub use variables::vm_var_index;
// Date and time types and macros for writing temporal expectations.
pub use time::macros::{date, datetime, time};
pub use time::{Date, Duration, PrimitiveDateTime, Time};
