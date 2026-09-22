//! The `list_options` MCP tool.
//!
//! Returns the set of compiler options (dialects and feature flags) that
//! callers may pass in an `options` object to analysis, context, and
//! execution tools.

use ironplc_parser::options::{CompilerOptions, Dialect};
use serde::Serialize;

/// Top-level response for the `list_options` tool.
#[derive(Debug, Serialize)]
pub struct ListOptionsResponse {
    pub dialects: Vec<DialectInfo>,
    pub flags: Vec<FlagInfo>,
}

/// Metadata for a single dialect preset.
#[derive(Debug, Serialize)]
pub struct DialectInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
}

/// Metadata for a single feature flag.
#[derive(Debug, Serialize)]
pub struct FlagInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub flag_type: String,
    pub default: serde_json::Value,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<String>>,
}

/// Builds the full `list_options` response from the compiler's dialect
/// and feature-flag metadata.
pub fn build_response() -> ListOptionsResponse {
    let dialects = Dialect::ALL
        .iter()
        .map(|d| DialectInfo {
            id: d.to_string(),
            display_name: d.display_name().to_string(),
            description: d.description().to_string(),
        })
        .collect();

    let mut flags = Vec::with_capacity(
        CompilerOptions::FEATURE_DESCRIPTORS.len() + CompilerOptions::POLICY_DESCRIPTORS.len(),
    );

    // All dialect-extension flags from the macro-generated descriptors.
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        flags.push(FlagInfo {
            id: fd.option_key.to_string(),
            flag_type: "bool".into(),
            default: serde_json::Value::Bool(false),
            description: fd.description.to_string(),
            allowed_values: None,
        });
    }

    // Behavior policies (ADR-0049): an enum option whose value is one of the
    // policy's alternatives.
    for pd in CompilerOptions::POLICY_DESCRIPTORS {
        flags.push(FlagInfo {
            id: pd.option_key.to_string(),
            flag_type: "enum".into(),
            default: serde_json::Value::String(pd.default.to_string()),
            description: pd.description.to_string(),
            allowed_values: Some(pd.alternatives.iter().map(|a| a.to_string()).collect()),
        });
    }

    ListOptionsResponse { dialects, flags }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_response_when_called_then_returns_all_dialects() {
        let resp = build_response();
        assert_eq!(resp.dialects.len(), Dialect::ALL.len());
    }

    #[test]
    fn build_response_when_called_then_dialect_ids_match_display_format() {
        let resp = build_response();
        let ids: Vec<&str> = resp.dialects.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"iec61131-3-ed2"));
        assert!(ids.contains(&"iec61131-3-ed3"));
        assert!(ids.contains(&"rusty"));
        assert!(ids.contains(&"codesys"));
    }

    #[test]
    fn build_response_when_called_then_contains_every_feature_and_policy_descriptor() {
        // Derived from the sources of truth (`FEATURE_DESCRIPTORS` and
        // `POLICY_DESCRIPTORS`) rather than a hard-coded count, so adding a
        // flag or a policy does not force an edit here. That each flag gates
        // real compiler behavior — not just sets a struct field — is proven
        // behaviorally in the `feature_flag_conformance` suite.
        let resp = build_response();
        assert_eq!(
            resp.flags.len(),
            CompilerOptions::FEATURE_DESCRIPTORS.len() + CompilerOptions::POLICY_DESCRIPTORS.len()
        );
    }

    #[test]
    fn build_response_when_called_then_feature_flags_are_bool_with_false_default() {
        let resp = build_response();
        for fd in CompilerOptions::FEATURE_DESCRIPTORS {
            let flag = resp.flags.iter().find(|f| f.id == fd.option_key).unwrap();
            assert_eq!(flag.flag_type, "bool");
            assert_eq!(flag.default, serde_json::Value::Bool(false));
            assert!(flag.allowed_values.is_none());
        }
    }

    #[test]
    fn build_response_when_called_then_policies_are_enum_with_alternatives() {
        let resp = build_response();
        for pd in CompilerOptions::POLICY_DESCRIPTORS {
            let flag = resp.flags.iter().find(|f| f.id == pd.option_key).unwrap();
            assert_eq!(flag.flag_type, "enum");
            assert_eq!(
                flag.default,
                serde_json::Value::String(pd.default.to_string())
            );
            let allowed = flag.allowed_values.as_ref().unwrap();
            assert_eq!(allowed, &pd.alternatives.to_vec());
        }
    }

    #[test]
    fn build_response_when_called_then_contains_c_style_comments_flag() {
        let resp = build_response();
        assert!(resp.flags.iter().any(|f| f.id == "allow_c_style_comments"));
    }

    #[test]
    fn build_response_when_serialized_then_valid_json() {
        let resp = build_response();
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["dialects"].is_array());
        assert!(parsed["flags"].is_array());
    }

    #[test]
    fn build_response_when_called_then_each_dialect_has_display_name_and_description() {
        let resp = build_response();
        for d in &resp.dialects {
            assert!(!d.display_name.is_empty());
            assert!(!d.description.is_empty());
        }
    }

    #[test]
    fn build_response_when_called_then_each_flag_has_nonempty_description() {
        let resp = build_response();
        for f in &resp.flags {
            assert!(
                !f.description.is_empty(),
                "flag {} has empty description",
                f.id
            );
        }
    }
}
