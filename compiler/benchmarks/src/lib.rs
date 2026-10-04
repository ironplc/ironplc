//! Shared helpers for the IronPLC benchmark suite.
//!
//! Hosts utilities used by both the Criterion benchmarks under `benches/`
//! and the integration tests under `tests/`.

pub mod programs;

use ironplc_analyzer::CleanAnalysis;
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

/// Compiles an IEC 61131-3 source string through the full pipeline:
/// parse → analyze (all semantic rules) → codegen.
///
/// Panics if the source fails to parse, has semantic diagnostics, or fails
/// to compile. The panic message lists the diagnostic codes so a failing
/// benchmark program can be fixed without re-running under a debugger.
pub fn compile_st(source: &str) -> Container {
    let options = CompilerOptions::default();
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let analysis = CleanAnalysis::new(&analyzed, &context).unwrap_or_else(|diagnostics| {
        let codes: Vec<&str> = diagnostics.iter().map(|d| d.code.as_str()).collect();
        panic!("Source has semantic diagnostics: {codes:?}")
    });
    let codegen_options = ironplc_codegen::CodegenOptions::default();
    compile(analysis, &codegen_options, &ironplc_codegen::EmptyLookup).unwrap()
}
