//! Throwaway comparison binary: VM counter loop vs native counter loop.
//!
//! Run under callgrind to get instruction counts:
//!   cargo build --release --example vm_vs_native -p ironplc-benchmarks
//!   valgrind --tool=callgrind --callgrind-out-file=/tmp/vm.cg \
//!     target/release/examples/vm_vs_native vm 10000
//!   valgrind --tool=callgrind --callgrind-out-file=/tmp/native.cg \
//!     target/release/examples/vm_vs_native native 10000
//!   callgrind_annotate /tmp/vm.cg | head -40
//!   callgrind_annotate /tmp/native.cg | head -40
//!
//! Or run directly for wall-clock comparison:
//!   target/release/examples/vm_vs_native compare 100000

use ironplc_benchmarks::compile_st;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{Slot, VmBuffers};
use std::env;
use std::hint::black_box;
use std::time::Instant;

// Sums i from `iterations` down to 1 in a counter-style loop.
// `total` is observable so the compiler can't fold the loop away.
const COUNTER_LOOP: &str = "PROGRAM main
  VAR counter : DINT; total : DINT; END_VAR
  WHILE counter > 0 DO
    total := total + counter;
    counter := counter - 1;
  END_WHILE;
END_PROGRAM";

#[inline(never)]
fn run_native(iterations: i32) -> i32 {
    let mut counter = black_box(iterations);
    let mut total: i32 = black_box(0);
    while black_box(counter) > 0 {
        total = black_box(total.wrapping_add(counter));
        counter = black_box(counter - 1);
    }
    black_box(total)
}

// Pre-built container; we time only execution.
fn run_vm_prebuilt(container: &ironplc_container::Container, iterations: i32) {
    let mut bufs = VmBuffers::from_container(container);
    bufs.vars[0] = Slot::from_i32(iterations);
    bufs.vars[1] = Slot::from_i32(0);
    let mut vm = load_and_start(container, &mut bufs).unwrap();
    vm.run_round(0).unwrap();
    black_box(&bufs);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("compare");
    let iters: i32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);

    match mode {
        "vm" => {
            // For callgrind: hoist compilation, then execute many times.
            let container = compile_st(COUNTER_LOOP);
            for _ in 0..1000 {
                run_vm_prebuilt(&container, iters);
            }
        }
        "native" => {
            let mut acc: i32 = 0;
            for _ in 0..1000 {
                acc = acc.wrapping_add(run_native(iters));
            }
            black_box(acc);
        }
        "compare" => {
            let container = compile_st(COUNTER_LOOP);

            const REPS: u32 = 200;

            for _ in 0..10 {
                black_box(run_native(iters));
                run_vm_prebuilt(&container, iters);
            }

            let t0 = Instant::now();
            for _ in 0..REPS {
                black_box(run_native(iters));
            }
            let native_dt = t0.elapsed() / REPS;

            let t0 = Instant::now();
            for _ in 0..REPS {
                run_vm_prebuilt(&container, iters);
            }
            let vm_dt = t0.elapsed() / REPS;

            let ratio = vm_dt.as_nanos() as f64 / native_dt.as_nanos() as f64;
            let ns_per_iter_vm = vm_dt.as_nanos() as f64 / iters as f64;
            let ns_per_iter_native = native_dt.as_nanos() as f64 / iters as f64;
            println!(
                "iters={iters} native={:?} ({:.2} ns/iter) vm={:?} ({:.2} ns/iter) ratio={:.1}x",
                native_dt, ns_per_iter_native, vm_dt, ns_per_iter_vm, ratio
            );
        }
        other => eprintln!("unknown mode: {other}"),
    }
}
