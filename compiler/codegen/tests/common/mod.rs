//! Shared test helpers for codegen integration tests.

#![allow(dead_code)]
#![allow(unused_macros)]
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
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    compile(&library, &context, &codegen_options)
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
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    let container = compile(&library, &context, &codegen_options).unwrap();
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
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    let container = compile(&library, &context, &codegen_options).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    f(&mut vm);
}

/// Runs `source` with default options and asserts each `(var_index, expected)`
/// pair against the corresponding `vars[i].as_i32()` slot after one scan.
///
/// This is the workhorse helper for the `end_to_end_*.rs` integer tests:
/// it collapses the recurring 3-line scaffold (`let source ...; let (_c, bufs)
/// = parse_and_run(...); assert_eq!(...)`) into a single call so that each
/// `#[test] fn` becomes one statement. Reduces duplicate AST mass enough that
/// `cargo dupes` no longer flags the tests as a group.
pub fn assert_run_i32(source: &str, asserts: &[(usize, i32)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

/// Same as [`assert_run_i32`] but reads slots as i64. Use for LINT/ULINT or
/// any value whose magnitude exceeds 32 bits.
pub fn assert_run_i64(source: &str, asserts: &[(usize, i64)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i64(), *expected, "vars[{idx}] mismatch");
    }
}

/// Like [`assert_run_i32`] but with explicit [`CompilerOptions`]. Use when a
/// test requires a non-default dialect flag (e.g. `allow_partial_access_syntax`).
pub fn assert_run_i32_with(source: &str, options: &CompilerOptions, asserts: &[(usize, i32)]) {
    let (_c, bufs) = parse_and_run(source, options);
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

/// Declares a `#[test] fn` that asserts an IEC 61131-3 program produces the
/// given i32 var values.
///
/// The macro form (vs writing the `#[test] fn` body directly as
/// `{ assert_run_i32(...); }`) matters for code duplication: without it,
/// every short 6-line body gets regrouped by `cargo dupes` as a new
/// exact-duplicate set. A macro invocation is opaque to the detector, so
/// each test becomes a single token and no new group forms.
///
/// Any `#[...]` attributes (including `///` docstrings) preceding the
/// macro invocation are forwarded to the generated `fn`.
///
/// Use from a test binary with `#[macro_use] mod common;`.
macro_rules! e2e_i32 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            common::assert_run_i32($source, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as i64 (LINT/ULINT).
macro_rules! e2e_i64 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            common::assert_run_i64($source, $asserts);
        }
    };
}

/// Like [`e2e_i32`] but takes a [`CompilerOptions`] expression so the test
/// can enable a non-default dialect flag. The options expression is
/// evaluated once inside the generated test body.
macro_rules! e2e_i32_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            common::assert_run_i32_with($source, &$opts, $asserts);
        }
    };
}
