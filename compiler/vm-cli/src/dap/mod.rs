//! Debug Adapter Protocol server for IronPLC.
//!
//! Feature-gated behind `dap` and built into the dedicated `ironplcdap`
//! binary. The production `ironplcvm` binary does not include this module.
//!
//! Phase 4 lands incrementally (see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`). So far: the wire
//! [`framing`] layer, the hand-rolled message [`types`], the request [`state`]
//! legality table, the [`launch`] preconditions, the isolated [`debug_info`]
//! resolver, and the [`server`] event loop implementing the
//! `initialize`/`launch`/`disconnect` handshake. The run/stop loop that drives
//! execution arrives in a later commit.

pub mod debug_info;
pub mod framing;
pub mod launch;
pub mod server;
pub mod state;
pub mod types;
