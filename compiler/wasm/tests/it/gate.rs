//! The differential gate: every program of the corpus on the VM, wasmtime
//! and wasmi.
//!
//! `cargo test -p ironplc-wasm --test it gate -- --ignored --nocapture`
//! runs it and writes `target/wasm-gate/report.md`. Set
//! `IRONPLC_WASM_PROBES` to a directory of `.st` probes to add them.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::corpus::{codegen_programs, repository_programs, st_files, Program};
use ironplc_wasm_runner::Engine;

use crate::harness::{differential_on, Outcome};

/// Cycles run for each program.
const CYCLES: usize = 30;

/// Differences whose cause is known and is not the WebAssembly target: the
/// program, a part of the difference, and the cause.
const EXPLAINED: &[(&str, &str, &str)] = &[
    (
        "codegen/end_to_end_math.rs#3",
        "y is 3 on the VM, 2.9999999999999996",
        "back end: LOG(1000.0) is not correctly rounded",
    ),
    (
        "codegen/end_to_end_math.rs#5",
        "y is 2.718281828459045 on the VM, 2.7182818284590455",
        "back end: EXP(1.0) is not correctly rounded",
    ),
    (
        "probes/implicit_widening.st",
        "r is NaN on the VM",
        "VM: an INT assigned to a REAL keeps its bit pattern (ironplc/ironplc#1812)",
    ),
];

fn explained(id: &str, message: &str) -> Option<&'static str> {
    EXPLAINED
        .iter()
        .find(|(i, m, _)| *i == id && message.contains(m))
        .map(|(_, _, cause)| *cause)
}

fn corpus() -> Vec<Program> {
    let mut all = codegen_programs();
    all.extend(repository_programs());
    if let Some(dir) = std::env::var("IRONPLC_WASM_PROBES")
        .ok()
        .filter(|d| !d.is_empty())
    {
        all.extend(st_files(Path::new(&dir), "probes"));
    }
    all
}

fn report_path() -> PathBuf {
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"));
    target.join("wasm-gate")
}

#[test]
#[ignore = "runs the whole corpus; see the module documentation"]
fn gate_when_corpus_runs_on_every_engine_then_no_unexplained_difference() {
    run_gate(CYCLES, &Engine::ALL, true);
}

/// The corpus on wasmi only and for a few cycles: fast enough for every
/// test run.
#[test]
fn gate_when_corpus_runs_on_wasmi_then_no_unexplained_difference() {
    run_gate(5, &[Engine::Wasmi], false);
}

fn run_gate(cycles: usize, engines: &[Engine], report: bool) {
    let mut rows = String::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unsupported: BTreeMap<String, usize> = BTreeMap::new();
    let mut differences = vec![];
    let programs = corpus();
    for (p, outcome) in programs
        .iter()
        .zip(outcomes(&programs, cycles, engines, report))
    {
        let (kind, detail) = match &outcome {
            Outcome::NotAccepted => ("not accepted by IronPLC", String::new()),
            Outcome::NoBytecode(m) => ("no bytecode", m.clone()),
            Outcome::NoWasm(m) => {
                *unsupported.entry(m.clone()).or_default() += 1;
                ("not supported by the WebAssembly target", m.clone())
            }
            Outcome::Same { cycles, variables } => {
                ("same", format!("{variables} variables, {cycles} cycles"))
            }
            Outcome::Differs(m) => match explained(&p.id, m) {
                Some(cause) => ("differs, explained", format!("{m} ({cause})")),
                None => {
                    differences.push((p.id.clone(), m.clone()));
                    ("differs", m.clone())
                }
            },
        };
        *counts.entry(kind).or_default() += 1;
        let _ = writeln!(
            rows,
            "| {} | {kind} | {} |",
            p.id,
            detail.replace('|', "\\|")
        );
    }
    let mut text = String::from("# WebAssembly gate\n\n| Outcome | Programs |\n|---|---|\n");
    for (k, n) in &counts {
        let _ = writeln!(text, "| {k} | {n} |");
    }
    text.push_str("\n## Constructs not supported\n\n| Diagnostic | Programs |\n|---|---|\n");
    let mut by_count: Vec<_> = unsupported.into_iter().collect();
    by_count.sort_by_key(|e| std::cmp::Reverse(e.1));
    for (m, n) in by_count {
        let _ = writeln!(text, "| {} | {n} |", m.replace('|', "\\|"));
    }
    text.push_str("\n## Programs\n\n| Program | Outcome | Detail |\n|---|---|---|\n");
    text.push_str(&rows);
    let dir = report_path();
    if report {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("report.md"), &text);
        println!("{}", text.split("\n## Programs").next().unwrap_or_default());
    }
    for (id, m) in &differences {
        println!("DIFF {id}: {m}");
    }
    assert!(
        differences.is_empty(),
        "{} programs differ, see {}",
        differences.len(),
        dir.join("report.md").display()
    );
}

/// The outcome of every program, computed on all the cores.
fn outcomes(programs: &[Program], cycles: usize, engines: &[Engine], report: bool) -> Vec<Outcome> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Outcome>>> = Mutex::new(programs.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let worker = || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(p) = programs.get(i) else {
                    break;
                };
                if report {
                    eprintln!("gate: {}", p.id);
                }
                let outcome = differential_on(&p.source, cycles, engines);
                if let Ok(mut r) = results.lock() {
                    r[i] = Some(outcome);
                }
            };
            // The analyzer recurses over the syntax tree: a large stack as
            // on the main thread.
            let _ = std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(scope, worker);
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|o| o.unwrap_or(Outcome::NotAccepted))
        .collect()
}
