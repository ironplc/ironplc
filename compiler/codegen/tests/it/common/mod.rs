//! Shared test helpers for codegen integration tests.
//!
//! - [`bc`]: per-instruction bytecode builders and `assert_bytecode!`
//! - `run`: parse, compile and run a program, and drive function blocks
//! - `assert`: single-scan assertion helpers and the `e2e!` / `e2e_*!` macros
//! - `slot_value`: reading a typed value out of a variable slot
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
mod run;
mod slot_value;

pub use assert::*;
pub use ironplc_vm::VmBuffers;
pub use run::*;
pub use slot_value::SlotValue;
// Date and time types and macros for writing temporal expectations.
pub use time::macros::{date, datetime, time};
pub use time::{Date, Duration, PrimitiveDateTime, Time};
