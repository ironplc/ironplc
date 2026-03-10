//! Performance benchmarks using compiled IEC 61131-3 Structured Text.
//!
//! These benchmarks compile real ST source through the full pipeline
//! (parser → analyzer → codegen) and measure VM execution time. They
//! complement the hand-crafted bytecode benchmarks in `ironplc-vm` by
//! exercising realistic code paths.
//!
//! Run with: `cargo bench --package ironplc-benchmarks`

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{Slot, VmBuffers};

/// Compiles an IEC 61131-3 source string through the full pipeline:
/// parse → analyze (all semantic rules) → codegen.
fn compile_st(source: &str) -> Container {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, context) = ironplc_analyzer::stages::analyze(&[&library]).unwrap();
    assert!(
        !context.has_diagnostics(),
        "Benchmark source has semantic diagnostics"
    );
    compile(&analyzed).unwrap()
}

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
    let container = compile_st(
        "PROGRAM main
  VAR counter : DINT; END_VAR
  WHILE counter > 0 DO
    counter := counter - 1;
  END_WHILE;
END_PROGRAM",
    );

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
        let mut source = String::from("PROGRAM main\n  VAR x : DINT; END_VAR\n");
        for _ in 0..reps {
            source.push_str("  x := (x + 7 - 3) * 2;\n");
        }
        source.push_str("END_PROGRAM\n");
        let container = compile_st(&source);

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
        let mut source = String::from("PROGRAM main\n  VAR x : LREAL; END_VAR\n");
        for _ in 0..reps {
            source.push_str("  x := (x + 7.0 - 3.0) * 2.0;\n");
        }
        source.push_str("END_PROGRAM\n");
        let container = compile_st(&source);

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
        let mut source = String::from("PROGRAM main\n  VAR sel : DINT; result : DINT; END_VAR\n");
        for i in 0..branches {
            if i == 0 {
                source.push_str(&format!("  IF sel = {} THEN\n    result := {};\n", i, i));
            } else {
                source.push_str(&format!("  ELSIF sel = {} THEN\n    result := {};\n", i, i));
            }
        }
        source.push_str("  END_IF;\nEND_PROGRAM\n");
        let container = compile_st(&source);

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
    let container = compile_st(
        "PROGRAM main
  VAR i : DINT; sum : DINT; limit : DINT; END_VAR
  sum := 0;
  FOR i := 1 TO limit DO
    sum := sum + i;
  END_FOR;
END_PROGRAM",
    );

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
    let container = compile_st(
        "PROGRAM main
  VAR i : DINT; j : DINT; acc : DINT;
      outer : DINT; inner : DINT; END_VAR
  acc := 0;
  FOR i := 1 TO outer DO
    FOR j := 1 TO inner DO
      acc := acc + i * j;
    END_FOR;
  END_FOR;
END_PROGRAM",
    );

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
