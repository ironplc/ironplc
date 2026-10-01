//! The `project_io` MCP tool.
//!
//! Returns every variable the caller can drive (`inputs`) and every variable
//! the caller can observe (`outputs`) across the supplied sources. Implements
//! REQ-TOL-mcp-210, REQ-TOL-mcp-211, REQ-TOL-mcp-212, and REQ-TOL-mcp-213.

use ironplc_analyzer::symbol_environment::{ScopeKind, SymbolInfo, SymbolKind};
use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::VariableType;
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use serde::Serialize;

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

/// A single input or output entry (REQ-TOL-mcp-212).
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

    let diagnostics_json = serialize_diagnostics(&project.semantic());

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

/// Where a variable was declared, as far as its classification goes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IoScope {
    /// Declared in a Program; named `<program>.<variable>`.
    Program,
    /// A global variable; named by its bare name.
    Global,
}

/// Walks Programs and Globals, classifying each variable into inputs and/or
/// outputs per REQ-TOL-mcp-210, REQ-TOL-mcp-211 and REQ-TOL-mcp-213.
fn collect_io(context: &SemanticContext) -> (Vec<IoEntry>, Vec<IoEntry>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    // Programs: name variables as `<program>.<variable>`.
    for (program_name, _) in context.symbols().get_programs() {
        let scope = ScopeKind::Named(program_name.clone().into());
        for (var_name, info) in context.symbols().get_variables_in_scope(&scope) {
            let qualified = format!("{}.{}", program_name, var_name);
            classify(
                &qualified,
                info,
                IoScope::Program,
                &mut inputs,
                &mut outputs,
            );
        }
    }

    // Globals: name is the bare variable name.
    for (var_name, info) in context.symbols().get_variables_in_scope(&ScopeKind::Global) {
        classify(
            &var_name.to_string(),
            info,
            IoScope::Global,
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
    scope: IoScope,
    inputs: &mut Vec<IoEntry>,
    outputs: &mut Vec<IoEntry>,
) {
    let addr = info.address.as_deref();

    // REQ-TOL-mcp-211: marker memory (`%M*`) is neither, whatever else the
    // declaration would qualify it for.
    if addr.is_some_and(|a| a.starts_with("%M")) {
        return;
    }

    let direction = direction_of(info);
    let is_program_scope = scope == IoScope::Program;
    let is_non_addressed_global = scope == IoScope::Global && addr.is_none();
    let is_hw_input = addr.is_some_and(|a| a.starts_with("%I"));
    let is_hw_output = addr.is_some_and(|a| a.starts_with("%Q"));

    // REQ-TOL-mcp-213: a constant, or a global the VM updates itself, cannot
    // be driven however it is declared.
    let is_writable = !info.is_constant() && !info.compiler_provided;

    // REQ-TOL-mcp-210: inputs.
    let is_input = is_writable
        && ((is_program_scope && matches!(direction, "In" | "InOut"))
            || direction == "External"
            || is_non_addressed_global
            || is_hw_input);

    // REQ-TOL-mcp-211: outputs.
    let is_output = (is_program_scope && matches!(direction, "Out" | "InOut"))
        || is_non_addressed_global
        || is_hw_output;

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
    use crate::tools::test_support::{
        ed2_options, source, unnamed_source, SEMANTIC_ERROR_PROGRAM, VALID_PROGRAM,
    };
    use spec_test_macro::spec_test;

    fn build(src: &str) -> ProjectIoResponse {
        build_response(&source(src), &ed2_options())
    }

    /// Builds with `flag` (a boolean option key) turned on.
    fn build_with(flag: &str, src: &str) -> ProjectIoResponse {
        let mut options = ed2_options();
        options[flag] = serde_json::Value::Bool(true);
        build_response(&source(src), &options)
    }

    fn names(entries: &[IoEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// A configuration whose `VAR_GLOBAL` block declares `globals`, running
    /// a program `p` whose body is `program_body`.
    fn configuration(globals: &str, program_body: &str) -> String {
        format!(
            "CONFIGURATION config\n\
             VAR_GLOBAL {globals} END_VAR\n\
             RESOURCE resource1 ON PLC\n\
             TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);\n\
             PROGRAM main_instance WITH plc_task : p;\n\
             END_RESOURCE\n\
             END_CONFIGURATION\n\
             PROGRAM p\n{program_body}\nEND_PROGRAM"
        )
    }

    /// Classifies a global declared at `address`: located globals do not
    /// parse in any `VAR_GLOBAL` block yet (#1913), so the symbol is built
    /// the way the analyzer records one.
    fn classify_global_at(address: &str) -> (Vec<IoEntry>, Vec<IoEntry>) {
        let info = SymbolInfo::new(
            SymbolKind::Variable,
            ScopeKind::Global,
            ironplc_dsl::core::SourceSpan::default(),
        )
        .with_variable_type(VariableType::Global)
        .with_address(address.to_string());
        let (mut inputs, mut outputs) = (vec![], vec![]);
        classify("g", &info, IoScope::Global, &mut inputs, &mut outputs);
        (inputs, outputs)
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
        // REQ-TOL-mcp-211: %M* variables are neither inputs nor outputs.
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
        let resp = build(SEMANTIC_ERROR_PROGRAM);
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_empty_source_name_then_p8001() {
        let resp = build_response(&unnamed_source(), &ed2_options());
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_missing_dialect_then_p8001() {
        let resp = build_response(&source(VALID_PROGRAM), &serde_json::json!({}));
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

    #[spec_test(REQ_TOL_mcp_211)]
    fn build_response_when_configuration_global_then_bare_name_in_inputs_and_outputs() {
        let resp = build(&configuration("shared : INT;", ""));
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(names(&resp.inputs), vec!["shared"]);
        assert_eq!(names(&resp.outputs), vec!["shared"]);
    }

    #[spec_test(REQ_TOL_mcp_210)]
    fn build_response_when_top_level_global_then_bare_name_in_inputs_and_outputs() {
        let resp = build_with(
            "allow_top_level_var_global",
            &format!("VAR_GLOBAL level : REAL; END_VAR\n{VALID_PROGRAM}"),
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(names(&resp.inputs), vec!["level"]);
        assert_eq!(names(&resp.outputs), vec!["level"]);
    }

    #[spec_test(REQ_TOL_mcp_210)]
    fn build_response_when_program_var_external_then_qualified_name_in_inputs_only() {
        let resp = build(&configuration(
            "shared : INT;",
            "VAR_EXTERNAL shared : INT; END_VAR",
        ));
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(names(&resp.inputs), vec!["p.shared", "shared"]);
        assert_eq!(names(&resp.outputs), vec!["shared"]);
    }

    #[spec_test(REQ_TOL_mcp_210)]
    fn classify_when_global_at_input_address_then_inputs_only() {
        let (inputs, outputs) = classify_global_at("%IX0.0");
        assert_eq!(names(&inputs), vec!["g"]);
        assert_eq!(inputs[0].address.as_deref(), Some("%IX0.0"));
        assert!(outputs.is_empty(), "outputs: {outputs:?}");
    }

    #[spec_test(REQ_TOL_mcp_211)]
    fn classify_when_global_at_output_address_then_outputs_only() {
        let (inputs, outputs) = classify_global_at("%QX0.0");
        assert!(inputs.is_empty(), "inputs: {inputs:?}");
        assert_eq!(names(&outputs), vec!["g"]);
        assert_eq!(outputs[0].address.as_deref(), Some("%QX0.0"));
    }

    #[spec_test(REQ_TOL_mcp_211)]
    fn classify_when_global_at_memory_address_then_in_neither() {
        let (inputs, outputs) = classify_global_at("%MW2");
        assert!(inputs.is_empty(), "inputs: {inputs:?}");
        assert!(outputs.is_empty(), "outputs: {outputs:?}");
    }

    #[spec_test(REQ_TOL_mcp_213)]
    fn build_response_when_constant_global_then_outputs_only() {
        let resp = build(&configuration(
            "CONSTANT limit : INT := 3;",
            "VAR_EXTERNAL CONSTANT limit : INT; END_VAR",
        ));
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.inputs.is_empty(), "inputs: {:?}", resp.inputs);
        assert_eq!(names(&resp.outputs), vec!["limit"]);
    }

    #[spec_test(REQ_TOL_mcp_213)]
    fn build_response_when_global_never_written_then_still_input() {
        // Inferred constant (constant-variable-inference.md), not declared so.
        let resp = build(&configuration("setpoint : INT := 3;", ""));
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(names(&resp.inputs), vec!["setpoint"]);
        assert_eq!(names(&resp.outputs), vec!["setpoint"]);
    }

    #[spec_test(REQ_TOL_mcp_213)]
    fn build_response_when_compiler_provided_global_then_outputs_only() {
        let resp = build_with("allow_system_uptime_global", VALID_PROGRAM);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.inputs.is_empty(), "inputs: {:?}", resp.inputs);
        assert_eq!(
            names(&resp.outputs),
            vec!["__SYSTEM_UP_LTIME", "__SYSTEM_UP_TIME"]
        );
    }
}
