//! Browser-based playground for IronPLC.
//!
//! Exposes functions to JavaScript:
//! - [`compile`] - Parse IEC 61131-3 source and produce bytecode
//! - [`run`] - Execute pre-compiled bytecode (.iplc)
//! - [`run_source`] - Compile and execute in one step
//! - [`load_program`] - Compile source and create a stepping session
//! - [`step`] - Execute N scans within a stepping session
//! - [`reset_session`] - Clear the stepping session

use std::cell::RefCell;
use std::io::Cursor;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ironplc_container::debug_format::VariableRenderer;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, LineColumn};
use ironplc_parser::options::{CompilerOptions, Dialect, FeatureDescriptor};
use ironplc_project::MemoryBackedProject;
use ironplc_sources::{parse_source, FileType};
use ironplc_vm::{Slot, VariableView, Vm, VmBuffers};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Persistent state for step-through execution.
///
/// Stores compiled bytecode and variable buffer so variables persist
/// across calls to [`step`]. The VM is re-created each step because
/// `VmRunning` borrows all buffers and cannot be stored across WASM calls.
struct VmSession {
    container_bytes: Vec<u8>,
    var_buf: Vec<Slot>,
    data_region: Vec<u8>,
    scan_count: u64,
    cycle_time_us: u64,
    faulted: bool,
}

thread_local! {
    static SESSION: RefCell<Option<VmSession>> = const { RefCell::new(None) };
}

/// Resolve a playground dialect string to a [`Dialect`].
///
/// Accepts the canonical [`Dialect::cli_name`] values (`"iec61131-3-ed2"`,
/// `"iec61131-3-ed3"`, `"rusty"`, `"codesys"`).
///
/// The empty string (and any unrecognized value) resolves to the RuSTy
/// dialect, which enables the broadest set of extensions. This keeps the many existing
/// documentation embeds that omit a dialect working, since they rely on the
/// lenient default to explore non-standard features without toggling flags.
fn dialect_from(dialect: &str) -> Dialect {
    if dialect.is_empty() {
        return Dialect::Rusty;
    }
    dialect.parse().unwrap_or(Dialect::Rusty)
}

/// Build [`CompilerOptions`] from a dialect string and an optional list of
/// `--allow-*` feature flags layered on top.
///
/// The dialect string is resolved by [`dialect_from`]; see that function for
/// the accepted values and the lenient default.
///
/// `allows` is a comma-separated list of feature short names — the part
/// after `--allow-` in the CLI flag, e.g. `"sizeof,c-style-comments"`.
/// Unknown names are ignored.
fn compiler_options_from(dialect: &str, allows: &str) -> CompilerOptions {
    let mut options = CompilerOptions::from_dialect(dialect_from(dialect));
    for name in allows.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let cli_flag = format!("--allow-{name}");
        if let Some(fd) = CompilerOptions::FEATURE_DESCRIPTORS
            .iter()
            .find(|fd: &&FeatureDescriptor| fd.cli_flag == cli_flag)
        {
            options.set_flag_by_key(fd.option_key, true);
        }
    }
    options
}

/// Install a panic hook that logs to `console.error` with a full stack trace.
///
/// Called once from JavaScript before using any other exports.
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Return the crate version so the playground can include it in problem-code URLs.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// A selectable dialect for the playground dialect picker.
#[derive(Serialize, Deserialize)]
struct DialectOption {
    /// Canonical dialect name (matches [`Dialect::cli_name`]), passed back to
    /// the compile/run entry points.
    value: String,
    /// Human-readable label for display in the picker.
    label: String,
    /// Whether this is the default selection.
    is_default: bool,
}

/// Return the selectable dialects as a JSON array so the UI builds its dialect
/// picker from the compiler's own [`Dialect`] list. This keeps the picker from
/// drifting out of sync with the dialects the compiler actually supports.
#[wasm_bindgen]
pub fn dialects() -> String {
    let default = Dialect::default();
    let options: Vec<DialectOption> = Dialect::ALL
        .iter()
        .map(|d| DialectOption {
            value: d.cli_name().to_string(),
            label: d.display_name().to_string(),
            is_default: *d == default,
        })
        .collect();
    serde_json::to_string(&options).unwrap_or_else(|_| "[]".to_string())
}

/// Result of a compilation attempt.
#[derive(Serialize, Deserialize)]
struct CompileResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bytecode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
}

/// A single diagnostic (error or warning) from compilation.
///
/// Line and column fields are 1-based for display, computed from the
/// diagnostic's byte offsets using the same helper the LSP server uses.
#[derive(Debug, Serialize, Deserialize)]
struct DiagnosticInfo {
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    label: String,
    /// Guidance on how to resolve the problem (e.g. "use `(* *)` comments").
    /// Empty when the diagnostic carries no help notes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    help: Vec<String>,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    /// The compiler source file that produced this diagnostic (e.g. the
    /// `file!()` recorded by `Diagnostic::todo`). Present for the P9999/P9998
    /// families; empty otherwise. This is the compiler's own location, never
    /// the user's program, so it is safe to report automatically.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    compiler_file: String,
    /// The compiler source line paired with `compiler_file`. Zero when absent.
    #[serde(default, skip_serializing_if = "is_zero")]
    compiler_line: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Build a [`DiagnosticInfo`] from a compiler diagnostic, computing 1-based
/// line/column from the supplied source text.
fn diagnostic_info(diag: &Diagnostic, source: &str) -> DiagnosticInfo {
    let start = LineColumn::from_offset(source, diag.primary.location.start);
    let end = LineColumn::from_offset(source, diag.primary.location.end);
    DiagnosticInfo {
        code: diag.code.clone(),
        message: diag.description(),
        label: diag.primary.message.clone(),
        help: diag.help().to_vec(),
        start_line: start.line + 1,
        start_column: start.column + 1,
        end_line: end.line + 1,
        end_column: end.column + 1,
        compiler_file: diag.source_file.clone().unwrap_or_default(),
        compiler_line: diag.source_line.unwrap_or(0),
    }
}

/// A structured runtime error surfaced across the WASM boundary.
///
/// Carries a human-readable `message` and, for VM traps, the trap's stable
/// v-code (e.g. `"V4001"`). Kept as a single object — rather than sibling
/// `error`/`error_code` fields — so the front end can treat it uniformly with a
/// compiler diagnostic (which likewise has a message and a code) and render
/// both through one path. This shape is also the natural fit as the playground
/// moves toward JSON-RPC.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RunError {
    /// Human-readable message including task and instance context for traps.
    message: String,
    /// The error's stable code — a VM trap's v-code (e.g. `"V4001"`) or, for a
    /// host/embedding-layer illegal state, `"P9998"` (the internal-error code).
    /// Every error site populates this, so it is only absent on values
    /// deserialized from an older payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    /// For a `P9998` internal error, the WASM host `file`/`line` where the
    /// illegal state was detected — the same `compiler_file`/`compiler_line`
    /// contract a P9xxx [`DiagnosticInfo`] carries, so the front end ranks host
    /// bugs by location just like compiler ones. Empty for VM traps.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    compiler_file: String,
    /// The host source line paired with `compiler_file`. Zero when absent.
    #[serde(default, skip_serializing_if = "is_zero")]
    compiler_line: u32,
}

/// Builds a `P9998` internal-error [`RunError`] stamped with the WASM host
/// `file`/`line` of the call site.
///
/// Host/embedding-layer illegal states — frontend↔WASM contract violations and
/// failures that should never occur in normal use — are bugs, not distinct user
/// conditions. Rather than mint a bespoke code and doc page per site, they all
/// share the existing internal-error code and are told apart by the recorded
/// location, mirroring how the compiler records `file#Lline` for its own P9998
/// diagnostics (see [`Diagnostic::internal_error`]).
#[track_caller]
fn internal_run_error(message: String) -> RunError {
    let loc = std::panic::Location::caller();
    // Derive the stable code from the shared diagnostic constructor rather than
    // hard-coding "P9998", so it tracks the compiler's internal-error code.
    let code = Diagnostic::internal_error().code;
    RunError {
        message,
        code: Some(code),
        compiler_file: loc.file().to_string(),
        compiler_line: loc.line(),
    }
}

/// Serializes a fallback [`RunError`] for the serde-to-JSON error path. The full
/// result already failed to serialize, but this tiny error object does not; the
/// static literal is a last-ditch guard should even that fail.
fn fallback_error_json(err: &RunError) -> String {
    serde_json::to_string(err)
        .unwrap_or_else(|_| r#"{"message":"Serialization error","code":"P9998"}"#.to_string())
}

/// The [`DiagnosticInfo`] counterpart of [`internal_run_error`], for host
/// illegal states on the compile path (which report through `diagnostics`
/// rather than a `RunError`). Same `P9998` + `file#Lline` contract.
#[track_caller]
fn internal_diagnostic(message: String) -> DiagnosticInfo {
    let loc = std::panic::Location::caller();
    let code = Diagnostic::internal_error().code;
    DiagnosticInfo {
        code,
        message,
        label: String::new(),
        help: Vec::new(),
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 1,
        compiler_file: loc.file().to_string(),
        compiler_line: loc.line(),
    }
}

/// Result of executing bytecode.
#[derive(Serialize, Deserialize)]
struct RunResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    scans_completed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<RunError>,
}

/// A variable value read from the VM after execution.
#[derive(Serialize, Deserialize)]
struct VariableInfo {
    index: u16,
    value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    type_name: String,
    /// `false` when `value` is a placeholder shown because the actual value
    /// could not be read (e.g., STRING data-region offset out of bounds, or
    /// WSTRING which is not yet implemented).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    valid: bool,
}

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// Result of compile-and-run (combines both).
#[derive(Serialize, Deserialize)]
struct RunSourceResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    scans_completed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<RunError>,
}

/// Result of a step-through operation (load or step).
#[derive(Serialize, Deserialize)]
struct StepResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    total_scans: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<RunError>,
}

/// Parse IEC 61131-3 source code and produce bytecode.
///
/// Returns a JSON string with shape:
/// ```json
/// { "ok": true, "bytecode": "<base64>" }
/// ```
/// or on error:
/// ```json
/// {
///   "ok": false,
///   "diagnostics": [{
///     "code": "...", "message": "...",
///     "start_line": L, "start_column": C,
///     "end_line": L, "end_column": C
///   }]
/// }
/// ```
/// Line and column are 1-based.
#[wasm_bindgen]
pub fn compile(source: &str, dialect: &str, allows: &str, libraries: &str) -> String {
    let result = compile_inner(source, dialect, allows, libraries);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        // Even the full result failed to serialize; the tiny internal-error
        // diagnostic still serializes, so build the fallback payload from it.
        let diag = serde_json::to_string(&internal_diagnostic(format!("Serialization error: {e}")))
            .unwrap_or_else(|_| r#"{"code":"P9998","message":"Serialization error"}"#.to_string());
        format!(r#"{{"ok":false,"diagnostics":[{diag}]}}"#)
    })
}

/// Parse the activated compatibility libraries from their served plain-text
/// sources (`REQ-CL-playground-001`).
///
/// `libraries` is a JSON array of ST source strings — the plain-text library
/// files the browser fetched from the app's served assets. Each is parsed into
/// a [`Library`] to be injected ahead of user source in analysis, so its
/// symbols (e.g. `Tc2_System`'s `PI`) resolve under their exact vendor names.
/// An empty or blank string activates no library.
fn parse_activated_libraries(
    libraries: &str,
    options: &CompilerOptions,
) -> Result<Vec<Library>, CompileResult> {
    if libraries.trim().is_empty() {
        return Ok(Vec::new());
    }

    let sources: Vec<String> = serde_json::from_str(libraries).map_err(|e| CompileResult {
        ok: false,
        bytecode: None,
        diagnostics: vec![internal_diagnostic(format!(
            "Failed to parse library sources: {e}"
        ))],
    })?;

    let mut parsed = Vec::with_capacity(sources.len());
    for source in &sources {
        let file_type = FileType::from_content(source);
        match parse_source(file_type, source, &FileId::default(), options) {
            Ok(lib) => parsed.push(lib),
            Err(diag) => {
                return Err(CompileResult {
                    ok: false,
                    bytecode: None,
                    diagnostics: vec![diagnostic_info(&diag, source)],
                });
            }
        }
    }
    Ok(parsed)
}

fn compile_inner(source: &str, dialect: &str, allows: &str, libraries: &str) -> CompileResult {
    let file_type = FileType::from_content(source);
    let options = compiler_options_from(dialect, allows);

    // Activated compatibility libraries, loaded from their served plain-text
    // files. They are injected ahead of user source (base stdlib -> library ->
    // user), so a user declaration shadows a library declaration of the same
    // name (`REQ-CL-playground-001`).
    let compat_libraries = match parse_activated_libraries(libraries, &options) {
        Ok(libs) => libs,
        Err(result) => return result,
    };

    // The pipeline (parse -> analysis -> codegen) is owned by
    // `ironplc-project`, which compiles for wasm32 -- the playground supplies
    // the editor buffer and the fetched library text, and does nothing with
    // the filesystem-backed half of that crate. The editor buffer has no
    // filename, so its type comes from the content rather than an extension.
    let mut project = MemoryBackedProject::new(options);
    project.set_preparsed_libraries(compat_libraries);
    project.add_source_with_file_type(FileId::default(), source.to_owned(), file_type);

    let output = ironplc_project::compile(
        &mut project,
        &options,
        &ironplc_codegen::EmptyLookup,
        vec![],
    );

    let Some(container) = output.container else {
        return CompileResult {
            ok: false,
            bytecode: None,
            diagnostics: output
                .diagnostics
                .iter()
                .map(|d| diagnostic_info(d, source))
                .collect(),
        };
    };

    let mut buf = Vec::new();
    if let Err(e) = container.write_to(&mut buf) {
        return CompileResult {
            ok: false,
            bytecode: None,
            diagnostics: vec![internal_diagnostic(format!(
                "Failed to serialize bytecode: {e}"
            ))],
        };
    }

    CompileResult {
        ok: true,
        bytecode: Some(BASE64.encode(&buf)),
        diagnostics: vec![],
    }
}

/// Execute pre-compiled bytecode (.iplc format).
///
/// `bytecode_base64` is the base64-encoded .iplc file content.
/// `scans` is the number of scan cycles to run.
///
/// Returns a JSON string with variable values after execution.
#[wasm_bindgen]
pub fn run(bytecode_base64: &str, scans: u32) -> String {
    let result = run_inner(bytecode_base64, scans);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        let error = fallback_error_json(&internal_run_error(format!("Serialization error: {e}")));
        format!(r#"{{"ok":false,"variables":[],"scans_completed":0,"error":{error}}}"#)
    })
}

fn run_inner(bytecode_base64: &str, scans: u32) -> RunResult {
    let bytes = match BASE64.decode(bytecode_base64) {
        Ok(b) => b,
        Err(e) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(internal_run_error(format!("Invalid base64: {e}"))),
            };
        }
    };

    run_bytes(&bytes, scans)
}

fn run_bytes(bytes: &[u8], scans: u32) -> RunResult {
    let container = match Container::read_from(&mut Cursor::new(bytes)) {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(internal_run_error(format!(
                    "Invalid bytecode container: {e}"
                ))),
            };
        }
    };

    let mut bufs = VmBuffers::from_container(&container);

    let mut running = match Vm::new().load(&container, &mut bufs).start() {
        Ok(vm) => vm,
        Err(ctx) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(RunError {
                    message: format!(
                        "VM trap during init: {} (task {}, instance {})",
                        ctx.trap, ctx.task_id, ctx.instance_id
                    ),
                    code: Some(ctx.trap.v_code().to_string()),
                    ..Default::default()
                }),
            };
        }
    };

    let renderer = VariableRenderer::new(&container);

    for round in 0..scans {
        let uptime_us = (round as u64) * 1000;
        if let Err(ctx) = running.run_round(uptime_us) {
            let faulted = running.fault(ctx);
            let variables = read_all_variables(&faulted, &renderer);
            return RunResult {
                ok: false,
                variables,
                scans_completed: round as u64,
                error: Some(RunError {
                    message: format!(
                        "VM trap: {} (task {}, instance {})",
                        faulted.trap(),
                        faulted.task_id(),
                        faulted.instance_id()
                    ),
                    code: Some(faulted.trap().v_code().to_string()),
                    ..Default::default()
                }),
            };
        }
    }

    let variables = read_all_variables(&running, &renderer);
    let scans_completed = running.scan_count();
    running.stop();

    RunResult {
        ok: true,
        variables,
        scans_completed,
        error: None,
    }
}

/// Compile IEC 61131-3 source and execute in one step.
///
/// Returns a JSON string with both compilation diagnostics and execution results.
#[wasm_bindgen]
pub fn run_source(
    source: &str,
    scans: u32,
    dialect: &str,
    allows: &str,
    libraries: &str,
) -> String {
    let result = run_source_inner(source, scans, dialect, allows, libraries);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        let error = fallback_error_json(&internal_run_error(format!("Serialization error: {e}")));
        format!(
            r#"{{"ok":false,"diagnostics":[],"variables":[],"scans_completed":0,"error":{error}}}"#
        )
    })
}

fn run_source_inner(
    source: &str,
    scans: u32,
    dialect: &str,
    allows: &str,
    libraries: &str,
) -> RunSourceResult {
    let compile_result = compile_inner(source, dialect, allows, libraries);
    if !compile_result.ok {
        return RunSourceResult {
            ok: false,
            diagnostics: compile_result.diagnostics,
            variables: vec![],
            scans_completed: 0,
            error: None,
        };
    }

    let bytecode_b64 = compile_result.bytecode.unwrap();
    let bytes = BASE64.decode(&bytecode_b64).unwrap();
    let run_result = run_bytes(&bytes, scans);

    RunSourceResult {
        ok: run_result.ok,
        diagnostics: vec![],
        variables: run_result.variables,
        scans_completed: run_result.scans_completed,
        error: run_result.error,
    }
}

/// Snapshots every variable slot with its debug name, type and formatted value.
///
/// Takes the VM as a [`VariableView`] so a running and a faulted VM share one
/// body: the caller's lifecycle state does not change how a value is read.
///
/// Rendering goes through [`VariableRenderer`], the one place that formats a
/// variable for display (`specs/design/variable-value-rendering.md`), so the
/// playground agrees with `--dump-vars`, the debugger and the VS Code run
/// panel — the playground previously carried its own near-copy of that logic,
/// and the two disagreed on STRING, on the date types and on TIME's unit.
fn read_all_variables(vm: &dyn VariableView, renderer: &VariableRenderer) -> Vec<VariableInfo> {
    let data_region = vm.data_region();
    (0..vm.num_variables())
        .filter_map(|i| {
            vm.read_variable_raw(ironplc_container::VarIndex::new(i))
                .ok()
                .map(|raw| {
                    let rendered = renderer.render(i, raw, data_region);
                    VariableInfo {
                        index: i,
                        value: rendered.text,
                        name: renderer
                            .var(i)
                            .map(|info| info.name.clone())
                            .unwrap_or_default(),
                        type_name: renderer
                            .var(i)
                            .map(|info| info.type_name.clone())
                            .unwrap_or_default(),
                        valid: rendered.valid,
                    }
                })
        })
        .collect()
}

/// Compile IEC 61131-3 source and create a stepping session.
///
/// The session stores compiled bytecode and a variable buffer that persists
/// across calls to [`step`]. Returns a JSON `StepResult` with `total_scans: 0`.
#[wasm_bindgen]
pub fn load_program(
    source: &str,
    cycle_time_us: u32,
    dialect: &str,
    allows: &str,
    libraries: &str,
) -> String {
    let result = load_program_inner(source, cycle_time_us, dialect, allows, libraries);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        let error = fallback_error_json(&internal_run_error(format!("Serialization error: {e}")));
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"total_scans":0,"error":{error}}}"#)
    })
}

fn load_program_inner(
    source: &str,
    cycle_time_us: u32,
    dialect: &str,
    allows: &str,
    libraries: &str,
) -> StepResult {
    let compile_result = compile_inner(source, dialect, allows, libraries);
    if !compile_result.ok {
        return StepResult {
            ok: false,
            diagnostics: compile_result.diagnostics,
            variables: vec![],
            total_scans: 0,
            error: None,
        };
    }

    let bytecode_b64 = compile_result.bytecode.unwrap();
    let container_bytes = BASE64.decode(&bytecode_b64).unwrap();

    let container = match Container::read_from(&mut Cursor::new(&container_bytes)) {
        Ok(c) => c,
        Err(e) => {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some(internal_run_error(format!("Failed to load bytecode: {e}"))),
            };
        }
    };

    // Run the init function once to apply initial values to the variable buffer.
    // Subsequent calls to step() will use resume() to skip re-initialization.
    let mut bufs = VmBuffers::from_container(&container);

    match Vm::new().load(&container, &mut bufs).start() {
        Ok(running) => {
            running.stop();
        }
        Err(ctx) => {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some(RunError {
                    message: format!("VM init trap: {}", ctx.trap),
                    code: Some(ctx.trap.v_code().to_string()),
                    ..Default::default()
                }),
            };
        }
    }

    SESSION.with(|cell| {
        *cell.borrow_mut() = Some(VmSession {
            container_bytes,
            var_buf: bufs.vars,
            data_region: bufs.data_region,
            scan_count: 0,
            cycle_time_us: cycle_time_us as u64,
            faulted: false,
        });
    });

    StepResult {
        ok: true,
        diagnostics: vec![],
        variables: vec![],
        total_scans: 0,
        error: None,
    }
}

/// Execute N scan cycles within the current stepping session.
///
/// Variable values persist between calls. Returns a JSON `StepResult`
/// with accumulated `total_scans`.
#[wasm_bindgen]
pub fn step(scans: u32) -> String {
    let result = step_inner(scans);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        let error = fallback_error_json(&internal_run_error(format!("Serialization error: {e}")));
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"total_scans":0,"error":{error}}}"#)
    })
}

fn step_inner(scans: u32) -> StepResult {
    SESSION.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let session = match borrow.as_mut() {
            Some(s) => s,
            None => {
                return StepResult {
                    ok: false,
                    diagnostics: vec![],
                    variables: vec![],
                    total_scans: 0,
                    error: Some(internal_run_error(
                        "No program loaded. Call load_program first.".to_string(),
                    )),
                };
            }
        };

        if session.faulted {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some(internal_run_error(
                    "Session is faulted. Call reset_session to start over.".to_string(),
                )),
            };
        }

        let container = match Container::read_from(&mut Cursor::new(&session.container_bytes)) {
            Ok(c) => c,
            Err(e) => {
                return StepResult {
                    ok: false,
                    diagnostics: vec![],
                    variables: vec![],
                    total_scans: 0,
                    error: Some(internal_run_error(format!("Failed to load bytecode: {e}"))),
                };
            }
        };

        let (variables, total_scans, error) = run_vm_step(
            &container,
            &mut session.var_buf,
            &mut session.data_region,
            session.scan_count,
            scans,
            session.cycle_time_us,
        );

        session.scan_count = total_scans;
        if error.is_some() {
            session.faulted = true;
        }

        StepResult {
            ok: error.is_none(),
            diagnostics: vec![],
            variables,
            total_scans,
            error,
        }
    })
}

/// Run an ephemeral VM for N scans using the given container and variable buffer.
///
/// Uses [`VmReady::resume`] to skip re-initialization so that variable values
/// (including initial values) persist across calls. The VM's internal scan
/// counter is the source of truth for total cycles executed.
///
/// Returns `(variables, total_scan_count, error)`, where `error` carries the
/// message and the trap's v-code when execution stopped on a trap.
fn run_vm_step(
    container: &Container,
    var_buf: &mut Vec<Slot>,
    data_region: &mut Vec<u8>,
    base_scan_count: u64,
    scans: u32,
    cycle_time_us: u64,
) -> (Vec<VariableInfo>, u64, Option<RunError>) {
    let mut bufs = VmBuffers::from_container(container);
    // Swap the session's persistent buffers into VmBuffers so the VM
    // operates on them directly, avoiding a copy.
    std::mem::swap(&mut bufs.vars, var_buf);
    std::mem::swap(&mut bufs.data_region, data_region);

    let result = run_vm_scans(container, &mut bufs, base_scan_count, scans, cycle_time_us);

    // Swap the (now-updated) persistent buffers back to the session.
    std::mem::swap(&mut bufs.vars, var_buf);
    std::mem::swap(&mut bufs.data_region, data_region);

    result
}

/// Runs scan cycles on an already-prepared [`VmBuffers`], returning variable
/// snapshots and the total scan count.
fn run_vm_scans(
    container: &Container,
    bufs: &mut VmBuffers,
    base_scan_count: u64,
    scans: u32,
    cycle_time_us: u64,
) -> (Vec<VariableInfo>, u64, Option<RunError>) {
    let mut running = Vm::new().load(container, bufs).resume(base_scan_count);

    for _ in 0..scans {
        let uptime_us = running.scan_count() * cycle_time_us;
        if let Err(ctx) = running.run_round(uptime_us) {
            let total_scans = running.scan_count();
            let faulted = running.fault(ctx);
            let variables = read_all_variables(&faulted, &VariableRenderer::new(container));
            let error = RunError {
                message: format!(
                    "VM trap: {} (task {}, instance {})",
                    faulted.trap(),
                    faulted.task_id(),
                    faulted.instance_id()
                ),
                code: Some(faulted.trap().v_code().to_string()),
                ..Default::default()
            };
            return (variables, total_scans, Some(error));
        }
    }

    let variables = read_all_variables(&running, &VariableRenderer::new(container));
    let total_scans = running.scan_count();
    running.stop();
    (variables, total_scans, None)
}

/// Clear the stepping session.
///
/// Returns `{"ok":true}`.
#[wasm_bindgen]
pub fn reset_session() -> String {
    SESSION.with(|cell| {
        *cell.borrow_mut() = None;
    });
    r#"{"ok":true}"#.to_string()
}

// Spec conformance testing infrastructure (test-only).
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_when_valid_source_then_returns_bytecode() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(result.ok);
        assert!(result.bytecode.is_some());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn compile_when_syntax_error_then_returns_diagnostics() {
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(result.bytecode.is_none());
        assert!(!result.diagnostics.is_empty());
        assert!(!result.diagnostics[0].label.is_empty());
    }

    #[test]
    fn compile_when_error_on_later_line_then_diagnostic_has_line_and_column() {
        // Line numbers are 1-based; the error is after the first line.
        let source = "PROGRAM main\nVAR\nEND_VAR\nINVALID\nEND_PROGRAM";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
        let diag = &result.diagnostics[0];
        assert!(
            diag.start_line >= 2,
            "expected error on line 2 or later, got {}",
            diag.start_line
        );
        assert!(diag.start_column >= 1, "expected 1-based column");
        assert!(diag.end_line >= diag.start_line);
    }

    #[test]
    fn run_when_valid_bytecode_then_returns_variables() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let compile_result: CompileResult =
            serde_json::from_str(&compile(source, "", "", "")).unwrap();
        let bytecode = compile_result.bytecode.unwrap();

        let result: RunResult = serde_json::from_str(&run(&bytecode, 1)).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 1);
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[2].value, "42"); // indices 0-1 are system globals
    }

    #[test]
    fn run_when_program_traps_then_error_has_v_code() {
        // Divide-by-zero traps with v-code V4001. The structured code must
        // survive the WASM boundary in the error object's `code` member, not be
        // flattened into the message.
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := 0;
  x := 1 / y;
END_PROGRAM
";
        let compile_result: CompileResult =
            serde_json::from_str(&compile(source, "", "", "")).unwrap();
        let bytecode = compile_result.bytecode.unwrap();

        let json = run(&bytecode, 1);
        let result: RunResult = serde_json::from_str(&json).unwrap();
        assert!(!result.ok);
        let error = result.error.unwrap();
        assert_eq!(error.code.as_deref(), Some("V4001"));
        // The JSON payload carries the code as a member of the error object.
        assert!(json.contains("\"code\":\"V4001\""));
    }

    #[test]
    fn run_when_invalid_base64_then_error_is_internal_with_location() {
        let json = run("not-valid-base64!!!", 1);
        let result: RunResult = serde_json::from_str(&json).unwrap();
        assert!(!result.ok);
        let error = result.error.unwrap();
        // Host illegal states share the internal-error code and are told apart
        // by the recorded call-site location, not by a bespoke per-error code.
        assert_eq!(error.code.as_deref(), Some("P9998"));
        assert!(error.compiler_file.ends_with("lib.rs"));
        assert!(error.compiler_line > 0);
        assert!(json.contains("\"code\":\"P9998\""));
    }

    #[test]
    fn run_when_invalid_container_then_error_is_internal() {
        let bytes = BASE64.encode(b"not a container");
        let result: RunResult = serde_json::from_str(&run(&bytes, 1)).unwrap();
        assert!(!result.ok);
        let error = result.error.unwrap();
        assert_eq!(error.code.as_deref(), Some("P9998"));
        assert!(error.compiler_line > 0);
    }

    #[test]
    fn run_source_when_steel_thread_then_returns_values() {
        let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(result.ok);
        assert!(result.diagnostics.is_empty());
        assert!(result.error.is_none());
        assert_eq!(result.scans_completed, 1);
        assert!(result.variables.len() >= 2);
        assert_eq!(result.variables[2].value, "10"); // indices 0-1 are system globals
        assert_eq!(result.variables[3].value, "42"); // indices 0-1 are system globals
    }

    #[test]
    fn run_source_when_syntax_error_then_returns_diagnostics() {
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.scans_completed, 0);
    }

    #[test]
    fn run_source_when_multiple_scans_then_correct_count() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 5, "", "", "")).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 5);
        assert_eq!(result.variables[2].value, "99"); // indices 0-1 are system globals
    }

    #[test]
    fn compile_when_valid_source_then_bytecode_is_valid_base64() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        let bytecode = result.bytecode.unwrap();
        let decoded = BASE64.decode(&bytecode);
        assert!(decoded.is_ok());
        assert!(!decoded.unwrap().is_empty());
    }

    #[test]
    fn run_when_zero_scans_then_returns_zero_variables() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let compile_result: CompileResult =
            serde_json::from_str(&compile(source, "", "", "")).unwrap();
        let bytecode = compile_result.bytecode.unwrap();

        let result: RunResult = serde_json::from_str(&run(&bytecode, 0)).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 0);
    }

    // --- Stepping tests ---

    #[test]
    fn load_program_when_valid_source_then_creates_session() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let result: StepResult =
            serde_json::from_str(&load_program(source, 100_000, "", "", "")).unwrap();
        assert!(result.ok);
        assert_eq!(result.total_scans, 0);
        assert!(result.diagnostics.is_empty());
        assert!(result.error.is_none());
    }

    #[test]
    fn load_program_when_syntax_error_then_returns_diagnostics() {
        reset_session();
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: StepResult =
            serde_json::from_str(&load_program(source, 100_000, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn step_when_no_session_then_returns_error() {
        reset_session();
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!result.ok);
        let error = result.error.unwrap();
        assert!(error.message.contains("No program loaded"));
        assert_eq!(error.code.as_deref(), Some("P9998"));
        assert!(error.compiler_line > 0);
    }

    #[test]
    fn step_when_session_loaded_then_returns_variables() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(result.ok);
        assert_eq!(result.total_scans, 1);
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[2].value, "42"); // indices 0-1 are system globals
    }

    #[test]
    fn step_when_called_twice_then_variables_persist() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");

        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r1.ok);
        assert_eq!(r1.variables[2].value, "1"); // indices 0-1 are system globals

        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r2.ok);
        assert_eq!(r2.variables[2].value, "2"); // indices 0-1 are system globals
    }

    #[test]
    fn step_when_called_twice_then_total_scans_accumulate() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");

        let r1: StepResult = serde_json::from_str(&step(3)).unwrap();
        assert_eq!(r1.total_scans, 3);

        let r2: StepResult = serde_json::from_str(&step(2)).unwrap();
        assert_eq!(r2.total_scans, 5);
    }

    #[test]
    fn step_when_session_faulted_then_returns_error() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := 0;
  x := 1 / y;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");

        // First step should fault (divide by zero)
        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!r1.ok);
        assert!(r1.error.as_ref().unwrap().message.contains("VM trap"));

        // Subsequent step should report faulted session
        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!r2.ok);
        let error = r2.error.unwrap();
        assert!(error.message.contains("faulted"));
        assert_eq!(error.code.as_deref(), Some("P9998"));
    }

    #[test]
    fn reset_session_when_session_exists_then_clears_it() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");
        step(1);

        reset_session();

        // After reset, step should fail with no session
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!result.ok);
        assert!(result.error.unwrap().message.contains("No program loaded"));
    }

    #[test]
    fn compile_when_valid_xml_then_returns_bytecode() {
        let source = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous>
      <pou name="main" pouType="program">
        <interface>
          <localVars>
            <variable name="bSwitch">
              <type><BOOL/></type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">
bSwitch := TRUE;
            </xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_twincat_xml_then_returns_bytecode() {
        let source = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="main" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM main
VAR
    x : DINT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[x := 42;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_malformed_xml_then_returns_diagnostics() {
        let source = "<?xml version=\"1.0\"?><project><invalid";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn load_program_when_called_twice_then_replaces_session() {
        reset_session();
        let source_a = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
";
        load_program(source_a, 100_000, "", "", "");
        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert_eq!(r1.variables[2].value, "10"); // indices 0-1 are system globals

        let source_b = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 20;
END_PROGRAM
";
        load_program(source_b, 100_000, "", "", "");
        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert_eq!(r2.variables[2].value, "20"); // indices 0-1 are system globals
        assert_eq!(r2.total_scans, 1);
    }

    #[test]
    fn step_when_variable_has_initial_value_then_persists_across_steps() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    exponentially : INT := 1;
  END_VAR
  exponentially := exponentially * 2;
END_PROGRAM
";
        load_program(source, 100_000, "", "", "");

        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r1.ok);
        assert_eq!(r1.total_scans, 1);
        assert_eq!(r1.variables[2].value, "2"); // 1 * 2; indices 0-1 are system globals

        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r2.ok);
        assert_eq!(r2.total_scans, 2);
        assert_eq!(r2.variables[2].value, "4"); // 2 * 2; indices 0-1 are system globals

        let r3: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r3.ok);
        assert_eq!(r3.total_scans, 3);
        assert_eq!(r3.variables[2].value, "8"); // 4 * 2; indices 0-1 are system globals
    }

    #[test]
    fn run_source_when_bcd_to_int_with_literal_then_returns_value() {
        let source = "
PROGRAM main
  VAR
    int_val : USINT;
  END_VAR
  int_val := BCD_TO_INT(BYTE#16#42);
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(result.ok, "Expected ok but got error: {:?}", result.error);
        assert_eq!(result.variables[2].value, "42"); // indices 0-1 are system globals
    }

    #[test]
    fn run_source_when_int_to_bcd_with_literal_then_returns_value() {
        let source = "
PROGRAM main
  VAR
    bcd_val : BYTE;
  END_VAR
  bcd_val := INT_TO_BCD(USINT#42);
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(result.ok, "Expected ok but got error: {:?}", result.error);
        assert_eq!(result.variables[2].value, "16#42"); // indices 0-1 are system globals
    }

    #[test]
    fn compile_when_p9999_then_diagnostic_has_compiler_file_and_line() {
        // A direct hardware-address write is not supported by codegen and
        // produces P9999. The diagnostic must carry the compiler file/line so
        // the playground can report the location without the program source.
        let source = "
PROGRAM main
  VAR
    x : BOOL;
  END_VAR
  %QX0.0 := TRUE;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(!result.ok);
        let diag = result
            .diagnostics
            .iter()
            .find(|d| d.code == "P9999")
            .unwrap();
        assert!(
            diag.compiler_file.ends_with(".rs"),
            "expected a compiler .rs file, got {:?}",
            diag.compiler_file
        );
        assert!(diag.compiler_line > 0, "expected a non-zero compiler line");
    }

    // A semantic error failing the compile is owned by
    // `ironplc_project::compile::compile_when_semantic_error_then_no_container`;
    // that a failed compile becomes `ok: false` with diagnostics attached is
    // this binding's own contract, asserted by
    // `compile_when_syntax_error_then_returns_diagnostics` above.

    #[test]
    fn step_when_ton_then_q_transitions_to_true() {
        reset_session();
        // PT = T#5s = 5000 ms. With cycle_time_us = 100_000 (100ms per step),
        // Q should become TRUE after 50 steps (50 * 100ms = 5s).
        let source = "
PROGRAM main
  VAR
    myTimer : TON;
    start : BOOL := TRUE;
    done : BOOL;
    elapsed : TIME;
  END_VAR
  myTimer(IN := start, PT := T#5s, Q => done, ET => elapsed);
END_PROGRAM
";
        let load: StepResult =
            serde_json::from_str(&load_program(source, 100_000, "", "", "")).unwrap();
        assert!(
            load.ok,
            "load failed: error={:?}, diagnostics={:?}",
            load.error, load.diagnostics
        );

        // After 10 steps (1s elapsed), Q should still be FALSE
        let r1: StepResult = serde_json::from_str(&step(10)).unwrap();
        assert!(r1.ok, "step(10) failed: {:?}", r1.error);
        let done_var = r1.variables.iter().find(|v| v.name == "done").unwrap();
        assert_eq!(done_var.value, "FALSE");

        // After 50 total steps (5s elapsed), Q should be TRUE
        let r2: StepResult = serde_json::from_str(&step(41)).unwrap();
        assert!(r2.ok, "step(41) failed: {:?}", r2.error);
        let done_var = r2.variables.iter().find(|v| v.name == "done").unwrap();
        assert_eq!(done_var.value, "TRUE");

        // Verify TIME variable displays correct type name
        let elapsed_var = r2.variables.iter().find(|v| v.name == "elapsed").unwrap();
        assert_eq!(
            elapsed_var.type_name, "TIME",
            "TIME variable should display as TIME, not TIME_OF_DAY"
        );
    }

    #[test]
    fn step_when_tof_then_q_transitions_to_false() {
        reset_session();
        // PT = T#5s = 5000 ms. With cycle_time_us = 100_000 (100ms per step),
        // Q should become FALSE after 50 steps of IN=FALSE (50 * 100ms = 5s).
        let source = "
PROGRAM main
  VAR
    myTimer : TOF;
    run : BOOL := TRUE;
    active : BOOL;
    elapsed : TIME;
  END_VAR
  myTimer(IN := run, PT := T#5s, Q => active, ET => elapsed);
END_PROGRAM
";
        let load: StepResult =
            serde_json::from_str(&load_program(source, 100_000, "", "", "")).unwrap();
        assert!(
            load.ok,
            "load failed: error={:?}, diagnostics={:?}",
            load.error, load.diagnostics
        );

        // After 10 steps with IN=TRUE, Q should be TRUE
        let r1: StepResult = serde_json::from_str(&step(10)).unwrap();
        assert!(r1.ok, "step(10) failed: {:?}", r1.error);
        let active_var = r1.variables.iter().find(|v| v.name == "active").unwrap();
        assert_eq!(active_var.value, "TRUE");
    }

    #[test]
    fn run_source_when_string_assignment_then_value_displays() {
        let source = "
PROGRAM main
  VAR
    s : STRING := 'hello';
  END_VAR
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}, error: {:?}",
            result.diagnostics, result.error
        );
        let s = result.variables.iter().find(|v| v.name == "s").unwrap();
        assert_eq!(s.value, "'hello'");
        assert!(s.valid, "expected s.valid == true for a real STRING value");
    }

    /// The playground used to render durations as `T#1.5s` while the CLI dump
    /// rendered the same variable as `T#1500ms`. Both now come from the shared
    /// renderer, so this asserts the playground shows the shared form.
    #[test]
    fn run_source_when_duration_and_date_then_shared_rendering() {
        let source = "
PROGRAM main
  VAR
    t : TIME := T#1500ms;
    d : DATE := D#2024-01-15;
    clock : TIME_OF_DAY := TOD#14:30:00;
  END_VAR
END_PROGRAM
";
        let result: RunSourceResult =
            serde_json::from_str(&run_source(source, 1, "", "", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}, error: {:?}",
            result.diagnostics, result.error
        );
        let value = |name: &str| {
            result
                .variables
                .iter()
                .find(|v| v.name == name)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(value("t"), "T#1500ms");
        assert_eq!(value("d"), "D#2024-01-15");
        assert_eq!(value("clock"), "TOD#14:30:00");
    }

    // Whether a given dialect or `--allow-` flag accepts a construct is owned
    // by `parser/src/tests/dialect_flags.rs` (and re-verified behaviorally
    // across the whole flag set by mcp's feature_flag_conformance). What is
    // playground-owned is the string plumbing: that a dialect name and a
    // comma-separated allows list arriving from JS are resolved and actually
    // reach the compiler. Each pair below is kept as an off/on contrast for
    // exactly that reason — a single positive case would also pass if the
    // strings were ignored and the baseline were simply permissive.
    #[test]
    fn compile_when_dialect_2013_then_accepts_ltime() {
        let source = "
PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#100ms;
END_PROGRAM
";
        let result: CompileResult =
            serde_json::from_str(&compile(source, "iec61131-3-ed3", "", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_default_dialect_then_rejects_ltime_as_type() {
        let source = "
PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#100ms;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn compile_when_dialect_2013_without_allow_sizeof_then_rejects_sizeof() {
        let source = "
PROGRAM main
  VAR
    x : INT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
";
        let result: CompileResult =
            serde_json::from_str(&compile(source, "iec61131-3-ed3", "", "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn compile_when_dialect_2013_with_allow_sizeof_then_accepts_sizeof() {
        let source = "
PROGRAM main
  VAR
    x : INT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
";
        let result: CompileResult =
            serde_json::from_str(&compile(source, "iec61131-3-ed3", "sizeof", "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_allows_has_unknown_name_then_ignored() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(
            source,
            "iec61131-3-ed3",
            "not-a-real-flag,sizeof",
            "",
        ))
        .unwrap();
        assert!(result.ok);
    }

    #[test]
    fn compile_when_allows_has_multiple_then_each_applied() {
        // Use Ed3 baseline + two allows. Just verify it compiles successfully
        // (the program itself doesn't exercise the flags — we're checking the
        // allows parser handles whitespace and commas).
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(
            source,
            "iec61131-3-ed3",
            " sizeof , c-style-comments ",
            "",
        ))
        .unwrap();
        assert!(result.ok, "Expected ok but got: {:?}", result.diagnostics);
    }

    #[test]
    fn dialect_from_when_canonical_names_then_resolves() {
        assert_eq!(dialect_from("iec61131-3-ed2"), Dialect::Iec61131_3Ed2);
        assert_eq!(dialect_from("iec61131-3-ed3"), Dialect::Iec61131_3Ed3);
        assert_eq!(dialect_from("rusty"), Dialect::Rusty);
        assert_eq!(dialect_from("codesys"), Dialect::Codesys);
    }

    #[test]
    fn dialect_from_when_empty_or_unknown_then_defaults_to_rusty() {
        assert_eq!(dialect_from(""), Dialect::Rusty);
        assert_eq!(dialect_from("not-a-dialect"), Dialect::Rusty);
        // Year-based aliases are no longer recognized.
        assert_eq!(dialect_from("2013"), Dialect::Rusty);
    }

    #[test]
    fn dialects_when_called_then_lists_all_dialects_with_one_default() {
        let options: Vec<DialectOption> = serde_json::from_str(&dialects()).unwrap();
        assert_eq!(options.len(), Dialect::ALL.len());
        // Every listed value round-trips back to a real dialect.
        for opt in &options {
            assert!(opt.value.parse::<Dialect>().is_ok());
            assert!(!opt.label.is_empty());
        }
        // Exactly one default, and it is the compiler's default dialect.
        let defaults: Vec<&DialectOption> = options.iter().filter(|o| o.is_default).collect();
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].value, Dialect::default().cli_name());
    }

    #[test]
    fn compile_when_ed2_and_cstyle_comment_then_error_carries_help() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  // C-style comment
  x := 1;
END_PROGRAM
";
        let result: CompileResult =
            serde_json::from_str(&compile(source, "iec61131-3-ed2", "", "")).unwrap();
        assert!(!result.ok);
        let cstyle = result
            .diagnostics
            .iter()
            .find(|d| d.code == "P0004")
            .unwrap();
        assert!(!cstyle.help.is_empty());
    }

    #[test]
    fn compile_when_codesys_and_cstyle_comment_then_ok() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  // C-style comment
  x := 1;
END_PROGRAM
";
        let result: CompileResult =
            serde_json::from_str(&compile(source, "codesys", "", "")).unwrap();
        assert!(result.ok, "Expected ok but got: {:?}", result.diagnostics);
    }
}
