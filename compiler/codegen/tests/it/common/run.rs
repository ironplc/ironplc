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

/// Reads a STRING value from the data region at the given byte offset.
pub fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + ironplc_container::STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Computes the data_offset of a STRING variable given the declared max
/// lengths of the string variables that precede it in declaration order.
///
/// Each STRING variable occupies `STRING_HEADER_BYTES + max_length` bytes,
/// so `string_offset(&[])` is the first declared string and
/// `string_offset(&[254, 254])` is the third.
pub fn string_offset(preceding_max_lengths: &[u16]) -> usize {
    preceding_max_lengths
        .iter()
        .map(|&ml| ironplc_container::STRING_HEADER_BYTES + ml as usize)
        .sum()
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
    let codegen_options = ironplc_codegen::CodegenOptions::from(options);
    compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
}

/// Parses, analyzes, compiles, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
pub fn parse_and_run(source: &str, options: &CompilerOptions) -> (Container, VmBuffers) {
    let (container, bufs) = parse_and_try_run(source, options).unwrap();
    (container, bufs)
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(
    source: &str,
    options: &CompilerOptions,
) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source, options);
    let codegen_options = ironplc_codegen::CodegenOptions::from(options);
    let container = compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs)?;
        assert_stack_balanced(&vm, "after init");
        vm.run_round(0)?;
        assert_stack_balanced(&vm, "after scan round");
    }
    Ok((container, bufs))
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
    let codegen_options = ironplc_codegen::CodegenOptions::from(options);
    let container = compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    assert_stack_balanced(&vm, "after init");
    f(&mut vm);
    // The closure may have run any number of rounds. Each individual round
    // is covered by the `debug_assert` at the end of `VmRunning::run_round`;
    // this catches the final state even when that assertion is compiled out.
    assert_stack_balanced(&vm, "after scenario");
}

/// A single step in a function-block (`TON`/`CTU`/`R_TRIG`/`RS`/…) driver
/// scenario.
///
/// The timer/counter/edge/bistable end-to-end tests all share the same shape:
/// build a program, then drive the VM across several scan rounds, writing
/// inputs and asserting outputs along the way. Rather than repeat that
/// `load_and_start` + `run_round` + `read_variable` scaffold in every test,
/// each scenario is expressed as a `&[FbStep]` table and executed by
/// [`drive_fb`]. This keeps every original scenario as one `rstest` `#[case]`
/// while the driver lives in exactly one place.
#[derive(Clone, Copy)]
pub enum FbStep {
    /// Write `value` into the variable at `index` (no scan round runs).
    Write(u16, i32),
    /// Run one scan round at absolute VM time `time_us` (microseconds).
    Run(u64),
    /// Assert the variable at `index` currently reads `value`.
    Expect(u16, i32),
    /// Feed `n` rising edges on the variable at `var`: for each edge, write 1
    /// and run a round, then write 0 and run a round. Rounds run at
    /// `time_base + i*2` and `+1`. Edge/counter FBs are edge-triggered, so the
    /// exact time values only need to increase monotonically.
    Pulse { var: u16, n: u64, time_base: u64 },
}

/// Compiles `source` and drives the VM through `steps`, executing each
/// [`FbStep`] in order. See [`FbStep`] for the step semantics.
pub fn drive_fb(source: &str, options: &CompilerOptions, steps: &[FbStep]) {
    use ironplc_container::VarIndex;
    parse_and_run_rounds(source, options, |vm| {
        for step in steps {
            match *step {
                FbStep::Write(index, value) => {
                    vm.write_variable(VarIndex::new(index), value).unwrap();
                }
                FbStep::Run(time_us) => {
                    vm.run_round(time_us).unwrap();
                    assert_stack_balanced(&*vm, "after scan round");
                }
                FbStep::Expect(index, value) => {
                    assert_eq!(
                        vm.read_variable(VarIndex::new(index)).unwrap(),
                        value,
                        "vars[{index}] mismatch"
                    );
                }
                FbStep::Pulse { var, n, time_base } => {
                    for i in 0..n {
                        vm.write_variable(VarIndex::new(var), 1).unwrap();
                        vm.run_round(time_base + i * 2).unwrap();
                        assert_stack_balanced(&*vm, "after rising-edge round");
                        vm.write_variable(VarIndex::new(var), 0).unwrap();
                        vm.run_round(time_base + i * 2 + 1).unwrap();
                        assert_stack_balanced(&*vm, "after falling-edge round");
                    }
                }
            }
        }
    });
}
