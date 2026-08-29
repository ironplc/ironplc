//! Debug Adapter Protocol server for IronPLC.
//!
//! Built into the dedicated `ironplcvmd` binary. The production `ironplcvm`
//! binary does not include this module.
//!
//! Phase 4 lands incrementally (see `specs/design/debugger-support.md`
//! §"Layer 3: DAP Server"). So far: the wire
//! [`framing`] layer, the hand-rolled message [`types`], the request [`state`]
//! legality table, the [`launch`] preconditions, the isolated [`debug_info`]
//! resolver, and the [`server`] event loop implementing the
//! `initialize`/`launch`/`disconnect` handshake. The run/stop loop that drives
//! execution arrives in a later commit.

pub mod debug_info;
pub mod framing;
pub mod launch;
pub mod problem_codes;
pub mod server;
pub mod state;
pub mod types;
