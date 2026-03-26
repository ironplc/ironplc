//! Shared test helpers for codegen integration tests.

#![allow(dead_code)]
#![allow(clippy::result_large_err)]

use ironplc_analyzer::SemanticContext;
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::{Dialect, ParseOptions};
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::FaultContext;
pub use ironplc_vm::VmBuffers;

/// Parses an IEC 61131-3 source string and runs type resolution via the analyzer.
///
/// The analyzer populates `Expr.resolved_type` and resolves type aliases in
/// variable declarations, which codegen requires.
pub fn parse(source: &str) -> (Library, SemanticContext) {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    (analyzed, ctx)
}

/// Like [`parse`], but enables IEC 61131-3 Edition 3 (2013) features such as LTIME.
pub fn parse_edition3(source: &str) -> (Library, SemanticContext) {
    let options = ParseOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    (analyzed, ctx)
}

/// Like [`parse`], but uses the RuSTy dialect (Edition 2 base with REF_TO
/// and all vendor extensions enabled).
pub fn parse_rusty(source: &str) -> (Library, SemanticContext) {
    let options = ParseOptions::from_dialect(Dialect::Rusty);
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    (analyzed, ctx)
}

/// Parses, analyzes, and compiles an IEC 61131-3 source string into a Container.
pub fn parse_and_compile(source: &str) -> Container {
    try_parse_and_compile(source).unwrap()
}

/// Like [`parse_and_compile`], but returns the Result so callers can test error cases.
pub fn try_parse_and_compile(source: &str) -> Result<Container, Diagnostic> {
    let (library, context) = parse(source);
    compile(&library, &context)
}

/// Like [`parse_and_compile`], but enables IEC 61131-3 Edition 3 (2013) features.
pub fn parse_and_compile_edition3(source: &str) -> Container {
    let (library, context) = parse_edition3(source);
    compile(&library, &context).unwrap()
}

/// Parses, analyzes, compiles, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
pub fn parse_and_run(source: &str) -> (Container, VmBuffers) {
    let (container, bufs) = parse_and_try_run(source).expect("VM execution trapped unexpectedly");
    (container, bufs)
}

/// Like [`parse_and_run`], but enables IEC 61131-3 Edition 3 (2013) features.
pub fn parse_and_run_edition3(source: &str) -> (Container, VmBuffers) {
    let (library, context) = parse_edition3(source);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm =
            load_and_start(&container, &mut bufs).expect("VM execution trapped unexpectedly");
        vm.run_round(0).expect("VM round trapped unexpectedly");
    }
    (container, bufs)
}

/// Like [`parse_and_run`], but uses the RuSTy dialect.
pub fn parse_and_run_rusty(source: &str) -> (Container, VmBuffers) {
    let (library, context) = parse_rusty(source);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm =
            load_and_start(&container, &mut bufs).expect("VM execution trapped unexpectedly");
        vm.run_round(0).expect("VM round trapped unexpectedly");
    }
    (container, bufs)
}

/// Like [`parse_and_try_run`], but enables IEC 61131-3 Edition 3 (2013) features.
pub fn parse_and_try_run_edition3(source: &str) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse_edition3(source);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs)?;
        vm.run_round(0)?;
    }
    Ok((container, bufs))
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(source: &str) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source);
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
pub fn parse_and_run_rounds(source: &str, f: impl FnOnce(&mut ironplc_vm::VmRunning<'_>)) {
    let (library, context) = parse(source);
    let container = compile(&library, &context).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    f(&mut vm);
}
