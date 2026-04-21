//! The `project_io` MCP tool.
//!
//! Returns every variable the caller can drive (`inputs`) and every variable
//! the caller can observe (`outputs`) across the supplied sources. Implements
//! REQ-TOL-210, REQ-TOL-211, and REQ-TOL-212.

use ironplc_analyzer::symbol_environment::{ScopeKind, SymbolInfo, SymbolKind};
use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::VariableType;
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use serde::Serialize;

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

/// A single input or output entry (REQ-TOL-212).
#[derive(Debug, Clone, Serialize)]
pub struct IoEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub address: Option<String>,
}

/// Response returned by the `project_io` tool.
#[derive(Debug, Serialize)]
pub struct ProjectIoResponse {
    pub ok: bool,
    pub inputs: Vec<IoEntry>,
    pub outputs: Vec<IoEntry>,
    pub diagnostics: Vec<serde_json::Value>,
}

impl ProjectIoResponse {
    fn empty(ok: bool, diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            ok,
            inputs: vec![],
            outputs: vec![],
            diagnostics,
        }
    }
}

/// Builds the `project_io` response from raw inputs.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
) -> ProjectIoResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return ProjectIoResponse::empty(false, serialize_diagnostics(&source_errors));
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return ProjectIoResponse::empty(false, serialize_diagnostics(&errs));
        }
    };

    let mut project = MemoryBackedProject::new(options);
    for src in sources {
        project.add_source(FileId::from_string(&src.name), src.content.clone());
    }

    let diagnostics_json = match project.semantic() {
        Ok(()) => vec![],
        Err(diags) => serialize_diagnostics(&diags),
    };

    let has_errors = diagnostics_json
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));

    let context = match project.semantic_context() {
        Some(ctx) => ctx,
        None => {
            return ProjectIoResponse::empty(!has_errors, diagnostics_json);
        }
    };

    let (mut inputs, mut outputs) = collect_io(context);
    inputs.sort_by(|a, b| a.name.cmp(&b.name));
    outputs.sort_by(|a, b| a.name.cmp(&b.name));

    ProjectIoResponse {
        ok: !has_errors,
        inputs,
        outputs,
        diagnostics: diagnostics_json,
    }
}

/// Walks Programs and Globals, classifying each variable into inputs and/or
/// outputs per REQ-TOL-210 and REQ-TOL-211.
fn collect_io(context: &SemanticContext) -> (Vec<IoEntry>, Vec<IoEntry>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    // Programs: name variables as `<program>.<variable>`.
    for (program_name, _) in context.symbols().get_programs() {
        let scope = ScopeKind::Named(program_name.clone());
        for (var_name, info) in context.symbols().get_variables_in_scope(&scope) {
            let qualified = format!("{}.{}", program_name, var_name);
            classify(&qualified, info, true, false, &mut inputs, &mut outputs);
        }
    }

    // Globals: name is the bare variable name.
    for (var_name, info) in context.symbols().get_variables_in_scope(&ScopeKind::Global) {
        classify(
            &var_name.to_string(),
            info,
            false,
            true,
            &mut inputs,
            &mut outputs,
        );
    }

    (inputs, outputs)
}

/// Appends the variable to `inputs` and/or `outputs` based on its role.
fn classify(
    qualified_name: &str,
    info: &SymbolInfo,
    is_program_scope: bool,
    is_global_scope: bool,
    inputs: &mut Vec<IoEntry>,
    outputs: &mut Vec<IoEntry>,
) {
    let direction = direction_of(info);
    let addr = info.address.as_deref();

    let is_hw_input = addr.is_some_and(|a| a.starts_with("%I"));
    let is_hw_output = addr.is_some_and(|a| a.starts_with("%Q"));
    let is_hw_memory = addr.is_some_and(|a| a.starts_with("%M"));

    // REQ-TOL-210: inputs.
    let is_input = (is_program_scope && matches!(direction, "In" | "InOut"))
        || direction == "External"
        || (is_global_scope && addr.is_none())
        || is_hw_input;

    // REQ-TOL-211: outputs.
    let is_output = (is_program_scope && matches!(direction, "Out" | "InOut"))
        || (is_global_scope && addr.is_none())
        || is_hw_output;

    // %M* memory is neither — already excluded by the rules above, but keep
    // this explicit for clarity.
    let _ = is_hw_memory;

    if is_input {
        inputs.push(entry(qualified_name, info));
    }
    if is_output {
        outputs.push(entry(qualified_name, info));
    }
}

fn entry(name: &str, info: &SymbolInfo) -> IoEntry {
    IoEntry {
        name: name.to_string(),
        type_name: info.data_type.clone().unwrap_or_default(),
        address: info.address.clone(),
    }
}

/// Translates `SymbolInfo` into a short direction tag, mirroring the same
/// logic used by `symbols.rs`.
fn direction_of(info: &SymbolInfo) -> &'static str {
    match &info.variable_type {
        Some(VariableType::Input) => "In",
        Some(VariableType::Output) => "Out",
        Some(VariableType::InOut) => "InOut",
        Some(VariableType::Global) => "Global",
        Some(VariableType::External) => "External",
        _ => match info.kind {
            SymbolKind::Parameter => "In",
            SymbolKind::OutputParameter => "Out",
            SymbolKind::InOutParameter => "InOut",
            _ => "Local",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({"dialect": "iec61131-3-ed2"})
    }

    fn build(src: &str) -> ProjectIoResponse {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: src.into(),
        }];
        build_response(&sources, &ed2_options())
    }

    #[test]
    fn build_response_when_program_with_var_input_then_listed_in_inputs() {
        let resp = build("PROGRAM p\nVAR_INPUT a : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.inputs.iter().any(|e| e.name == "p.a"));
        assert!(resp.outputs.iter().all(|e| e.name != "p.a"));
    }

    #[test]
    fn build_response_when_program_with_var_output_then_listed_in_outputs() {
        let resp = build("PROGRAM p\nVAR_OUTPUT b : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.outputs.iter().any(|e| e.name == "p.b"));
        assert!(resp.inputs.iter().all(|e| e.name != "p.b"));
    }

    #[test]
    fn build_response_when_program_with_var_in_out_then_listed_in_both() {
        let resp = build("PROGRAM p\nVAR_IN_OUT c : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.inputs.iter().any(|e| e.name == "p.c"));
        assert!(resp.outputs.iter().any(|e| e.name == "p.c"));
    }

    #[test]
    fn build_response_when_variable_has_input_address_then_address_populated() {
        let resp = build("PROGRAM p\nVAR button AT %IX0.0 : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .inputs
            .iter()
            .find(|e| e.address.as_deref() == Some("%IX0.0"));
        assert!(entry.is_some(), "inputs: {:?}", resp.inputs);
    }

    #[test]
    fn build_response_when_variable_has_output_address_then_in_outputs() {
        let resp = build("PROGRAM p\nVAR buzzer AT %QX0.0 : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .outputs
            .iter()
            .find(|e| e.address.as_deref() == Some("%QX0.0"));
        assert!(entry.is_some(), "outputs: {:?}", resp.outputs);
    }

    #[test]
    fn build_response_when_variable_has_memory_address_then_in_neither() {
        // REQ-TOL-211: %M* variables are neither inputs nor outputs.
        let resp = build("PROGRAM p\nVAR counter AT %MX0.0 : BOOL; END_VAR\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(
            !resp
                .inputs
                .iter()
                .any(|e| e.address.as_deref() == Some("%MX0.0")),
            "inputs: {:?}",
            resp.inputs
        );
        assert!(
            !resp
                .outputs
                .iter()
                .any(|e| e.address.as_deref() == Some("%MX0.0")),
            "outputs: {:?}",
            resp.outputs
        );
    }

    #[test]
    fn build_response_when_multiple_io_then_sorted_lexicographically() {
        let resp = build(
            "PROGRAM p\nVAR_INPUT zeta : BOOL; alpha : BOOL; mid : BOOL; END_VAR\nEND_PROGRAM",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let names: Vec<String> = resp.inputs.iter().map(|e| e.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false_with_diagnostics() {
        let resp = build("PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM");
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_empty_source_name_then_p8001() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_missing_dialect_then_p8001() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}));
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_program_with_input_and_output_then_both_classified() {
        // `type` population from `SymbolInfo.data_type` is not yet wired for
        // program parameters (same gap the `symbols` tool has today). This
        // test confirms the classification; the type string is best-effort.
        let resp = build(
            "PROGRAM p\nVAR_INPUT start : BOOL; END_VAR\nVAR_OUTPUT count : INT; END_VAR\nEND_PROGRAM",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.inputs.iter().any(|e| e.name == "p.start"));
        assert!(resp.outputs.iter().any(|e| e.name == "p.count"));
    }
}
