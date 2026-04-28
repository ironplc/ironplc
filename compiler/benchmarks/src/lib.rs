//! Shared helpers for the IronPLC benchmark suite.
//!
//! Hosts utilities used by both the Criterion benchmarks under `benches/`
//! and the integration tests under `tests/`.

use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;

/// Compiles an IEC 61131-3 source string through the full pipeline:
/// parse → analyze (all semantic rules) → codegen.
pub fn compile_st(source: &str) -> Container {
    let options = CompilerOptions::default();
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "Source has semantic diagnostics"
    );
    let codegen_options = ironplc_codegen::CodegenOptions::default();
    compile(&analyzed, &context, &codegen_options).unwrap()
}
