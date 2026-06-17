//! The `pou_scope` MCP tool.
//!
//! Returns every variable visible to a single named POU. Implements
//! REQ-TOL-220 and REQ-TOL-221.

use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, InitialValueAssignmentKind, Library,
    LibraryElementKind, ProgramDeclaration, TypeReference, VarDecl, VariableType,
};
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PouScopeInput {
    pub sources: Vec<SourceInput>,
    /// Compiler options (dialect + optional feature-flag overrides).
    #[schemars(with = "serde_json::Value")]
    pub options: serde_json::Value,
    pub pou: String,
}

#[derive(Debug, Serialize)]
pub struct VariableEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub direction: String,
    pub initial_value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PouScopeResponse {
    pub ok: bool,
    pub found: bool,
    pub pou: String,
    pub variables: Vec<VariableEntry>,
    pub diagnostics: Vec<serde_json::Value>,
}

impl PouScopeResponse {
    fn empty(ok: bool, found: bool, pou: String, diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            ok,
            found,
            pou,
            variables: vec![],
            diagnostics,
        }
    }
}

/// Builds the `pou_scope` response.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
    pou_name: &str,
) -> PouScopeResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return PouScopeResponse::empty(
            false,
            false,
            pou_name.to_string(),
            serialize_diagnostics(&source_errors),
        );
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return PouScopeResponse::empty(
                false,
                false,
                pou_name.to_string(),
                serialize_diagnostics(&errs),
            );
        }
    };

    let mut project = MemoryBackedProject::new(options);
    for src in sources {
        project.add_source(FileId::from_string(&src.name), src.content.clone());
    }

    let mut diagnostics_json = match project.semantic() {
        Ok(()) => vec![],
        Err(diags) => serialize_diagnostics(&diags),
    };

    let has_errors = diagnostics_json
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));

    let library = match project.analyzed_library() {
        Some(lib) => lib,
        None => {
            return PouScopeResponse::empty(
                !has_errors,
                false,
                pou_name.to_string(),
                diagnostics_json,
            );
        }
    };

    // REQ-TOL-221: resolve against Programs, then Functions, then FBs.
    let variables = match resolve_pou(library, pou_name) {
        Some(vars) => vars,
        None => {
            diagnostics_json.push(not_found_diagnostic(pou_name));
            return PouScopeResponse {
                ok: false,
                found: false,
                pou: pou_name.to_string(),
                variables: vec![],
                diagnostics: diagnostics_json,
            };
        }
    };

    PouScopeResponse {
        ok: !has_errors,
        found: true,
        pou: pou_name.to_string(),
        variables,
        diagnostics: diagnostics_json,
    }
}

fn resolve_pou(library: &Library, pou_name: &str) -> Option<Vec<VariableEntry>> {
    let lower = pou_name.to_lowercase();

    // Programs first (REQ-TOL-221).
    for element in &library.elements {
        if let LibraryElementKind::ProgramDeclaration(p) = element {
            if p.name.to_string().to_lowercase() == lower {
                return Some(variables_from_program(p));
            }
        }
    }

    // Functions next.
    for element in &library.elements {
        if let LibraryElementKind::FunctionDeclaration(f) = element {
            if f.name.to_string().to_lowercase() == lower {
                return Some(variables_from_function(f));
            }
        }
    }

    // Function blocks last.
    for element in &library.elements {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
            if fb.name.to_string().to_lowercase() == lower {
                return Some(variables_from_function_block(fb));
            }
        }
    }

    None
}

fn variables_from_program(p: &ProgramDeclaration) -> Vec<VariableEntry> {
    p.variables.iter().map(variable_entry).collect()
}

fn variables_from_function(f: &FunctionDeclaration) -> Vec<VariableEntry> {
    f.variables.iter().map(variable_entry).collect()
}

fn variables_from_function_block(fb: &FunctionBlockDeclaration) -> Vec<VariableEntry> {
    fb.variables.iter().map(variable_entry).collect()
}

fn variable_entry(v: &VarDecl) -> VariableEntry {
    let name = v
        .identifier
        .symbolic_id()
        .map(|id| id.to_string())
        .unwrap_or_default();
    VariableEntry {
        name,
        type_name: render_type_name(&v.type_name()),
        direction: direction_of(&v.var_type),
        initial_value: render_initial_value(&v.initializer),
    }
}

fn render_type_name(t: &TypeReference) -> String {
    match t {
        TypeReference::Named(n) => n.to_string(),
        TypeReference::Inline => String::new(),
        TypeReference::Unspecified => String::new(),
    }
}

/// Maps `VariableType` to the short direction tag required by REQ-TOL-220.
fn direction_of(vt: &VariableType) -> String {
    match vt {
        VariableType::Input => "In".into(),
        VariableType::Output => "Out".into(),
        VariableType::InOut => "InOut".into(),
        VariableType::Global => "Global".into(),
        VariableType::External => "External".into(),
        // Var, Temp, AccessDeclarations, etc.
        _ => "Local".into(),
    }
}

/// Best-effort opaque rendering of an initial value.
///
/// Returns a display string when the initializer carries a primitive value
/// (integer/real/bool/string literal, enum value, or `NULL` reference);
/// returns `None` otherwise. Per REQ-TOL-220 the rendering is for display
/// only and is not guaranteed to be a parseable expression.
fn render_initial_value(init: &InitialValueAssignmentKind) -> Option<String> {
    match init {
        InitialValueAssignmentKind::Simple(s) => s.initial_value.as_ref().map(|c| c.to_string()),
        InitialValueAssignmentKind::String(s) => s
            .initial_value
            .as_ref()
            .map(|chars| format!("'{}'", chars.iter().collect::<String>())),
        InitialValueAssignmentKind::EnumeratedType(e) => {
            e.initial_value.as_ref().map(|v| v.value.to_string())
        }
        InitialValueAssignmentKind::EnumeratedValues(e) => {
            e.initial_value.as_ref().map(|v| v.value.to_string())
        }
        InitialValueAssignmentKind::Reference(r) => match &r.initial_value {
            Some(ironplc_dsl::common::ReferenceInitialValue::Null(_)) => Some("NULL".into()),
            Some(ironplc_dsl::common::ReferenceInitialValue::Ref(_)) => Some("REF".into()),
            None => None,
        },
        _ => None,
    }
}

fn not_found_diagnostic(pou_name: &str) -> serde_json::Value {
    serde_json::json!({
        "code": "P8001",
        "message": format!("No POU named '{}' found.", pou_name),
        "severity": "error",
        "file": "",
        "start": 0,
        "end": 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({"dialect": "iec61131-3-ed2"})
    }

    fn build(src: &str, pou: &str) -> PouScopeResponse {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: src.into(),
        }];
        build_response(&sources, &ed2_options(), pou)
    }

    #[test]
    fn build_response_when_program_then_variables_populated() {
        let resp = build(
            "PROGRAM p\nVAR_INPUT start : BOOL := FALSE; END_VAR\nVAR count : DINT := 0; END_VAR\nEND_PROGRAM",
            "p",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert_eq!(resp.pou, "p");
        let start = resp
            .variables
            .iter()
            .find(|v| v.name == "start")
            .expect("start not found");
        assert_eq!(start.type_name.to_uppercase(), "BOOL");
        assert_eq!(start.direction, "In");
        assert_eq!(start.initial_value.as_deref(), Some("FALSE"));

        let count = resp
            .variables
            .iter()
            .find(|v| v.name == "count")
            .expect("count not found");
        assert_eq!(count.type_name.to_uppercase(), "DINT");
        assert_eq!(count.direction, "Local");
        assert_eq!(count.initial_value.as_deref(), Some("0"));
    }

    #[test]
    fn build_response_when_var_output_then_direction_out() {
        let resp = build(
            "PROGRAM p\nVAR_OUTPUT run : BOOL; END_VAR\nEND_PROGRAM",
            "p",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .variables
            .iter()
            .find(|v| v.name == "run")
            .expect("run not found");
        assert_eq!(entry.direction, "Out");
        assert_eq!(entry.initial_value, None);
    }

    #[test]
    fn build_response_when_var_in_out_then_direction_inout() {
        let resp = build(
            "FUNCTION_BLOCK fb\nVAR_IN_OUT c : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR inst : fb; END_VAR\nEND_PROGRAM",
            "fb",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        let entry = resp
            .variables
            .iter()
            .find(|v| v.name == "c")
            .expect("c not found");
        assert_eq!(entry.direction, "InOut");
    }

    #[test]
    fn build_response_when_program_and_fb_same_name_then_program_wins() {
        // REQ-TOL-221: Programs resolve before function blocks.
        let resp = build(
            "FUNCTION_BLOCK Motor\nVAR fb_var : INT; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Motor\nVAR prog_var : INT; END_VAR\nEND_PROGRAM",
            "Motor",
        );
        assert!(resp.found, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.variables.iter().any(|v| v.name == "prog_var"));
        assert!(resp.variables.iter().all(|v| v.name != "fb_var"));
    }

    #[test]
    fn build_response_when_pou_not_found_then_found_false_and_p8001() {
        let resp = build("PROGRAM p\nEND_PROGRAM", "nonexistent");
        assert!(!resp.ok);
        assert!(!resp.found);
        assert_eq!(resp.pou, "nonexistent");
        assert!(resp.variables.is_empty());
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_function_block_then_variables_populated() {
        let resp = build(
            "FUNCTION_BLOCK fb\nVAR_INPUT x : INT := 5; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR i : fb; END_VAR\nEND_PROGRAM",
            "fb",
        );
        assert!(resp.found);
        let entry = resp
            .variables
            .iter()
            .find(|v| v.name == "x")
            .expect("x not found");
        assert_eq!(entry.type_name.to_uppercase(), "INT");
        assert_eq!(entry.direction, "In");
        assert_eq!(entry.initial_value.as_deref(), Some("5"));
    }

    #[test]
    fn build_response_when_function_then_variables_populated() {
        let resp = build(
            "FUNCTION f : INT\nVAR_INPUT a : INT := 1; END_VAR\nf := a;\nEND_FUNCTION\nPROGRAM p\nVAR r : INT; END_VAR\nr := f(a := 1);\nEND_PROGRAM",
            "f",
        );
        assert!(resp.found, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .variables
            .iter()
            .find(|v| v.name == "a")
            .expect("a not found");
        assert_eq!(entry.type_name.to_uppercase(), "INT");
        assert_eq!(entry.direction, "In");
    }

    #[test]
    fn build_response_when_empty_source_name_then_p8001() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), "p");
        assert!(!resp.ok);
        assert!(!resp.found);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_missing_dialect_then_p8001() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}), "p");
        assert!(!resp.ok);
        assert!(!resp.found);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false_but_partial_variables() {
        // Even when analysis fails, the AST is available and we can still
        // return a best-effort variables list.
        let resp = build("PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM", "p");
        assert!(!resp.ok);
        assert!(resp.found);
        assert!(!resp.variables.is_empty());
    }
}
