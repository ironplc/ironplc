//! V-code constants generated from `resources/problem-codes.csv`.
//!
//! The `ironplcvm` binary reaches these constants through `error.rs`; the
//! `ironplcdap` binary does not compile that module, so this thin re-include
//! gives the DAP server the same generated constants from the one CSV source of
//! truth. Only a few are used on the launch path (`FILE_OPEN`, `CONTAINER_READ`,
//! and the `LAUNCH_*` codes); the rest are unused here, hence the allowance.
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/io_codes.rs"));
