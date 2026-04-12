//! MCP server handler for IronPLC.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

use crate::tools;
use crate::tools::common::ParseCheckInput;

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
        description = "Enumerates dialects and feature flags accepted in the options object of analysis and execution tools."
    )]
    fn list_options(&self) -> Result<Content, rmcp::ErrorData> {
        let response = tools::list_options::build_response();
        let json = serde_json::to_string(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        Ok(Content::text(json))
    }

    /// Syntax check only.
    #[tool(
        name = "parse",
        description = "Syntax check only. Use while drafting to confirm the source tokenizes and parses. Do NOT use this to validate a change -- it does not catch type errors, undeclared symbols, or any other semantic rule. Call `check` for that."
    )]
    fn parse(&self, params: Parameters<ParseCheckInput>) -> Result<Content, rmcp::ErrorData> {
        let input = params.0;
        let response = tools::parse::build_response(&input.sources, &input.options);
        let json = serde_json::to_string(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        Ok(Content::text(json))
    }

    /// Primary validator.
    #[tool(
        name = "check",
        description = "Primary validator. Runs parse and full semantic analysis and returns structured diagnostics. ALWAYS run this before reporting success to the user and before calling `compile` or `run`. Self-heal by reading the returned diagnostics, fixing the code, and calling `check` again. Call `explain_diagnostic` to understand any unfamiliar problem code BEFORE editing the source."
    )]
    fn check(&self, params: Parameters<ParseCheckInput>) -> Result<Content, rmcp::ErrorData> {
        let input = params.0;
        let response = tools::check::build_response(&input.sources, &input.options);
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
