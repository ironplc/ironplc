//! The `compile` MCP tool.
//!
//! Runs the full pipeline (parse → semantic analysis → codegen) on the
//! supplied sources and returns an opaque container handle plus task and
//! program metadata.

use std::sync::Mutex;

use base64::Engine;
use ironplc_dsl::common::Library;
use ironplc_dsl::configuration::{
    ConfigurationDeclaration, ProgramConfiguration, TaskConfiguration,
};
use ironplc_dsl::core::{FileId, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use ironplc_project::project::{MemoryBackedProject, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    parse_options, serialize_diagnostic, serialize_diagnostics, validate_sources, SourceInput,
};
use crate::cache::{CachedContainer, ContainerCache, InsertError, ProgramMeta, TaskMeta};

/// Combined input accepted by `compile`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileInput {
    /// One or more source files.
    pub sources: Vec<SourceInput>,
    /// Compiler options (dialect + optional feature-flag overrides).
    #[schemars(with = "serde_json::Value")]
    pub options: serde_json::Value,
    /// When `true`, the response includes `container_base64`.
    #[serde(default)]
    pub include_bytes: bool,
}

/// Top-level response for the `compile` tool.
#[derive(Debug, Serialize)]
pub struct CompileResponse {
    pub ok: bool,
    pub container_id: Option<String>,
    pub container_base64: Option<String>,
    pub tasks: Vec<TaskInfo>,
    pub programs: Vec<ProgramInfo>,
    pub diagnostics: Vec<serde_json::Value>,
}

/// Task metadata in the compile response.
#[derive(Debug, Serialize)]
pub struct TaskInfo {
    pub name: String,
    pub priority: u32,
    pub kind: String,
    pub interval_ms: Option<f64>,
}

/// Program metadata in the compile response.
#[derive(Debug, Serialize)]
pub struct ProgramInfo {
    pub name: String,
    pub task: Option<String>,
}

/// Builds the compile response from raw inputs.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
    include_bytes: bool,
    cache: &Mutex<ContainerCache>,
) -> CompileResponse {
    // Validate sources (shared infra)
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return CompileResponse {
            ok: false,
            container_id: None,
            container_base64: None,
            tasks: vec![],
            programs: vec![],
            diagnostics: serialize_diagnostics(&source_errors),
        };
    }

    // Parse options (shared infra)
    let compiler_options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return CompileResponse {
                ok: false,
                container_id: None,
                container_base64: None,
                tasks: vec![],
                programs: vec![],
                diagnostics: serialize_diagnostics(&errs),
            };
        }
    };

    // Construct a fresh in-memory project (REQ-ARC-010)
    let mut project = MemoryBackedProject::new(compiler_options);

    // Load sources (REQ-ARC-011)
    for src in sources {
        project.add_source(FileId::from_string(&src.name), src.content.clone());
    }

    // Run semantic analysis
    let mut diagnostics: Vec<serde_json::Value> = Vec::new();
    if let Err(errs) = project.semantic() {
        diagnostics = serialize_diagnostics(&errs);
    }

    // Check if we can proceed to codegen
    let has_errors = diagnostics
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));
    let library = project.analyzed_library();
    let context = project.semantic_context();

    if has_errors || library.is_none() || context.is_none() {
        return CompileResponse {
            ok: false,
            container_id: None,
            container_base64: None,
            tasks: vec![],
            programs: vec![],
            diagnostics,
        };
    }

    let library = library.unwrap();
    let context = context.unwrap();

    // Run codegen
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: compiler_options.allow_system_uptime_global,
    };
    let container = match ironplc_codegen::compile(library, context, &codegen_options) {
        Ok(c) => c,
        Err(err) => {
            diagnostics.push(serialize_diagnostic(&err));
            return CompileResponse {
                ok: false,
                container_id: None,
                container_base64: None,
                tasks: vec![],
                programs: vec![],
                diagnostics,
            };
        }
    };

    // Serialize container to bytes
    let mut bytes = Vec::new();
    if let Err(e) = container.write_to(&mut bytes) {
        let err = Diagnostic::problem(
            Problem::InternalError,
            Label::span(
                SourceSpan::default(),
                format!("Failed to serialize container: {e}"),
            ),
        );
        diagnostics.push(serialize_diagnostic(&err));
        return CompileResponse {
            ok: false,
            container_id: None,
            container_base64: None,
            tasks: vec![],
            programs: vec![],
            diagnostics,
        };
    }

    // Extract task/program metadata from the AST
    let (task_metas, program_metas) = extract_task_program_metadata(library);

    // Build response task/program info
    let task_infos: Vec<TaskInfo> = task_metas
        .iter()
        .map(|t| TaskInfo {
            name: t.name.clone(),
            priority: t.priority,
            kind: t.kind.clone(),
            interval_ms: t.interval_ms,
        })
        .collect();
    let program_infos: Vec<ProgramInfo> = program_metas
        .iter()
        .map(|p| ProgramInfo {
            name: p.name.clone(),
            task: p.task.clone(),
        })
        .collect();

    // Optionally encode as base64
    let container_base64 = if include_bytes {
        Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
    } else {
        None
    };

    // Cache the container
    let cached = CachedContainer::new(bytes, task_metas, program_metas);
    let container_id = {
        let mut guard = cache.lock().unwrap();
        match guard.insert(cached) {
            Ok(id) => id,
            Err(InsertError::TooLarge { size, max }) => {
                let err = Diagnostic::problem(
                    Problem::InternalError,
                    Label::span(
                        SourceSpan::default(),
                        format!(
                            "Compiled container ({size} bytes) exceeds cache byte budget ({max} bytes)"
                        ),
                    ),
                );
                diagnostics.push(serialize_diagnostic(&err));
                return CompileResponse {
                    ok: false,
                    container_id: None,
                    container_base64: None,
                    tasks: task_infos,
                    programs: program_infos,
                    diagnostics,
                };
            }
        }
    };

    CompileResponse {
        ok: true,
        container_id: Some(container_id),
        container_base64,
        tasks: task_infos,
        programs: program_infos,
        diagnostics,
    }
}

/// Extracts task and program metadata from the Library's configuration.
fn extract_task_program_metadata(library: &Library) -> (Vec<TaskMeta>, Vec<ProgramMeta>) {
    // Find the first ConfigurationDeclaration
    let config = library.elements.iter().find_map(|e| {
        if let ironplc_dsl::common::LibraryElementKind::ConfigurationDeclaration(c) = e {
            Some(c)
        } else {
            None
        }
    });

    match config {
        Some(config) => extract_from_configuration(config),
        None => {
            // No configuration: synthesize a default from the first PROGRAM
            let program_name = library
                .elements
                .iter()
                .find_map(|e| {
                    if let ironplc_dsl::common::LibraryElementKind::ProgramDeclaration(p) = e {
                        Some(p.name.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "default".to_string());

            let tasks = vec![TaskMeta {
                name: program_name.clone(),
                priority: 0,
                kind: "event".to_string(),
                interval_ms: None,
            }];
            let programs = vec![ProgramMeta {
                name: program_name.clone(),
                task: Some(program_name),
            }];
            (tasks, programs)
        }
    }
}

fn extract_from_configuration(
    config: &ConfigurationDeclaration,
) -> (Vec<TaskMeta>, Vec<ProgramMeta>) {
    let mut tasks = Vec::new();
    let mut programs = Vec::new();

    for resource in &config.resource_decl {
        for task in &resource.tasks {
            tasks.push(task_meta_from_config(task));
        }
        for prog in &resource.programs {
            programs.push(program_meta_from_config(prog));
        }
    }

    (tasks, programs)
}

fn task_meta_from_config(task: &TaskConfiguration) -> TaskMeta {
    let kind = if task.interval.is_some() {
        "cyclic"
    } else if task.single.is_some() {
        "single"
    } else {
        "event"
    };

    let interval_ms = task
        .interval
        .as_ref()
        .map(|d| d.interval.whole_milliseconds() as f64);

    TaskMeta {
        name: task.name.to_string(),
        priority: task.priority,
        kind: kind.to_string(),
        interval_ms,
    }
}

fn program_meta_from_config(prog: &ProgramConfiguration) -> ProgramMeta {
    ProgramMeta {
        name: prog.name.to_string(),
        task: prog.task_name.as_ref().map(|t| t.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> Mutex<ContainerCache> {
        Mutex::new(ContainerCache::new(64, 64 * 1024 * 1024))
    }

    fn valid_program_source() -> Vec<SourceInput> {
        vec![SourceInput {
            name: "main.st".into(),
            content: r#"
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#
            .into(),
        }]
    }

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({ "dialect": "iec61131-3-ed2" })
    }

    #[test]
    fn build_response_when_valid_program_then_ok_true() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), false, &cache);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    }

    #[test]
    fn build_response_when_syntax_error_then_ok_false() {
        let cache = make_cache();
        let sources = vec![SourceInput {
            name: "bad.st".into(),
            content: "PROGRAM END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), false, &cache);
        assert!(!resp.ok);
        assert!(resp.container_id.is_none());
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false() {
        let cache = make_cache();
        let sources = vec![SourceInput {
            name: "bad.st".into(),
            content: r#"
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := undeclared_var;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#
            .into(),
        }];
        let resp = build_response(&sources, &ed2_options(), false, &cache);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_valid_then_container_id_present() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), false, &cache);
        assert!(resp.ok);
        assert!(resp.container_id.is_some());
        assert!(resp.container_id.unwrap().starts_with("c_"));
    }

    #[test]
    fn build_response_when_valid_then_tasks_populated() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), false, &cache);
        assert!(resp.ok);
        assert!(!resp.tasks.is_empty());
        assert_eq!(resp.tasks[0].name, "plc_task");
        assert_eq!(resp.tasks[0].kind, "cyclic");
        assert_eq!(resp.tasks[0].priority, 1);
        assert!(resp.tasks[0].interval_ms.is_some());
    }

    #[test]
    fn build_response_when_valid_then_programs_populated() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), false, &cache);
        assert!(resp.ok);
        assert!(!resp.programs.is_empty());
        assert_eq!(resp.programs[0].name, "program1");
        assert_eq!(resp.programs[0].task.as_deref(), Some("plc_task"));
    }

    #[test]
    fn build_response_when_include_bytes_true_then_base64_present() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), true, &cache);
        assert!(resp.ok);
        assert!(resp.container_base64.is_some());
        // Verify it's valid base64
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(resp.container_base64.as_ref().unwrap());
        assert!(bytes.is_ok());
    }

    #[test]
    fn build_response_when_include_bytes_false_then_base64_null() {
        let cache = make_cache();
        let resp = build_response(&valid_program_source(), &ed2_options(), false, &cache);
        assert!(resp.ok);
        assert!(resp.container_base64.is_none());
    }

    #[test]
    fn build_response_when_no_configuration_then_default_task() {
        let cache = make_cache();
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: r#"
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := 1;
END_PROGRAM
"#
            .into(),
        }];
        let resp = build_response(&sources, &ed2_options(), false, &cache);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(!resp.tasks.is_empty());
        assert_eq!(resp.tasks[0].name, "Main");
    }

    #[test]
    fn build_response_when_invalid_sources_then_error_diagnostic() {
        let cache = make_cache();
        let sources = vec![SourceInput {
            name: "".into(),
            content: "PROGRAM Main END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options(), false, &cache);
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_invalid_options_then_error_diagnostic() {
        let cache = make_cache();
        let resp = build_response(
            &valid_program_source(),
            &serde_json::json!({ "dialect": "nonexistent" }),
            false,
            &cache,
        );
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }
}
