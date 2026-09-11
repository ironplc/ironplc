//! Compiles every benchmark program once through the full pipeline.
//!
//! `benches/st_benchmark.rs` compiles all of its programs when the Criterion
//! groups are registered, so an analyzer change that rejects any one of them
//! panics the whole bench binary before a single benchmark runs. Nothing in
//! CI runs the benchmarks, so this test is what turns that into a failing
//! test instead.

use ironplc_benchmarks::compile_st;
use ironplc_benchmarks::programs;

#[test]
fn compile_st_when_each_benchmark_program_then_compiles_without_diagnostics() {
    for (name, source) in programs::all() {
        eprintln!("compiling benchmark program: {name}");
        compile_st(&source);
    }
}
