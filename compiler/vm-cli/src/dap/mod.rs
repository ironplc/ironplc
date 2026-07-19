//! Debug Adapter Protocol server for IronPLC.
//!
//! Feature-gated behind `dap` and built into the dedicated `ironplcdap`
//! binary. The production `ironplcvm` binary does not include this module.
//!
//! Phase 4 lands incrementally (see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`). So far: the wire
//! [`framing`] layer, the hand-rolled message [`types`], and the request
//! [`state`] legality table. The request-dispatch loop that consumes them
//! arrives in a later commit.

pub mod framing;
pub mod state;
pub mod types;
