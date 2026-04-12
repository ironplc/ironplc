//! The `container_drop` MCP tool.
//!
//! Removes a previously compiled container from the process container cache.

use std::sync::Mutex;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::serialize_diagnostics;
use crate::cache::ContainerCache;

/// Input for the `container_drop` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContainerDropInput {
    /// The opaque container ID returned by `compile`.
    pub container_id: String,
}

/// Response for the `container_drop` tool.
#[derive(Debug, Serialize)]
pub struct ContainerDropResponse {
    pub ok: bool,
    pub removed: bool,
    pub diagnostics: Vec<serde_json::Value>,
}

/// Builds the container_drop response.
pub fn build_response(container_id: &str, cache: &Mutex<ContainerCache>) -> ContainerDropResponse {
    let removed = {
        let mut guard = cache.lock().unwrap();
        guard.remove(container_id)
    };

    if removed {
        ContainerDropResponse {
            ok: true,
            removed: true,
            diagnostics: vec![],
        }
    } else {
        let err = Diagnostic::problem(
            Problem::McpInputValidation,
            Label::span(
                SourceSpan::default(),
                format!("unknown container_id '{container_id}'"),
            ),
        );
        ContainerDropResponse {
            ok: false,
            removed: false,
            diagnostics: serialize_diagnostics(&[err]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CachedContainer, ContainerCache};

    fn make_cache_with_entry() -> (Mutex<ContainerCache>, String) {
        let mut cache = ContainerCache::new(64, 64 * 1024 * 1024);
        let container = CachedContainer::new(vec![0u8; 100], vec![], vec![]);
        let id = cache.insert(container).unwrap();
        (Mutex::new(cache), id)
    }

    #[test]
    fn build_response_when_existing_container_then_removed() {
        let (cache, id) = make_cache_with_entry();
        let resp = build_response(&id, &cache);
        assert!(resp.ok);
        assert!(resp.removed);
        assert!(resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_unknown_container_then_not_removed() {
        let cache = Mutex::new(ContainerCache::new(64, 64 * 1024 * 1024));
        let resp = build_response("c_nonexistent", &cache);
        assert!(!resp.ok);
        assert!(!resp.removed);
        assert!(!resp.diagnostics.is_empty());
    }
}
