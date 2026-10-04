//! Parse, compile and run helpers shared by the codegen integration tests.

use ironplc_analyzer::SemanticContext;
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{FaultContext, VmBuffers};

use super::session::Session;
use super::value::Value;

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
pub fn compile_analyzed(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> Result<Container, Diagnostic> {
    let codegen_options = ironplc_codegen::CodegenOptions::from(options);
    compile(
        library,
        context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(
    source: &str,
    options: &CompilerOptions,
) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source, options);
    let container = compile_analyzed(&library, &context, options).unwrap();
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
    f: impl FnOnce(&Container, &mut ironplc_vm::VmRunning<'_>),
) {
    let container = parse_and_compile(source, options);
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
    let container = parse_and_compile(source, options);
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    assert_stack_balanced(&vm, "after init");
    f(&mut Session::new(&container, &mut vm));
    assert_stack_balanced(&vm, "after scenario");
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
