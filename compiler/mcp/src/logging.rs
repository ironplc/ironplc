//! Logger configuration for the MCP server.
//!
//! Logs go to stderr because stdout is the JSON-RPC channel (REQ-ARC-043).

use env_logger::Builder;
use log::LevelFilter;

/// Initializes the logger to write to stderr at the `info` level.
///
/// The log level can be overridden via the `RUST_LOG` environment variable.
pub fn init() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();
}
