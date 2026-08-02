//! `ironplcdap` — the IronPLC Debug Adapter Protocol server.
//!
//! Speaks DAP over stdin/stdout so an editor (VS Code, Phase 5) can drive a
//! debug session. Feature-gated behind `dap`; see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`.
//!
//! This slice drives the `initialize` → `launch` → `disconnect` handshake
//! ([`dap::server::serve`]) over stdin/stdout. Execution control (breakpoints,
//! stepping, inspection) arrives in later commits.

mod dap;

use std::io::{self, BufReader};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    dap::server::serve(&mut reader, &mut writer)
}
