//! Performance benchmarks using compiled IEC 61131-3 Structured Text.
//!
//! These benchmarks compile real ST source through the full pipeline
//! (parser → analyzer → codegen) and measure VM execution time. They
//! complement the hand-crafted bytecode benchmarks in `ironplc-vm` by
//! exercising realistic code paths.
//!
//! Run with: `cargo bench --package ironplc-benchmarks`

mod bench_helpers;

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{Slot, VmBuffers};

/// Defines a benchmark that iterates over parameter values, compiling a
/// container for each and measuring VM execution.
///
/// Arguments:
///   $name       — function name
///   $group      — benchmark group string
///   $params     — array of parameter values to iterate
///   $param:ident — binding for the current parameter value
///   container   — expression producing a `Container` (may reference $param)
///   throughput  — expression producing `Throughput` (may reference $param), or omitted
///   setup       — closure body `|bufs, param|` to initialize VmBuffers before each run
macro_rules! bench_fn {
    (
        $name:ident, $group:literal,
        $params:expr, $param:ident,
        container: $container:expr,
        throughput: $throughput:expr,
        setup: |$bufs:ident| $setup:block
    ) => {
        fn $name(c: &mut Criterion) {
            let mut group = c.benchmark_group($group);
            for $param in $params {
                let container = $container;
                group.throughput($throughput);
                group.bench_with_input(
                    BenchmarkId::from_parameter($param),
                    &$param,
                    |b, &$param| {
                        b.iter_batched(
                            || {
                                let mut $bufs = VmBuffers::from_container(&container);
                                $setup
                                $bufs
                            },
                            |mut bufs| {
                                let mut vm = load_and_start(&container, &mut bufs).unwrap();
                                black_box(vm.run_round(0).unwrap());
                            },
                            BatchSize::SmallInput,
                        );
                    },
                );
            }
            group.finish();
        }
    };
    // Variant without throughput.
    (
        $name:ident, $group:literal,
        $params:expr, $param:ident,
        container: $container:expr,
        setup: |$bufs:ident| $setup:block
    ) => {
        fn $name(c: &mut Criterion) {
            let mut group = c.benchmark_group($group);
            for $param in $params {
                let container = $container;
                group.bench_with_input(
                    BenchmarkId::from_parameter($param),
                    &$param,
                    |b, &$param| {
                        b.iter_batched(
                            || {
                                let mut $bufs = VmBuffers::from_container(&container);
                                $setup
                                $bufs
                            },
                            |mut bufs| {
                                let mut vm = load_and_start(&container, &mut bufs).unwrap();
                                black_box(vm.run_round(0).unwrap());
                            },
                            BatchSize::SmallInput,
                        );
                    },
                );
            }
            group.finish();
        }
    };
}

/// WHILE loop decrementing a counter — dispatch overhead baseline.
bench_fn!(
    bench_counter_loop, "st_counter_loop",
    [100, 1000, 10_000], count,
    container: bench_helpers::counter_loop(),
    throughput: Throughput::Elements(count as u64),
    setup: |bufs| { bufs.vars[0] = Slot::from_i32(count); }
);

/// Straight-line DINT arithmetic — per-instruction cost.
bench_fn!(
    bench_arithmetic_i32, "st_arithmetic_i32",
    [10, 100, 1000], reps,
    container: bench_helpers::arithmetic_i32(reps).0,
    throughput: Throughput::Elements(reps as u64),
    setup: |bufs| { bufs.vars[0] = Slot::from_i32(1); }
);

/// Straight-line LREAL arithmetic.
bench_fn!(
    bench_arithmetic_f64, "st_arithmetic_f64",
    [10, 100, 1000], reps,
    container: bench_helpers::arithmetic_f64(reps).0,
    throughput: Throughput::Elements(reps as u64),
    setup: |bufs| { bufs.vars[0] = Slot::from_f64(1.0); }
);

/// IF-ELSIF branching chain — worst-case sequential comparison.
bench_fn!(
    bench_branching, "st_branching",
    [5, 20, 50], branches,
    container: bench_helpers::branching(branches).0,
    setup: |bufs| { bufs.vars[0] = Slot::from_i32((branches - 1) as i32); }
);

/// FOR loop summing 1..limit — structured loop overhead.
bench_fn!(
    bench_for_loop, "st_for_loop",
    [100, 1000, 10_000], limit,
    container: bench_helpers::for_loop_sum(),
    throughput: Throughput::Elements(limit as u64),
    setup: |bufs| { bufs.vars[2] = Slot::from_i32(limit); }
);

/// Nested FOR loops — exercises loop overhead at scale.
fn bench_nested_loops(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_nested_loops");
    let container = bench_helpers::nested_loops();

    for (outer, inner) in [(10, 10), (10, 100), (100, 100)] {
        let label = format!("{}x{}", outer, inner);
        group.throughput(Throughput::Elements((outer * inner) as u64));
        group.bench_with_input(
            BenchmarkId::new("iters", &label),
            &(outer, inner),
            |b, &(outer, inner)| {
                b.iter_batched(
                    || {
                        let mut bufs = VmBuffers::from_container(&container);
                        bufs.vars[3] = Slot::from_i32(outer);
                        bufs.vars[4] = Slot::from_i32(inner);
                        bufs
                    },
                    |mut bufs| {
                        let mut vm = load_and_start(&container, &mut bufs).unwrap();
                        black_box(vm.run_round(0).unwrap());
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_counter_loop,
    bench_arithmetic_i32,
    bench_arithmetic_f64,
    bench_branching,
    bench_for_loop,
    bench_nested_loops,
);
criterion_main!(benches);
