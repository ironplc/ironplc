//! Performance benchmarks using compiled IEC 61131-3 Structured Text.
//!
//! These benchmarks compile real ST source through the full pipeline
//! (parser → analyzer → codegen) and measure VM execution time. They
//! complement the hand-crafted bytecode benchmarks in `ironplc-vm` by
//! exercising realistic code paths.
//!
//! The ST sources live in `ironplc_benchmarks::programs` so that
//! `tests/compile_programs.rs` can compile each one as a regular test.
//!
//! Run with: `cargo bench --package ironplc-benchmarks`

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use ironplc_benchmarks::compile_st;
use ironplc_benchmarks::programs;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{NoopDebugHook, Slot, VmBuffers};
use std::hint::black_box;

/// Runs one benchmark iteration: creates `VmBuffers`, applies `$setup`,
/// then executes one VM scan cycle.
macro_rules! bench_run {
    ($group:expr, $id:expr, $container:expr, |$bufs:ident| $setup:block) => {
        $group.bench_with_input($id, &(), |b, _| {
            b.iter_batched(
                || {
                    let mut $bufs = VmBuffers::from_container($container);
                    $setup
                    $bufs
                },
                |mut bufs| {
                    let mut vm = load_and_start($container, &mut bufs).unwrap();
                    black_box(vm.run_round(0).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    };
}

/// WHILE loop decrementing a counter — dispatch overhead baseline.
fn bench_counter_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_counter_loop");
    let container = compile_st(programs::COUNTER_LOOP);

    for count in [100, 1000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(count),
            &container,
            |bufs| {
                bufs.vars[0] = Slot::from_i32(count);
            }
        );
    }
    group.finish();
}

/// Straight-line DINT arithmetic — per-instruction cost.
fn bench_arithmetic_i32(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_arithmetic_i32");

    for reps in [10, 100, 1000] {
        let container = compile_st(&programs::arithmetic_i32(reps));

        group.throughput(Throughput::Elements(reps as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(reps),
            &container,
            |bufs| {
                bufs.vars[0] = Slot::from_i32(1);
            }
        );
    }
    group.finish();
}

/// Straight-line LREAL arithmetic.
fn bench_arithmetic_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_arithmetic_f64");

    for reps in [10, 100, 1000] {
        let container = compile_st(&programs::arithmetic_f64(reps));

        group.throughput(Throughput::Elements(reps as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(reps),
            &container,
            |bufs| {
                bufs.vars[0] = Slot::from_f64(1.0);
            }
        );
    }
    group.finish();
}

/// IF-ELSIF branching chain — worst-case sequential comparison.
fn bench_branching(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_branching");

    for branches in [5, 20, 50] {
        let container = compile_st(&programs::branching(branches));

        bench_run!(
            group,
            BenchmarkId::from_parameter(branches),
            &container,
            |bufs| {
                bufs.vars[0] = Slot::from_i32(branches - 1);
            }
        );
    }
    group.finish();
}

/// FOR loop summing 1..limit — structured loop overhead.
fn bench_for_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_for_loop");
    let container = compile_st(programs::FOR_LOOP);

    for limit in [100, 1000, 10_000] {
        group.throughput(Throughput::Elements(limit as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(limit),
            &container,
            |bufs| {
                bufs.vars[2] = Slot::from_i32(limit);
            }
        );
    }
    group.finish();
}

/// Nested FOR loops — exercises loop overhead at scale.
fn bench_nested_loops(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_nested_loops");
    let container = compile_st(programs::NESTED_LOOPS);

    for (outer, inner) in [(10, 10), (10, 100), (100, 100)] {
        let label = format!("{}x{}", outer, inner);
        group.throughput(Throughput::Elements((outer * inner) as u64));
        bench_run!(
            group,
            BenchmarkId::new("iters", &label),
            &container,
            |bufs| {
                bufs.vars[3] = Slot::from_i32(outer);
                bufs.vars[4] = Slot::from_i32(inner);
            }
        );
    }
    group.finish();
}

/// Narrow opcode diversity — loop body uses only ~5 distinct opcodes.
/// Baseline for comparison with diverse_opcodes below.
fn bench_narrow_opcodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_narrow_opcodes");
    let container = compile_st(programs::NARROW_OPCODES);

    for limit in [100, 1000, 10_000] {
        group.throughput(Throughput::Elements(limit as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(limit),
            &container,
            |bufs| {
                bufs.vars[2] = Slot::from_i32(limit);
            }
        );
    }
    group.finish();
}

/// Diverse opcode mix — loop body touches many distinct opcode handlers
/// (see [`programs::DIVERSE_OPCODES`] for the full list). This forces many
/// dispatch table entries into L1 icache simultaneously.
fn bench_diverse_opcodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_diverse_opcodes");
    let container = compile_st(programs::DIVERSE_OPCODES);

    for limit in [100, 1000, 10_000] {
        group.throughput(Throughput::Elements(limit as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(limit),
            &container,
            |bufs| {
                bufs.vars[1] = Slot::from_i32(limit);
            }
        );
    }
    group.finish();
}

/// Five distinct INT arithmetic operations on the same operands —
/// covers ADD, SUB, MUL, DIV, MOD in one scan. Single iteration; the
/// program does no looping.
fn bench_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_arithmetic");
    let container = compile_st(programs::ARITHMETIC);
    bench_run!(
        group,
        BenchmarkId::from_parameter("5ops"),
        &container,
        |_bufs| {}
    );
    group.finish();
}

/// CASE statement state machine — exercises CASE dispatch with four
/// states that advance per scan. Each scan executes one branch's body
/// (five assignments) plus the CASE selector load.
fn bench_case_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_case_state");
    let container = compile_st(programs::CASE_STATE);
    bench_run!(
        group,
        BenchmarkId::from_parameter("4states"),
        &container,
        |_bufs| {}
    );
    group.finish();
}

/// IF/ELSE counter with a saturation reset — minimal state-machine shape
/// representative of typical PLC scan code. Each scan does one comparison
/// plus either an increment (the common path) or a reset (1 in 1001 scans).
fn bench_counter_up(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_counter_up");
    let container = compile_st(programs::COUNTER_UP);
    bench_run!(
        group,
        BenchmarkId::from_parameter("1iter"),
        &container,
        |_bufs| {}
    );
    group.finish();
}

/// Debug-mode scan cost: runs an identical counter loop through the
/// production `run_round` and through the re-entrant `run_round_debug`
/// driver with the zero-cost [`NoopDebugHook`]. This tracks the cost a
/// debugger's "run to next breakpoint" pays while streaming through code at
/// full speed (interactive single-stepping is unaffected by per-scan cost).
///
/// The two arms are NOT expected to match: `run_round` inlines and
/// specialises `execute_with_hook` at its single call site, while
/// `run_round_debug` calls the out-of-line generic copy. The gap here is
/// that driver difference, not a hot-path regression — the production
/// `run_round` path itself is guarded against regression by the other
/// `run_round` benches in this file (compare a branch to a `main` baseline
/// with `--save-baseline` / `--baseline`).
fn bench_debug_scan_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_debug_scan_cost");
    let container = compile_st(programs::COUNTER_LOOP);

    let count = 10_000;
    group.throughput(Throughput::Elements(count as u64));

    // Production path — reuses the shared macro (run_round).
    bench_run!(
        group,
        BenchmarkId::new("run_round", count),
        &container,
        |bufs| {
            bufs.vars[0] = Slot::from_i32(count);
        }
    );

    // Debug driver path — run_round_debug with the zero-cost hook.
    group.bench_with_input(
        BenchmarkId::new("run_round_debug_noop", count),
        &(),
        |b, _| {
            b.iter_batched(
                || {
                    let mut bufs = VmBuffers::from_container(&container);
                    bufs.vars[0] = Slot::from_i32(count);
                    bufs
                },
                |mut bufs| {
                    let mut vm = load_and_start(&container, &mut bufs).unwrap();
                    let mut hook = NoopDebugHook;
                    black_box(vm.run_round_debug(0, &mut hook).unwrap());
                },
                BatchSize::SmallInput,
            );
        },
    );
    group.finish();
}

/// `STRING_TO_<integer>` under the strict policies (reject, trap) — the
/// scan-and-range-check cost of one conversion, per target width. Each
/// program converts a valid literal one hundred times; the widths differ
/// only in the bounds the scanner checks, so the four cases should sit
/// together, and a gap between them is a regression in one width's path.
fn bench_string_to_num(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_string_to_num");
    const REPS: usize = 100;

    for (type_name, literal) in [
        ("SINT", "100"),
        ("INT", "30000"),
        ("DINT", "2000000000"),
        ("UDINT", "4000000000"),
    ] {
        let mut source =
            format!("PROGRAM main\n  VAR s : STRING := '{literal}'; x : {type_name}; END_VAR\n");
        for _ in 0..REPS {
            source.push_str(&format!("  x := STRING_TO_{type_name}(s);\n"));
        }
        source.push_str("END_PROGRAM\n");
        let container = compile_st(&source);

        group.throughput(Throughput::Elements(REPS as u64));
        bench_run!(
            group,
            BenchmarkId::from_parameter(type_name),
            &container,
            |_bufs| {}
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_debug_scan_cost,
    bench_counter_loop,
    bench_arithmetic_i32,
    bench_arithmetic_f64,
    bench_branching,
    bench_for_loop,
    bench_nested_loops,
    bench_narrow_opcodes,
    bench_diverse_opcodes,
    bench_arithmetic,
    bench_case_state,
    bench_counter_up,
    bench_string_to_num,
);
criterion_main!(benches);
