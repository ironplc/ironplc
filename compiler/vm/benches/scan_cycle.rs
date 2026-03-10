//! Performance benchmarks for VM scan cycle execution.
//!
//! Each benchmark group targets a specific overhead source identified in
//! `specs/design/vm-performance.md`. Run with `cargo bench --package ironplc-vm`
//! or `just bench` from the compiler directory.

mod bench_helpers;

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{Slot, VmBuffers};

/// Benchmark Group 1: Counter loop (dispatch overhead baseline).
///
/// A tight WHILE loop decrementing a counter. Dominated by dispatch overhead
/// since per-iteration computation is trivial (one subtract). Parametric over
/// iteration count to produce a throughput metric.
fn bench_counter_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("counter_loop");
    for count in [100, 1000, 10_000] {
        let container = bench_helpers::counter_loop_container();
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut bufs = VmBuffers::from_container(&container);
                    bufs.vars[0] = Slot::from_i32(count);
                    bufs
                },
                |mut bufs| {
                    let mut vm = load_and_start(&container, &mut bufs).unwrap();
                    black_box(vm.run_round(0).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark Group 2: Straight-line i32 arithmetic (per-instruction cost).
///
/// No branches — chains of ADD, SUB, MUL with LOAD/STORE. Isolates the
/// per-instruction safety check overhead (stack bounds, variable bounds,
/// constant pool bounds).
fn bench_arithmetic_i32(c: &mut Criterion) {
    let mut group = c.benchmark_group("arithmetic_i32");
    for reps in [10, 100, 1000] {
        let container = bench_helpers::arithmetic_i32_container(reps);
        // Each repetition is 7 dispatched instructions.
        group.throughput(Throughput::Elements((reps * 7) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(reps), &reps, |b, _| {
            b.iter_batched(
                || {
                    let mut bufs = VmBuffers::from_container(&container);
                    bufs.vars[0] = Slot::from_i32(1);
                    bufs
                },
                |mut bufs| {
                    let mut vm = load_and_start(&container, &mut bufs).unwrap();
                    black_box(vm.run_round(0).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark Group 3: Straight-line f64 arithmetic.
///
/// Same pattern as i32 but with floating-point. Useful for comparing dispatch
/// cost across type families (Slot is already u64 so the dispatch path should
/// be similar).
fn bench_arithmetic_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("arithmetic_f64");
    for reps in [10, 100, 1000] {
        let container = bench_helpers::arithmetic_f64_container(reps);
        group.throughput(Throughput::Elements((reps * 7) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(reps), &reps, |b, _| {
            b.iter_batched(
                || {
                    let mut bufs = VmBuffers::from_container(&container);
                    bufs.vars[0] = Slot::from_f64(1.0);
                    bufs
                },
                |mut bufs| {
                    let mut vm = load_and_start(&container, &mut bufs).unwrap();
                    black_box(vm.run_round(0).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark Group 4: String assignment (copy overhead).
///
/// Loads a string constant and stores it to a variable, repeated many times.
/// Parametric over string length to show byte-by-byte copy scaling.
/// Targets optimization item 2 (copy_from_slice).
fn bench_string_assign(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_assign");
    for str_len in [10, 80, 254] {
        let container = bench_helpers::string_assign_container(str_len, 100);
        group.throughput(Throughput::Bytes((str_len * 100) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(str_len), &str_len, |b, _| {
            b.iter_batched(
                || VmBuffers::from_container(&container),
                |mut bufs| {
                    let mut vm = load_and_start(&container, &mut bufs).unwrap();
                    black_box(vm.run_round(0).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark Group 5: IF-ELSIF branching chain.
///
/// An IF-ELSIF chain where var[0] matches the last branch (worst case for
/// sequential comparison). Exercises dispatch + comparison + conditional jump.
/// Targets optimization items 4 (superinstructions) and 11 (fused compare-branch).
fn bench_branching(c: &mut Criterion) {
    let mut group = c.benchmark_group("branching");
    for branches in [5, 20, 50] {
        let container = bench_helpers::branching_container(branches);
        group.bench_with_input(
            BenchmarkId::from_parameter(branches),
            &branches,
            |b, &branches| {
                b.iter_batched(
                    || {
                        let mut bufs = VmBuffers::from_container(&container);
                        // Set var[0] to last branch index (worst case — must check all branches).
                        bufs.vars[0] = Slot::from_i32((branches - 1) as i32);
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
    bench_string_assign,
    bench_branching,
);
criterion_main!(benches);
