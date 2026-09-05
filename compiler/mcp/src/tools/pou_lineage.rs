//! The `pou_lineage` MCP tool.
//!
//! Returns the upstream (dependencies) and downstream (dependents) of a
//! named POU, derived from the library's call graph. Standard library POUs
//! participate in the graph like any other. Implements REQ-TOL-mcp-230,
//! REQ-TOL-mcp-231, and REQ-TOL-mcp-232.

use std::collections::{BTreeMap, BTreeSet};

use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::{
    FunctionBlockCallInitializer, FunctionBlockDeclaration, FunctionBlockInitialValueAssignment,
    FunctionDeclaration, Library, LibraryElementKind, ProgramDeclaration, VarDecl,
};
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{FbCall, Function};
use ironplc_dsl::visitor::Visitor;
use ironplc_project::project::{MemoryBackedProject, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PouLineageInput {
    pub sources: Vec<SourceInput>,
    /// Compiler options (dialect + optional feature-flag overrides).
    #[schemars(schema_with = "super::common::options_schema")]
    pub options: serde_json::Value,
    pub pou: String,
}

/// Where a POU is declared.
///
/// `Stdlib` marks the function blocks and functions the compiler provides
/// itself (TON, R_TRIG, MAX, ...); everything else is `User`, including POUs
/// that arrive through an activated compatibility library, which are files
/// the caller can supply like any other source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PouSource {
    User,
    Stdlib,
}

/// One POU in an `upstream` or `downstream` list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineageEntry {
    pub name: String,
    pub source: PouSource,
}

#[derive(Debug, Serialize)]
pub struct PouLineageResponse {
    pub ok: bool,
    pub found: bool,
    pub pou: String,
    pub upstream: Vec<LineageEntry>,
    pub downstream: Vec<LineageEntry>,
    pub diagnostics: Vec<serde_json::Value>,
}

impl PouLineageResponse {
    fn empty(ok: bool, found: bool, pou: String, diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            ok,
            found,
            pou,
            upstream: vec![],
            downstream: vec![],
            diagnostics,
        }
    }
}

/// Builds the `pou_lineage` response.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
    pou_name: &str,
) -> PouLineageResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return PouLineageResponse::empty(
            false,
            false,
            pou_name.to_string(),
            serialize_diagnostics(&source_errors),
        );
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return PouLineageResponse::empty(
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

    let mut diagnostics_json = serialize_diagnostics(&project.semantic());

    let has_errors = diagnostics_json
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));

    let library = match project.analyzed_library() {
        Some(lib) => lib,
        None => {
            return PouLineageResponse::empty(
                !has_errors,
                false,
                pou_name.to_string(),
                diagnostics_json,
            );
        }
    };

    let context = project.semantic_context();
    let graph = build_graph(library, context);
    let canonical = match graph.canonical_name(pou_name) {
        Some(n) => n,
        None => {
            diagnostics_json.push(not_found_diagnostic(pou_name));
            return PouLineageResponse {
                ok: false,
                found: false,
                pou: pou_name.to_string(),
                upstream: vec![],
                downstream: vec![],
                diagnostics: diagnostics_json,
            };
        }
    };

    let upstream = graph.transitive_upstream(&canonical);
    let downstream = graph.transitive_downstream(&canonical);

    PouLineageResponse {
        ok: !has_errors,
        found: true,
        pou: canonical,
        upstream,
        downstream,
        diagnostics: diagnostics_json,
    }
}

/// A simple directed graph over POU display names.
///
/// An edge `a -> b` means "POU `a` depends on POU `b`" (i.e. `a` calls `b`
/// or instantiates `b` as a function-block variable).
struct PouGraph {
    /// Lowercase → display name for every POU in the library.
    display: BTreeMap<String, String>,
    /// Lowercase → where the POU is declared.
    source: BTreeMap<String, PouSource>,
    /// Outgoing edges (upstream): caller lowercase → set of callee lowercase.
    edges_out: BTreeMap<String, BTreeSet<String>>,
    /// Incoming edges (downstream): callee lowercase → set of caller lowercase.
    edges_in: BTreeMap<String, BTreeSet<String>>,
}

impl PouGraph {
    fn new() -> Self {
        Self {
            display: BTreeMap::new(),
            source: BTreeMap::new(),
            edges_out: BTreeMap::new(),
            edges_in: BTreeMap::new(),
        }
    }

    /// Registers a POU. The first registration of a name wins, so user
    /// declarations are recorded before standard library ones.
    fn add_pou(&mut self, name: &str, source: PouSource) {
        let lower = name.to_lowercase();
        if self.display.contains_key(&lower) {
            return;
        }
        self.display.insert(lower.clone(), name.to_string());
        self.source.insert(lower.clone(), source);
        self.edges_out.entry(lower.clone()).or_default();
        self.edges_in.entry(lower).or_default();
    }

    fn add_edge(&mut self, caller: &str, callee: &str) {
        let c_lower = caller.to_lowercase();
        let e_lower = callee.to_lowercase();
        if c_lower == e_lower {
            return; // no self edges in lineage
        }
        if !self.display.contains_key(&e_lower) {
            return; // the name resolves to no POU at all
        }
        self.edges_out
            .entry(c_lower.clone())
            .or_default()
            .insert(e_lower.clone());
        self.edges_in.entry(e_lower).or_default().insert(c_lower);
    }

    fn canonical_name(&self, name: &str) -> Option<String> {
        self.display.get(&name.to_lowercase()).cloned()
    }

    fn transitive_upstream(&self, start: &str) -> Vec<LineageEntry> {
        self.transitive(&self.edges_out, start)
    }

    fn transitive_downstream(&self, start: &str) -> Vec<LineageEntry> {
        self.transitive(&self.edges_in, start)
    }

    fn transitive(
        &self,
        edges: &BTreeMap<String, BTreeSet<String>>,
        start: &str,
    ) -> Vec<LineageEntry> {
        let start_lower = start.to_lowercase();
        let mut visited = BTreeSet::new();
        let mut stack = vec![start_lower.clone()];
        while let Some(node) = stack.pop() {
            if let Some(neighbours) = edges.get(&node) {
                for n in neighbours {
                    if visited.insert(n.clone()) {
                        stack.push(n.clone());
                    }
                }
            }
        }
        // Exclude the starting POU from its own lineage.
        visited.remove(&start_lower);
        let mut out: Vec<LineageEntry> = visited
            .into_iter()
            .filter_map(|l| {
                Some(LineageEntry {
                    name: self.display.get(&l).cloned()?,
                    source: *self.source.get(&l)?,
                })
            })
            .collect();
        out.sort_by_key(|e| e.name.to_lowercase());
        out
    }
}

fn build_graph(library: &Library, context: Option<&SemanticContext>) -> PouGraph {
    let mut graph = PouGraph::new();

    // First pass: register every POU so references can be resolved.
    for element in &library.elements {
        match element {
            LibraryElementKind::ProgramDeclaration(p) => {
                graph.add_pou(&p.name.to_string(), PouSource::User)
            }
            LibraryElementKind::FunctionDeclaration(f) => {
                graph.add_pou(&f.name.to_string(), PouSource::User)
            }
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                graph.add_pou(&fb.name.to_string(), PouSource::User)
            }
            _ => {}
        }
    }

    // The standard library POUs the compiler supplies itself are real
    // dependencies but never library elements, so register them from the
    // semantic environments.
    if let Some(context) = context {
        register_stdlib_pous(&mut graph, context);
    }

    // Second pass: record edges.
    for element in &library.elements {
        match element {
            LibraryElementKind::ProgramDeclaration(p) => record_program(&mut graph, p),
            LibraryElementKind::FunctionDeclaration(f) => record_function(&mut graph, f),
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                record_function_block(&mut graph, fb)
            }
            _ => {}
        }
    }

    graph
}

/// Registers the standard library function blocks and functions as POUs.
///
/// A standard library declaration is the one the compiler builds itself, and
/// it is the only kind carrying a builtin span: everything reaching the
/// environments from a file — user source and activated compatibility
/// libraries alike — carries that file's span.
fn register_stdlib_pous(graph: &mut PouGraph, context: &SemanticContext) {
    for (name, attributes) in context.types().iter() {
        if attributes.span.is_builtin() && attributes.representation.is_function_block() {
            // The type environment case-normalizes its keys, so the spelling
            // stored for a standard library function block is lowercase and
            // says nothing about how the POU is named. IEC 61131-3 names every
            // one of them in upper case (TON, R_TRIG, CTUD_UDINT), so report
            // that rather than a registry artifact.
            graph.add_pou(&name.to_string().to_uppercase(), PouSource::Stdlib);
        }
    }

    // Function signatures keep the spelling they were declared with.
    for (_, signature) in context.functions().iter() {
        if signature.is_stdlib() {
            graph.add_pou(signature.name.original(), PouSource::Stdlib);
        }
    }
}

fn record_program(graph: &mut PouGraph, p: &ProgramDeclaration) {
    let caller = p.name.to_string();
    record_variables(graph, &caller, &p.variables);
    let mut collector = ReferenceCollector::new(&caller, graph);
    let _ = collector.visit_function_block_body_kind(&p.body);
}

fn record_function(graph: &mut PouGraph, f: &FunctionDeclaration) {
    let caller = f.name.to_string();
    record_variables(graph, &caller, &f.variables);
    let mut collector = ReferenceCollector::new(&caller, graph);
    for stmt in &f.body {
        let _ = collector.visit_stmt_kind(stmt);
    }
}

fn record_function_block(graph: &mut PouGraph, fb: &FunctionBlockDeclaration) {
    let caller = fb.name.to_string();
    record_variables(graph, &caller, &fb.variables);
    let mut collector = ReferenceCollector::new(&caller, graph);
    let _ = collector.visit_function_block_body_kind(&fb.body);
}

fn record_variables(graph: &mut PouGraph, caller: &str, variables: &[VarDecl]) {
    for v in variables {
        // Both the member-init form (`X : FB := (...)`) and the call-style
        // form (`X : FB(...)`) declare an instance of an FB type, so both
        // create a lineage edge to that type.
        let fb_type_name = match &v.initializer {
            ironplc_dsl::common::InitialValueAssignmentKind::FunctionBlock(
                FunctionBlockInitialValueAssignment { type_name, .. },
            ) => Some(type_name),
            ironplc_dsl::common::InitialValueAssignmentKind::FunctionBlockCall(
                FunctionBlockCallInitializer { type_name, .. },
            ) => Some(type_name),
            _ => None,
        };
        if let Some(type_name) = fb_type_name {
            graph.add_edge(caller, &type_name.to_string());
        }
    }
}

/// Visitor that records `caller` calls every `Function` and every `FbCall`
/// into the graph. FB calls are recorded by dispatching the variable name
/// against the caller's own FB variables; for cross-POU references we rely
/// on the VAR block walk in `record_variables`, which captures the FB
/// *type* dependency. Calls to standard library functions record an edge
/// like any other call — those POUs are registered by
/// `register_stdlib_pous`.
struct ReferenceCollector<'a> {
    caller: String,
    graph: &'a mut PouGraph,
}

impl<'a> ReferenceCollector<'a> {
    fn new(caller: &str, graph: &'a mut PouGraph) -> Self {
        Self {
            caller: caller.to_string(),
            graph,
        }
    }
}

impl<'a> Visitor<Diagnostic> for ReferenceCollector<'a> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        self.graph.add_edge(&self.caller, &node.name.to_string());
        // Still recurse into the argument list so nested calls are captured.
        for p in &node.param_assignment {
            let _ = self.visit_param_assignment_kind(p);
        }
        Ok(())
    }

    fn visit_fb_call(&mut self, _node: &FbCall) -> Result<Self::Value, Diagnostic> {
        // The variable name targets a specific FB *instance*; the FB type
        // dependency is already captured by `record_variables`. Nothing to
        // do here — avoid double counting.
        Ok(())
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
    use crate::tools::test_support::ed2_options;

    fn contains(entries: &[LineageEntry], name: &str) -> bool {
        entries.iter().any(|e| e.name.eq_ignore_ascii_case(name))
    }

    fn source_of(entries: &[LineageEntry], name: &str) -> Option<PouSource> {
        entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .map(|e| e.source)
    }

    fn build(src: &str, pou: &str) -> PouLineageResponse {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: src.into(),
        }];
        build_response(&sources, &ed2_options(), pou)
    }

    #[test]
    fn build_response_when_program_uses_fb_then_upstream_has_fb() {
        let resp = build(
            "FUNCTION_BLOCK Counter\nVAR_INPUT Inc : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR c : Counter; END_VAR\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert!(contains(&resp.upstream, "Counter"));
        assert_eq!(source_of(&resp.upstream, "Counter"), Some(PouSource::User));
        assert!(resp.downstream.is_empty());
    }

    #[test]
    fn build_response_when_fb_is_used_then_downstream_has_caller() {
        let resp = build(
            "FUNCTION_BLOCK Counter\nVAR_INPUT Inc : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR c : Counter; END_VAR\nEND_PROGRAM",
            "Counter",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert!(resp.upstream.is_empty());
        assert!(contains(&resp.downstream, "Main"));
    }

    #[test]
    fn build_response_when_function_calls_function_then_upstream_has_callee() {
        let resp = build(
            "FUNCTION AddPair : INT\nVAR_INPUT a : INT; b : INT; END_VAR\nAddPair := a + b;\nEND_FUNCTION\nFUNCTION Twice : INT\nVAR_INPUT x : INT; END_VAR\nTwice := AddPair(a := x, b := x);\nEND_FUNCTION\nPROGRAM Main\nVAR r : INT; END_VAR\nr := Twice(x := 3);\nEND_PROGRAM",
            "Twice",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert!(contains(&resp.upstream, "AddPair"));
    }

    #[test]
    fn build_response_when_chain_then_transitive_upstream() {
        let resp = build(
            "FUNCTION AddPair : INT\nVAR_INPUT a : INT; b : INT; END_VAR\nAddPair := a + b;\nEND_FUNCTION\nFUNCTION Twice : INT\nVAR_INPUT x : INT; END_VAR\nTwice := AddPair(a := x, b := x);\nEND_FUNCTION\nPROGRAM Main\nVAR r : INT; END_VAR\nr := Twice(x := 3);\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        // Main → Twice → AddPair.
        assert!(contains(&resp.upstream, "Twice"));
        assert!(contains(&resp.upstream, "AddPair"));
    }

    #[test]
    fn build_response_when_chain_then_transitive_downstream() {
        let resp = build(
            "FUNCTION AddPair : INT\nVAR_INPUT a : INT; b : INT; END_VAR\nAddPair := a + b;\nEND_FUNCTION\nFUNCTION Twice : INT\nVAR_INPUT x : INT; END_VAR\nTwice := AddPair(a := x, b := x);\nEND_FUNCTION\nPROGRAM Main\nVAR r : INT; END_VAR\nr := Twice(x := 3);\nEND_PROGRAM",
            "AddPair",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        // Downstream of AddPair is {Twice, Main}.
        assert!(contains(&resp.downstream, "Twice"));
        assert!(contains(&resp.downstream, "Main"));
    }

    #[test]
    fn build_response_when_program_uses_stdlib_fb_then_upstream_has_stdlib_entry() {
        let resp = build(
            "PROGRAM MotorStartStop\nVAR Star_Timer : TON; Run : BOOL; END_VAR\nStar_Timer(IN := Run, PT := T#5s);\nEND_PROGRAM",
            "MotorStartStop",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        // Reported under the IEC spelling, not the type registry's key.
        assert_eq!(resp.upstream[0].name, "TON");
        assert_eq!(source_of(&resp.upstream, "TON"), Some(PouSource::Stdlib));
    }

    #[test]
    fn build_response_when_queried_pou_is_stdlib_fb_then_found_with_downstream() {
        let resp = build(
            "PROGRAM MotorStartStop\nVAR Star_Timer : TON; Run : BOOL; END_VAR\nStar_Timer(IN := Run, PT := T#5s);\nEND_PROGRAM",
            "TON",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert!(contains(&resp.downstream, "MotorStartStop"));
        // Standard library POUs are leaves: nothing of ours is upstream.
        assert!(resp.upstream.is_empty());
    }

    #[test]
    fn build_response_when_pou_calls_stdlib_function_then_upstream_has_stdlib_entry() {
        let resp = build(
            "PROGRAM Main\nVAR r : INT; END_VAR\nr := MAX(IN1 := 1, IN2 := 2);\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.found);
        assert!(contains(&resp.upstream, "MAX"));
        assert_eq!(source_of(&resp.upstream, "MAX"), Some(PouSource::Stdlib));
    }

    #[test]
    fn build_response_when_stdlib_fb_unused_then_absent_from_lineage() {
        // Registering the standard library must not put its POUs into the
        // lineage of a POU that does not use them.
        let resp = build(
            "FUNCTION_BLOCK Counter\nVAR_INPUT Inc : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR c : Counter; END_VAR\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.upstream.len(), 1);
        assert!(contains(&resp.upstream, "Counter"));
    }

    #[test]
    fn build_response_when_mixed_origins_then_sorted_by_name() {
        let resp = build(
            "FUNCTION_BLOCK Zebra\nVAR_INPUT Inc : BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR z : Zebra; t : TON; END_VAR\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let names: Vec<String> = resp
            .upstream
            .iter()
            .map(|e| e.name.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(source_of(&resp.upstream, "Zebra"), Some(PouSource::User));
        assert_eq!(source_of(&resp.upstream, "TON"), Some(PouSource::Stdlib));
    }

    #[test]
    fn build_response_when_pou_not_found_then_found_false_and_p8001() {
        let resp = build("PROGRAM p\nEND_PROGRAM", "nonexistent");
        assert!(!resp.ok);
        assert!(!resp.found);
        assert!(resp.upstream.is_empty());
        assert!(resp.downstream.is_empty());
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_self_reference_excluded() {
        // The queried POU must not appear in its own upstream/downstream.
        let resp = build(
            "FUNCTION AddOne : INT\nVAR_INPUT a : INT; END_VAR\nAddOne := a;\nEND_FUNCTION\nPROGRAM Main\nVAR r : INT; END_VAR\nr := AddOne(a := 1);\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok);
        assert!(!contains(&resp.upstream, "Main"));
        assert!(!contains(&resp.downstream, "Main"));
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
    fn build_response_when_upstream_and_downstream_sorted() {
        let resp = build(
            "FUNCTION AddOne : INT\nVAR_INPUT a : INT; END_VAR\nAddOne := a;\nEND_FUNCTION\nFUNCTION Zap : INT\nVAR_INPUT a : INT; END_VAR\nZap := a;\nEND_FUNCTION\nFUNCTION Mix : INT\nVAR_INPUT a : INT; END_VAR\nMix := a;\nEND_FUNCTION\nPROGRAM Main\nVAR r : INT; END_VAR\nr := AddOne(a := 1);\nr := Zap(a := 2);\nr := Mix(a := 3);\nEND_PROGRAM",
            "Main",
        );
        assert!(resp.ok);
        let mut sorted = resp.upstream.clone();
        sorted.sort_by_key(|e| e.name.to_lowercase());
        assert_eq!(resp.upstream, sorted);
    }
}
