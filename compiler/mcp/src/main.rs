//! Placeholder binary for a future Model Context Protocol server.

use ironplc_project::FileBackedProject;

fn main() {
    // Keep `ironplc-project` linked; the real MCP server will build on this crate.
    let _ = FileBackedProject::new();
    eprintln!("ironplcmcp: MCP server not yet implemented");
}
