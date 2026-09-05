//! Building blocks shared by the IronPLC command line programs.
//!
//! The command line programs (`ironplcc`, `ironplcvm`) are separate crates
//! with deliberately different dependency trees. This crate holds the small
//! amount of behavior they genuinely share, so that it is written and tested
//! once. Each program keeps ownership of how it reports failures: functions
//! here return this crate's own error types, and each program maps those onto
//! its own user-facing error type at its boundary.

pub mod logger;
