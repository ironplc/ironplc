//! Entry point for the IronPLC MCP server.

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    ironplc_mcp::logging::init();
    ironplc_mcp::run_server().await
}
