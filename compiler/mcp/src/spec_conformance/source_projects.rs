//! Spec conformance for the project a `sources` entry belongs to
//! (REQ-ARC-mcp-013). Kept apart from `spec_conformance.rs`, which is past
//! the module size limit.

use spec_test_macro::spec_test;

use crate::tools;
use crate::tools::common::{validate_sources, SourceInput};

/// `BROTLib` declares `FB_Blinker`; `main.st` names it with its prefix.
fn sources(project: Option<&str>) -> Vec<SourceInput> {
    vec![
        SourceInput {
            name: "brot.st".into(),
            content: "FUNCTION_BLOCK FB_Blinker END_FUNCTION_BLOCK".into(),
            project: project.map(Into::into),
        },
        SourceInput {
            name: "main.st".into(),
            content: "PROGRAM main VAR b : BROTLib.FB_Blinker; END_VAR END_PROGRAM".into(),
            project: None,
        },
    ]
}

/// REQ-ARC-mcp-013: A `sources` entry may name the project the file belongs
/// to, so a library prefix on a type name may name the project whose files
/// declare the type. A `project` is validated like a `name`.
#[spec_test(REQ_ARC_mcp_013)]
fn mcp_spec_req_arc_013_source_project_names_library_for_prefix() {
    let options = serde_json::json!({ "dialect": "twincat" });

    let with_project = tools::check::build_response(&sources(Some("BROTLib")), &options);
    assert!(with_project.ok, "{:?}", with_project.diagnostics);

    // Without the project, no library declares `FB_Blinker`.
    let without = tools::check::build_response(&sources(None), &options);
    assert!(!without.ok);
    assert_eq!(without.diagnostics[0]["code"], "P2046");

    assert_eq!(validate_sources(&sources(Some(""))).len(), 1);
}
