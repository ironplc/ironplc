//! Debug Adapter Protocol server for IronPLC.
//!
//! Feature-gated behind `dap` and built into the dedicated `ironplcdap`
//! binary. The production `ironplcvm` binary does not include this module.
//!
//! Phase 4 lands incrementally (see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`). This first slice is the
//! wire [`framing`] layer only.

pub mod framing;
