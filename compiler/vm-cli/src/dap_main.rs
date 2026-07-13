//! `ironplcdap` — the IronPLC Debug Adapter Protocol server.
//!
//! Speaks DAP over stdin/stdout so an editor (VS Code, Phase 5) can drive a
//! debug session. Feature-gated behind `dap`; see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`.
//!
//! This first slice is a skeleton: the [`dap::framing`] layer is in place and
//! unit-tested, and `main` drains framed messages to prove the binary builds
//! and reads the wire correctly. Protocol handling (`initialize`, `launch`,
//! breakpoints, stepping) arrives in later commits.

mod dap;

use std::io::{self, BufReader};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());

    // No-op handler: consume framed messages until the client disconnects.
    // Later commits dispatch each message to the request handlers.
    while dap::framing::read_message(&mut reader)?.is_some() {}

    Ok(())
}
