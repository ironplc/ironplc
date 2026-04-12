//! IronPLC MCP server library.
//!
//! Exposes IEC 61131-3 compiler capabilities as MCP tools over stdio transport.

pub mod logging;
pub mod server;
pub mod tools;

use rmcp::ServiceExt;
use server::IronPlcMcp;

/// Runs the MCP server over stdin/stdout until the client disconnects.
pub async fn run_server() -> Result<(), String> {
    let service = IronPlcMcp::new();
    let transport = rmcp::transport::io::stdio();
    let server = service
        .serve(transport)
        .await
        .map_err(|e| format!("Failed to start MCP server: {e}"))?;
    server
        .waiting()
        .await
        .map_err(|e| format!("MCP server error: {e}"))?;
    Ok(())
}
