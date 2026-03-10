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
    let container = bench_helpers::counter_loop();

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
        let (container, _src) = bench_helpers::arithmetic_i32(reps);
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
        let (container, _src) = bench_helpers::arithmetic_f64(reps);
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
        let (container, _src) = bench_helpers::branching(branches);
        bench_run!(
            group,
            BenchmarkId::from_parameter(branches),
            &container,
            |bufs| {
                bufs.vars[0] = Slot::from_i32((branches - 1) as i32);
            }
        );
    }
    group.finish();
}

/// FOR loop summing 1..limit — structured loop overhead.
fn bench_for_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("st_for_loop");
    let container = bench_helpers::for_loop_sum();

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
    let container = bench_helpers::nested_loops();

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
