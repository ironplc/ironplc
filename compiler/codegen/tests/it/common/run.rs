//! Parse, compile and run helpers shared by the codegen integration tests.

use ironplc_analyzer::{CleanAnalysis, SemanticContext};
use ironplc_codegen::{compile_with_route_counts, Route, RouteCounts};
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::{load_and_start, TestVm};
use ironplc_vm::{FaultContext, VmBuffers};

use super::session::Session;
use super::value::Value;

/// Options for programs that use bit strings the way CODESYS and TwinCAT do: an
/// untyped integer literal assigned to a `BYTE`/`WORD`/`DWORD`/`LWORD`
/// (ADR-0031) and arithmetic on a bit string (ADR-0053). The strict default
/// rejects both.
pub fn bit_string_options() -> CompilerOptions {
    CompilerOptions {
        allow_int_literal_to_bit_string: true,
        allow_bit_string_arithmetic: true,
        ..CompilerOptions::default()
    }
}

/// Parses an IEC 61131-3 source string and runs type resolution via the analyzer.
///
/// The analyzer populates `Expr.expr_type` and resolves type aliases in
/// variable declarations, which codegen requires.
pub fn parse(source: &str, options: &CompilerOptions) -> (Library, SemanticContext) {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::analyze(&[&library], options).unwrap();
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
    compile_analyzed(&library, &context, options)
}

/// Compiles an analyzed library, such as user source analyzed together with a
/// bundled library, into a Container.
///
/// The library is compiled by both routes: every statement from the AST, and
/// the statements lowering supports from the lowered program
/// (`specs/design/lowered-program.md`, "Delivery Constraints"). Both must
/// compile, or both fail; a disagreement fails the test. The container
/// returned is the one compiled from the AST.
pub fn compile_analyzed(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> Result<Container, Diagnostic> {
    compile_both_routes(library, context, options).map(|(ast, _)| ast)
}

/// Like [`compile_analyzed`], for a program the test will run: one scan of
/// each route must also leave every variable with the same value, or both
/// trap. A test that only compiles does not run a scan, since its program
/// may never finish one.
pub fn compile_to_run(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> Result<Container, Diagnostic> {
    let (ast, lowered) = compile_both_routes(library, context, options)?;
    assert_routes_agree(&ast, &lowered);
    Ok(ast)
}

/// Parses, analyzes and compiles `source` with [`compile_to_run`].
pub fn parse_and_compile_to_run(source: &str, options: &CompilerOptions) -> Container {
    let (library, context) = parse(source, options);
    compile_to_run(&library, &context, options).unwrap()
}

/// Compiles an analyzed library by the AST route and by the lowered route,
/// failing the test unless both compile or both fail.
fn compile_both_routes(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> Result<(Container, Container), Diagnostic> {
    let ast = compile_by_route(library, context, options, Route::Ast);
    let lowered = compile_counting(library, context, options, Route::PreferLowered);
    match (ast, lowered) {
        (Ok(ast), Ok((lowered, counts))) => {
            record_route_counts(counts);
            Ok((ast, lowered))
        }
        (Err(diagnostic), Err(_)) => Err(diagnostic),
        (Ok(_), Err(diagnostic)) => {
            panic!("only the lowered route failed to compile: {diagnostic:?}")
        }
        (Err(diagnostic), Ok(_)) => panic!("only the AST route failed to compile: {diagnostic:?}"),
    }
}

/// Compiles an analyzed library with every statement taking `route`.
pub fn compile_by_route(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
    route: Route,
) -> Result<Container, Diagnostic> {
    compile_counting(library, context, options, route).map(|(container, _)| container)
}

/// Like [`compile_by_route`], also counting the statements each route compiled.
fn compile_counting(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
    route: Route,
) -> Result<(Container, RouteCounts), Diagnostic> {
    let analysis = CleanAnalysis::new(library, context).expect("analysis reported diagnostics");
    let codegen_options = ironplc_codegen::CodegenOptions {
        route,
        ..ironplc_codegen::CodegenOptions::from(options)
    };
    compile_with_route_counts(analysis, &codegen_options, &ironplc_codegen::EmptyLookup)
}

/// Appends `counts` as one line, `<lowered> <from AST>`, to the file the
/// `IRONPLC_ROUTE_COUNTS` environment variable names, when it names one. The
/// `lowered-count` recipe of `compiler/justfile` sums the lines to report how
/// many statements of these programs take the lowered route.
fn record_route_counts(counts: RouteCounts) {
    let Some(path) = std::env::var_os("IRONPLC_ROUTE_COUNTS") else {
        return;
    };
    let line = format!("{} {}\n", counts.lowered, counts.from_ast);
    // One `write` of a line opened for append lands whole, though tests
    // record from many threads.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
        .expect("cannot record the route counts");
}

/// Fails the test unless one scan of `ast` and one of `lowered` leave every
/// variable, and the data region, with the same value, or both trap.
fn assert_routes_agree(ast: &Container, lowered: &Container) {
    match (run_one_scan(ast), run_one_scan(lowered)) {
        (Ok(ast_bufs), Ok(lowered_bufs)) => {
            let variables = super::variables::Variables::of(ast);
            for (index, (a, l)) in ast_bufs.vars.iter().zip(&lowered_bufs.vars).enumerate() {
                assert_eq!(
                    a,
                    l,
                    "the AST and lowered routes leave variable {} with different values",
                    variables.name_at(index).unwrap_or("(unnamed)")
                );
            }
            assert_eq!(
                ast_bufs.data_region, lowered_bufs.data_region,
                "the AST and lowered routes leave the data region with different values"
            );
        }
        (Err(_), Err(_)) => {}
        (Ok(_), Err(fault)) => panic!("only the lowered route traps: {fault:?}"),
        (Err(fault), Ok(_)) => panic!("only the AST route traps: {fault:?}"),
    }
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(
    source: &str,
    options: &CompilerOptions,
) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source, options);
    let container = compile_to_run(&library, &context, options).unwrap();
    let bufs = run_one_scan(&container)?;
    Ok((container, bufs))
}

/// Loads `container` and runs one scan cycle, returning `Err` on VM trap.
pub fn run_one_scan(container: &Container) -> Result<VmBuffers, FaultContext> {
    let mut bufs = VmBuffers::from_container(container);
    {
        let mut vm = load_and_start(container, &mut bufs)?;
        assert_stack_balanced(&vm, "after init");
        vm.run_round(0)?;
        assert_stack_balanced(&vm, "after scan round");
    }
    Ok(bufs)
}

/// Asserts the VM's operand stack is empty.
///
/// Every function body is stack-balanced, so a completed scan must leave
/// the operand stack exactly as it found it. Nothing truncates that buffer
/// between rounds (`compiler/vm/src/stack.rs` has no `clear`, and
/// `run_round` does not call `truncate_by`), so a single leaked slot
/// survives every subsequent round and accumulates until the stack
/// overflows -- surfacing as a `Trap::StackOverflow` arbitrarily far in
/// time and code from the codegen path that leaked it.
///
/// Calling this from the shared harness makes every end-to-end test in
/// this suite a balance regression test, including tests written long
/// after this check was added.
pub fn assert_stack_balanced(vm: &ironplc_vm::VmRunning<'_>, phase: &str) {
    assert_eq!(
        vm.operand_stack_depth(),
        0,
        "operand stack not empty {phase}: {} value(s) left behind",
        vm.operand_stack_depth()
    );
}

/// Parses, analyzes, compiles, and runs a multi-round test scenario on the
/// VM itself, for tests whose subject is the VM's API.
///
/// The closure receives the container, to find a variable's slot with
/// [`vm_var_index`](super::vm_var_index), and a mutable VM reference so it can
/// write variables, run multiple rounds, and read back results.
pub fn parse_and_run_rounds(
    source: &str,
    options: &CompilerOptions,
    f: impl FnOnce(&Container, &mut TestVm<'_>),
) {
    let container = parse_and_compile_to_run(source, options);
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    assert_stack_balanced(&vm, "after init");
    f(&container, &mut vm);
    // The closure may have run any number of rounds. Each individual round
    // is covered by the `debug_assert` at the end of `VmRunning::run_round`;
    // this catches the final state even when that assertion is compiled out.
    assert_stack_balanced(&vm, "after scenario");
}

/// Parses, compiles and loads `source`, then hands `f` a [`Session`] to drive
/// it across scans, reading and writing variables by name.
pub fn run_scans(source: &str, options: &CompilerOptions, f: impl FnOnce(&mut Session<'_, '_>)) {
    run_compiled_scans(&parse_and_compile_to_run(source, options), f);
}

/// Loads the compiled `container`, then hands `f` a [`Session`] to drive it
/// across scans, as [`run_scans`] does, and returns what `f` returns.
pub fn run_compiled_scans<T>(
    container: &Container,
    f: impl FnOnce(&mut Session<'_, '_>) -> T,
) -> T {
    let mut bufs = VmBuffers::from_container(container);
    let mut vm = load_and_start(container, &mut bufs).unwrap();
    assert_stack_balanced(&vm, "after init");
    let observed = f(&mut Session::new(container, &mut vm));
    assert_stack_balanced(&vm, "after scenario");
    observed
}

/// A single step in a function-block (`TON`/`CTU`/`R_TRIG`/`RS`/…) driver
/// scenario, built with [`write`], [`run`], [`expect`] and [`pulse`].
///
/// The timer/counter/edge/bistable end-to-end tests all share the same shape:
/// build a program, then drive it across several scans, writing inputs and
/// asserting outputs along the way. Each scenario is a `&[FbStep]` table
/// executed by [`drive_fb`], so every original scenario stays one `rstest`
/// `#[case]` while the driver lives in exactly one place.
#[derive(Clone, Debug)]
pub enum FbStep {
    /// Write a value to a variable (no scan runs).
    Write(&'static str, Value),
    /// Run one scan at absolute VM time `time_us` (microseconds).
    Run(u64),
    /// Assert a variable currently reads a value.
    Expect(&'static str, Value),
    /// Feed `n` rising edges on a variable: for each edge, write 1 and run a
    /// scan, then write 0 and run a scan. Scans run at `time_base + i*2` and
    /// `+1`. Edge/counter FBs are edge-triggered, so the exact time values only
    /// need to increase monotonically.
    Pulse {
        var: &'static str,
        n: u64,
        time_base: u64,
    },
}

/// A step writing `value` to the variable `name`.
pub fn write(name: &'static str, value: impl Into<Value>) -> FbStep {
    FbStep::Write(name, value.into())
}

/// A step running one scan at absolute VM time `time_us` (microseconds).
pub fn run(time_us: u64) -> FbStep {
    FbStep::Run(time_us)
}

/// A step asserting the variable `name` reads `value`; a `BOOL` reads as 1 or 0.
pub fn expect(name: &'static str, value: impl Into<Value>) -> FbStep {
    FbStep::Expect(name, value.into())
}

/// A step feeding `n` rising edges on the variable `name`, from `time_base`.
pub fn pulse(name: &'static str, n: u64, time_base: u64) -> FbStep {
    FbStep::Pulse {
        var: name,
        n,
        time_base,
    }
}

/// Compiles `source` and drives it through `steps`, executing each [`FbStep`]
/// in order.
pub fn drive_fb(source: &str, options: &CompilerOptions, steps: &[FbStep]) {
    run_scans(source, options, |session| {
        for step in steps {
            match step {
                FbStep::Write(name, value) => session.write(name, value.clone()),
                FbStep::Run(time_us) => session.scan(*time_us).unwrap(),
                FbStep::Expect(name, expected) => {
                    let actual = session.read(name);
                    assert!(
                        actual.matches(expected),
                        "`{name}`: expected {expected:?}, got {actual:?}"
                    );
                }
                FbStep::Pulse { var, n, time_base } => {
                    for i in 0..*n {
                        session.write(var, 1);
                        session.scan(time_base + i * 2).unwrap();
                        session.write(var, 0);
                        session.scan(time_base + i * 2 + 1).unwrap();
                    }
                }
            }
        }
    });
}
