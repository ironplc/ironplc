//! MCP server handler for IronPLC.

use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

use crate::cache::ContainerCache;
use crate::tools;
use crate::tools::common::ParseCheckInput;
use crate::tools::compile::CompileInput;
use crate::tools::container_drop::ContainerDropInput;
use crate::tools::explain_diagnostic::ExplainDiagnosticInput;

#[derive(Clone)]
pub struct IronPlcMcp {
    tool_router: ToolRouter<Self>,
    cache: Arc<Mutex<ContainerCache>>,
}

impl Default for IronPlcMcp {
    fn default() -> Self {
        Self {
            tool_router: Self::tool_router(),
            cache: Arc::new(Mutex::new(ContainerCache::new(
                crate::cache::DEFAULT_MAX_ENTRIES,
                crate::cache::DEFAULT_MAX_BYTES,
            ))),
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

    /// Look up the explanation for a problem code.
    #[tool(
        name = "explain_diagnostic",
        description = "Look up the human-readable explanation for a problem code (e.g. `P0042`). Call this before editing code in response to a diagnostic you do not fully understand."
    )]
    fn explain_diagnostic(
        &self,
        Parameters(input): Parameters<ExplainDiagnosticInput>,
    ) -> Result<Content, rmcp::ErrorData> {
        let response = tools::explain_diagnostic::build_response(&input.code);
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

    /// Full pipeline: parse, semantic analysis, and codegen.
    #[tool(
        name = "compile",
        description = "Only call this when you need a compiled artifact to `run`. For validation, call `check` instead \u{2014} `check` is faster, produces the same diagnostics, and does not incur codegen cost. A failing `compile` does not give you any information that a failing `check` would not."
    )]
    async fn compile(
        &self,
        Parameters(input): Parameters<CompileInput>,
    ) -> Result<Content, rmcp::ErrorData> {
        let response = tools::compile::build_response(
            &input.sources,
            &input.options,
            input.include_bytes,
            &self.cache,
        );
        let json = serde_json::to_string(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        Ok(Content::text(json))
    }

    /// Explicitly releases a compiled container from the cache.
    #[tool(
        name = "container_drop",
        description = "Explicitly releases a compiled container from the cache. Not usually necessary \u{2014} the cache evicts on LRU pressure \u{2014} but available for long-running connections."
    )]
    async fn container_drop(
        &self,
        Parameters(input): Parameters<ContainerDropInput>,
    ) -> Result<Content, rmcp::ErrorData> {
        let response = tools::container_drop::build_response(&input.container_id, &self.cache);
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
