//! End-to-end profiling test for a small FOR loop.
//!
//! Compiles the FOR-loop reference programs through the full pipeline,
//! runs one VM scan with the instruction profiler enabled, and prints a
//! sorted opcode histogram. The data drives priority decisions for
//! dispatch reorganization (specs/design/vm-performance.md §4b) and the
//! superinstruction discussion (§4).
//!
//! Run with:
//!   cargo test -p ironplc-benchmarks --features profiling \
//!     -- --nocapture --test-threads=1
//!
//! `--test-threads=1` keeps the per-test histograms from interleaving.
//! Without the `profiling` feature this file compiles to nothing.
#![cfg(feature = "profiling")]

use ironplc_benchmarks::compile_st;
use ironplc_container::opcode;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{InstructionProfile, VmBuffers};

const FOR_LOOP_INT: &str = "PROGRAM for_loop_int
  VAR
    i : INT;
    total : INT := 0;
  END_VAR
  total := 0;
  FOR i := 1 TO 100 DO
    total := total + i;
  END_FOR;
END_PROGRAM";

const FOR_LOOP_DINT: &str = "PROGRAM for_loop_dint
  VAR
    i : DINT;
    total : DINT := 0;
  END_VAR
  total := 0;
  FOR i := 1 TO 100 DO
    total := total + i;
  END_FOR;
END_PROGRAM";

fn run_one_scan(source: &str) -> InstructionProfile {
    let container = compile_st(source);
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    vm.run_round(0).unwrap();
    vm.stop().profile().clone()
}

fn print_histogram(label: &str, profile: &InstructionProfile) {
    let mut entries: Vec<(usize, u64)> = profile
        .counts()
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(i, c)| (i, *c))
        .collect();
    entries.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    let total: u64 = entries.iter().map(|(_, c)| *c).sum();
    eprintln!();
    eprintln!("=== {label} — opcode histogram (total = {total}) ===");
    eprintln!("  {:>4}  {:>10}  {:>5}  name", "op", "count", "%");
    for (op, count) in &entries {
        let pct = (*count as f64) * 100.0 / (total.max(1) as f64);
        eprintln!(
            "  0x{:02X}  {:>10}  {:>4.1}%  {}",
            op,
            count,
            pct,
            opcode_name(*op as u8)
        );
    }
}

/// Best-effort opcode-byte → name lookup for histogram readability.
/// Unlisted opcodes print as `(other)`; correctness of the test does not
/// depend on this table being exhaustive.
fn opcode_name(op: u8) -> &'static str {
    match op {
        opcode::LOAD_CONST_I32 => "LOAD_CONST_I32",
        opcode::LOAD_CONST_I64 => "LOAD_CONST_I64",
        opcode::LOAD_CONST_F32 => "LOAD_CONST_F32",
        opcode::LOAD_CONST_F64 => "LOAD_CONST_F64",
        opcode::LOAD_TRUE => "LOAD_TRUE",
        opcode::LOAD_FALSE => "LOAD_FALSE",
        opcode::LOAD_VAR_I32 => "LOAD_VAR_I32",
        opcode::LOAD_VAR_I64 => "LOAD_VAR_I64",
        opcode::LOAD_VAR_F32 => "LOAD_VAR_F32",
        opcode::LOAD_VAR_F64 => "LOAD_VAR_F64",
        opcode::STORE_VAR_I32 => "STORE_VAR_I32",
        opcode::STORE_VAR_I64 => "STORE_VAR_I64",
        opcode::STORE_VAR_F32 => "STORE_VAR_F32",
        opcode::STORE_VAR_F64 => "STORE_VAR_F64",
        opcode::TRUNC_I8 => "TRUNC_I8",
        opcode::TRUNC_U8 => "TRUNC_U8",
        opcode::TRUNC_I16 => "TRUNC_I16",
        opcode::TRUNC_U16 => "TRUNC_U16",
        opcode::ADD_I32 => "ADD_I32",
        opcode::SUB_I32 => "SUB_I32",
        opcode::MUL_I32 => "MUL_I32",
        opcode::DIV_I32 => "DIV_I32",
        opcode::MOD_I32 => "MOD_I32",
        opcode::NEG_I32 => "NEG_I32",
        opcode::ADD_I64 => "ADD_I64",
        opcode::EQ_I32 => "EQ_I32",
        opcode::NE_I32 => "NE_I32",
        opcode::LT_I32 => "LT_I32",
        opcode::LE_I32 => "LE_I32",
        opcode::GT_I32 => "GT_I32",
        opcode::GE_I32 => "GE_I32",
        opcode::JMP => "JMP",
        opcode::JMP_IF_NOT => "JMP_IF_NOT",
        opcode::CALL => "CALL",
        opcode::RET => "RET",
        opcode::RET_VOID => "RET_VOID",
        opcode::POP => "POP",
        opcode::DUP => "DUP",
        opcode::SWAP => "SWAP",
        _ => "(other)",
    }
}

#[test]
fn profile_for_loop_int_then_expected_hot_opcodes_dominate() {
    let profile = run_one_scan(FOR_LOOP_INT);
    print_histogram("FOR_LOOP_INT (i, total : INT, 1 TO 100)", &profile);

    // Loose lower bounds (~50% of naive expectation). The diagnostic
    // value of this test is the printed histogram; the asserts only
    // catch the case where the loop didn't run at all.
    assert!(profile.count(opcode::ADD_I32) >= 100);
    assert!(profile.count(opcode::LOAD_VAR_I32) >= 200);
    assert!(profile.count(opcode::STORE_VAR_I32) >= 100);
    assert!(profile.count(opcode::GT_I32) >= 50);

    // The two FOR-loop-internal TRUNC_I16 sites (init and per-iteration
    // increment) are now elided because `1 TO 100` keeps every value of `i`
    // safely inside INT's range. See
    // specs/plans/2026-04-30-elide-for-loop-trunc.md. The 102 TRUNC_I16
    // ops still observed are 100 from the body's `total := total + i`
    // assignment plus 2 from initializing `total : INT` — narrow stores
    // that this slice of interval analysis does not address.
    assert_eq!(profile.count(opcode::TRUNC_I16), 102);
}

#[test]
fn profile_for_loop_dint_then_no_truncation_opcodes() {
    let profile = run_one_scan(FOR_LOOP_DINT);
    print_histogram("FOR_LOOP_DINT (i, total : DINT, 1 TO 100)", &profile);

    // DINT fills a native 32-bit slot — no narrow-store truncation.
    // Diff this histogram against the INT one to see the truncation cost.
    assert_eq!(profile.count(opcode::TRUNC_I8), 0);
    assert_eq!(profile.count(opcode::TRUNC_U8), 0);
    assert_eq!(profile.count(opcode::TRUNC_I16), 0);
    assert_eq!(profile.count(opcode::TRUNC_U16), 0);

    // Sanity: the loop still ran.
    assert!(profile.count(opcode::ADD_I32) >= 100);
}
