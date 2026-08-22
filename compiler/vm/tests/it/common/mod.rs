//! Shared test helpers for VM integration tests.
//!
//! Everything here now lives in `ironplc_vm::test_support` (which itself
//! re-exports the container fixtures from `ironplc_container::test_support`),
//! so the same helpers are reachable from `vm-cli`, `project` and `codegen`.
//! This module stays as the import surface the test files already use.

#![allow(unused_imports)]

pub use ironplc_vm::test_support::*;
pub use ironplc_vm::VmBuffers;
