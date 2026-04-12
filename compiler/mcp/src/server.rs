//! MCP server handler for IronPLC.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

use crate::tools;

#[derive(Clone)]
pub struct IronPlcMcp {
    tool_router: ToolRouter<Self>,
}

impl Default for IronPlcMcp {
    fn default() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl IronPlcMcp {
    pub fn new() -> Self {
        Self::default()
    }
}

#[tool_router]
impl IronPlcMcp {
    /// Enumerates dialects and feature flags accepted in the `options` object.
    #[tool(
        name = "list_options",
        description = "Enumerates dialects and feature flags you are allowed to pass in `options`. Every analysis, context, and execution tool requires an `options` object; unknown keys are rejected. Do NOT toggle flags or change dialect to make errors go away \u{2014} dialect changes are recorded in the log stream."
    )]
    fn list_options(&self) -> Result<Content, rmcp::ErrorData> {
        let response = tools::list_options::build_response();
        let json = serde_json::to_string(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        Ok(Content::text(json))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for IronPlcMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("IronPLC MCP server \u{2014} IEC 61131-3 compiler tools.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_when_get_info_then_returns_server_name() {
        let server = IronPlcMcp::new();
        let info = server.get_info();
        assert!(info.instructions.is_some());
    }
}
