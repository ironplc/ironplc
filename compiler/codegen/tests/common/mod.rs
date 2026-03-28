//! Shared test helpers for codegen integration tests.

#![allow(dead_code)]
#![allow(clippy::result_large_err)]

use ironplc_analyzer::SemanticContext;
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::FaultContext;
pub use ironplc_vm::VmBuffers;

/// Parses an IEC 61131-3 source string and runs type resolution via the analyzer.
///
/// The analyzer populates `Expr.resolved_type` and resolves type aliases in
/// variable declarations, which codegen requires.
pub fn parse(source: &str, options: &CompilerOptions) -> (Library, SemanticContext) {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], options).unwrap();
    (analyzed, ctx)
}

/// Parses, analyzes, and compiles an IEC 61131-3 source string into a Container.
pub fn parse_and_compile(source: &str, options: &CompilerOptions) -> Container {
    try_parse_and_compile(source, options).unwrap()
}

/// Like [`parse_and_compile`], but returns the Result so callers can test error cases.
pub fn try_parse_and_compile(
    source: &str,
    options: &CompilerOptions,
) -> Result<Container, Diagnostic> {
    let (library, context) = parse(source, options);
    compile(&library, &context)
}

/// Parses, analyzes, compiles, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
pub fn parse_and_run(source: &str, options: &CompilerOptions) -> (Container, VmBuffers) {
    let (container, bufs) =
        parse_and_try_run(source, options).expect("VM execution trapped unexpectedly");
    (container, bufs)
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(
    source: &str,
    options: &CompilerOptions,
) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source, options);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs)?;
        vm.run_round(0)?;
    }
    Ok((container, bufs))
}

/// Parses, analyzes, compiles, and runs a multi-round test scenario.
///
/// The closure receives a mutable VM reference so it can write variables,
/// run multiple rounds, and read back results.
pub fn parse_and_run_rounds(
    source: &str,
    options: &CompilerOptions,
    f: impl FnOnce(&mut ironplc_vm::VmRunning<'_>),
) {
    let (library, context) = parse(source, options);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    f(&mut vm);
}
