use ironplc_analyzer::symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind};
use ironplc_analyzer::{IntermediateType, SemanticContext, TypeCategory};
use ironplc_dsl::common::VariableType;
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

const MAX_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SymbolsInput {
    pub sources: Vec<SourceInput>,
    #[schemars(with = "serde_json::Value")]
    pub options: serde_json::Value,
    #[serde(default)]
    pub pou: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<bool>,
    pub programs: Vec<ProgramSymbol>,
    pub functions: Vec<FunctionSymbol>,
    pub function_blocks: Vec<FunctionBlockSymbol>,
    pub types: Vec<TypeSymbol>,
    pub truncated: bool,
    pub diagnostics: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ProgramSymbol {
    pub name: String,
    pub variables: Vec<VariableInfo>,
}

#[derive(Debug, Serialize)]
pub struct FunctionSymbol {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<VariableInfo>,
}

#[derive(Debug, Serialize)]
pub struct FunctionBlockSymbol {
    pub name: String,
    pub variables: Vec<VariableInfo>,
}

#[derive(Debug, Serialize)]
pub struct VariableInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub direction: String,
    pub address: Option<String>,
    pub external: bool,
}

#[derive(Debug, Serialize)]
pub struct TypeSymbol {
    pub name: String,
    pub kind: String,
}

pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
    pou_filter: Option<&str>,
) -> SymbolsResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return empty_response(false, None, false, serialize_diagnostics(&source_errors));
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return empty_response(false, None, false, serialize_diagnostics(&errs));
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
        Some(ctx) if !has_errors => ctx,
        _ => {
            return empty_response(false, None, false, diagnostics_json);
        }
    };

    let mut programs = extract_programs(context);
    let mut functions = extract_functions(context);
    let mut function_blocks = extract_function_blocks(context);
    let mut types = extract_types(context);

    if let Some(pou_name) = pou_filter {
        return apply_pou_filter(
            pou_name,
            &mut programs,
            &mut functions,
            &mut function_blocks,
            &mut types,
            diagnostics_json,
        );
    }

    let response = SymbolsResponse {
        ok: true,
        found: None,
        programs,
        functions,
        function_blocks,
        types,
        truncated: false,
        diagnostics: diagnostics_json,
    };

    if let Ok(json) = serde_json::to_string(&response) {
        if json.len() > MAX_RESPONSE_BYTES {
            return empty_response(
                false,
                None,
                true,
                vec![serde_json::json!({
                    "code": "P8001",
                    "message": "Response exceeds 256 KiB limit. Use the `pou` filter or a context tool (`pou_scope`, `project_io`, `types_all`) instead.",
                    "severity": "error",
                    "file": "",
                    "start": 0,
                    "end": 0
                })],
            );
        }
    }

    response
}

fn extract_programs(context: &SemanticContext) -> Vec<ProgramSymbol> {
    context
        .symbols()
        .get_global_symbols()
        .iter()
        .filter(|(_, info)| info.kind == SymbolKind::Program)
        .map(|(name, _)| {
            let scope = ScopeKind::Named(name.clone());
            let variables = extract_variables_from_scope(context.symbols(), &scope);
            ProgramSymbol {
                name: name.to_string(),
                variables,
            }
        })
        .collect()
}

fn extract_functions(context: &SemanticContext) -> Vec<FunctionSymbol> {
    context
        .functions()
        .iter()
        .filter(|(_, sig)| !sig.is_stdlib())
        .map(|(_, sig)| {
            let params: Vec<VariableInfo> = sig
                .parameters
                .iter()
                .map(|p| {
                    let direction = if p.is_inout {
                        "InOut"
                    } else if p.is_output {
                        "Out"
                    } else {
                        "In"
                    };
                    VariableInfo {
                        name: p.name.to_string(),
                        type_name: p.param_type.to_string(),
                        direction: direction.to_string(),
                        address: None,
                        external: false,
                    }
                })
                .collect();
            FunctionSymbol {
                name: sig.name.to_string(),
                return_type: sig
                    .return_type
                    .as_ref()
                    .map(|rt| rt.to_type_name().to_string())
                    .unwrap_or_default(),
                parameters: params,
            }
        })
        .collect()
}

fn extract_function_blocks(context: &SemanticContext) -> Vec<FunctionBlockSymbol> {
    context
        .symbols()
        .get_global_symbols()
        .iter()
        .filter(|(_, info)| info.kind == SymbolKind::FunctionBlock)
        .map(|(name, _)| {
            let scope = ScopeKind::Named(name.clone());
            let variables = extract_variables_from_scope(context.symbols(), &scope);
            FunctionBlockSymbol {
                name: name.to_string(),
                variables,
            }
        })
        .collect()
}

fn extract_types(context: &SemanticContext) -> Vec<TypeSymbol> {
    context
        .types()
        .iter()
        .filter(|(_, attrs)| attrs.type_category != TypeCategory::Elementary)
        .filter(|(_, attrs)| {
            !matches!(attrs.representation, IntermediateType::FunctionBlock { .. })
        })
        .filter(|(_, attrs)| !matches!(attrs.representation, IntermediateType::Function { .. }))
        .map(|(name, attrs)| {
            let kind = match &attrs.representation {
                IntermediateType::Enumeration { .. } => "enumeration",
                IntermediateType::Structure { .. } => "structure",
                IntermediateType::Array { .. } => "array",
                IntermediateType::Subrange { .. } => "subrange",
                IntermediateType::String { .. } => "string",
                IntermediateType::Reference { .. } => "reference",
                _ => "alias",
            };
            TypeSymbol {
                name: name.to_string(),
                kind: kind.to_string(),
            }
        })
        .collect()
}

fn extract_variables_from_scope(
    symbols: &SymbolEnvironment,
    scope: &ScopeKind,
) -> Vec<VariableInfo> {
    let Some(scope_symbols) = symbols.get_scope_symbols(scope) else {
        return vec![];
    };
    scope_symbols
        .iter()
        .filter(|(_, info)| {
            matches!(
                info.kind,
                SymbolKind::Variable
                    | SymbolKind::Parameter
                    | SymbolKind::OutputParameter
                    | SymbolKind::InOutParameter
            )
        })
        .map(|(name, info)| {
            let direction = match &info.variable_type {
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
            };
            let external = matches!(direction, "In" | "InOut" | "External" | "Global")
                || info.address.as_ref().is_some_and(|a| a.starts_with("%I"));
            VariableInfo {
                name: name.to_string(),
                type_name: info.data_type.clone().unwrap_or_default(),
                direction: direction.to_string(),
                address: info.address.clone(),
                external,
            }
        })
        .collect()
}

fn apply_pou_filter(
    pou_name: &str,
    programs: &mut Vec<ProgramSymbol>,
    functions: &mut Vec<FunctionSymbol>,
    function_blocks: &mut Vec<FunctionBlockSymbol>,
    types: &mut Vec<TypeSymbol>,
    diagnostics: Vec<serde_json::Value>,
) -> SymbolsResponse {
    let pou_lower = pou_name.to_lowercase();

    let matched_program = programs
        .iter()
        .position(|p| p.name.to_lowercase() == pou_lower);
    let matched_function = functions
        .iter()
        .position(|f| f.name.to_lowercase() == pou_lower);
    let matched_fb = function_blocks
        .iter()
        .position(|fb| fb.name.to_lowercase() == pou_lower);

    if matched_program.is_none() && matched_function.is_none() && matched_fb.is_none() {
        let mut diags = diagnostics;
        diags.push(serde_json::json!({
            "code": "P8001",
            "message": format!("No POU named '{}' found.", pou_name),
            "severity": "error",
            "file": "",
            "start": 0,
            "end": 0
        }));
        return empty_response(false, Some(false), false, diags);
    }

    let mut referenced_types = std::collections::HashSet::new();

    let filtered_programs = if let Some(idx) = matched_program {
        let p = programs.swap_remove(idx);
        for v in &p.variables {
            if !v.type_name.is_empty() {
                referenced_types.insert(v.type_name.to_lowercase());
            }
        }
        vec![p]
    } else {
        vec![]
    };

    let filtered_functions = if let Some(idx) = matched_function {
        let f = functions.swap_remove(idx);
        if !f.return_type.is_empty() {
            referenced_types.insert(f.return_type.to_lowercase());
        }
        for v in &f.parameters {
            if !v.type_name.is_empty() {
                referenced_types.insert(v.type_name.to_lowercase());
            }
        }
        vec![f]
    } else {
        vec![]
    };

    let filtered_fbs = if let Some(idx) = matched_fb {
        let fb = function_blocks.swap_remove(idx);
        for v in &fb.variables {
            if !v.type_name.is_empty() {
                referenced_types.insert(v.type_name.to_lowercase());
            }
        }
        vec![fb]
    } else {
        vec![]
    };

    let filtered_types: Vec<TypeSymbol> = types
        .drain(..)
        .filter(|t| referenced_types.contains(&t.name.to_lowercase()))
        .collect();

    SymbolsResponse {
        ok: true,
        found: Some(true),
        programs: filtered_programs,
        functions: filtered_functions,
        function_blocks: filtered_fbs,
        types: filtered_types,
        truncated: false,
        diagnostics,
    }
}

fn empty_response(
    ok: bool,
    found: Option<bool>,
    truncated: bool,
    diagnostics: Vec<serde_json::Value>,
) -> SymbolsResponse {
    SymbolsResponse {
        ok,
        found,
        programs: vec![],
        functions: vec![],
        function_blocks: vec![],
        types: vec![],
        truncated,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({"dialect": "iec61131-3-ed2"})
    }

    #[test]
    fn build_response_when_valid_program_then_ok_true() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    }

    #[test]
    fn build_response_when_program_with_vars_then_variables_populated() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.programs.len(), 1);
        assert_eq!(resp.programs[0].name, "p");
        assert!(!resp.programs[0].variables.is_empty());
    }

    #[test]
    fn build_response_when_var_input_then_direction_is_in() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content:
                "FUNCTION_BLOCK fb\nVAR_INPUT x : INT; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR inst : fb; END_VAR\nEND_PROGRAM"
                    .into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let fb = resp
            .function_blocks
            .iter()
            .find(|fb| fb.name == "fb")
            .expect("fb not found");
        let var = fb
            .variables
            .iter()
            .find(|v| v.name == "x")
            .expect("x not found");
        assert_eq!(var.direction, "In");
        assert!(var.external);
    }

    #[test]
    fn build_response_when_local_var_then_direction_is_local() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let var = &resp.programs[0].variables[0];
        assert_eq!(var.direction, "Local");
        assert!(!var.external);
    }

    #[test]
    fn build_response_when_function_then_return_type_and_params() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content:
                "FUNCTION f : INT\nVAR_INPUT a : INT; END_VAR\nf := a;\nEND_FUNCTION\nPROGRAM p\nVAR r : INT; END_VAR\nr := f(a := 1);\nEND_PROGRAM"
                    .into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(!resp.functions.is_empty());
        let func = &resp.functions[0];
        assert_eq!(func.name, "f");
        assert_eq!(func.return_type, "INT");
        assert_eq!(func.parameters.len(), 1);
        assert_eq!(func.parameters[0].direction, "In");
    }

    #[test]
    fn build_response_when_enum_type_then_types_has_enumeration() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "TYPE\nMyEnum : (A, B, C);\nEND_TYPE\nPROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let t = resp
            .types
            .iter()
            .find(|t| t.name == "MyEnum")
            .expect("MyEnum not found");
        assert_eq!(t.kind, "enumeration");
    }

    #[test]
    fn build_response_when_pou_filter_then_narrowed_response() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM a\nEND_PROGRAM\nPROGRAM b\nVAR x : INT; END_VAR\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), Some("b"));
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.found, Some(true));
        assert_eq!(resp.programs.len(), 1);
        assert_eq!(resp.programs[0].name, "b");
    }

    #[test]
    fn build_response_when_pou_filter_unknown_then_found_false() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), Some("nonexistent"));
        assert!(!resp.ok);
        assert_eq!(resp.found, Some(false));
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_invalid_sources_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), None);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_invalid_options_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}), None);
        assert!(!resp.ok);
    }
}
