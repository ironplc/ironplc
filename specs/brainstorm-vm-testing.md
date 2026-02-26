# Brainstorm: VM Integration & End-to-End Testing Strategy

**Status:** Draft for discussion
**Date:** 2026-02-26

## Problem Statement

IronPLC is gaining a virtual machine (`ironplcvm`) and codegen pipeline, but the testing strategy hasn't kept pace. The current gaps:

1. **No E2E test on Linux or macOS** - the only E2E smoke test is Windows-only (installs VS Code + compiler, opens a file, checks a log file exists)
2. **No integration test of the VM in CI** - the VM has unit tests and a hand-assembled steel-thread test, but nothing that exercises the full pipeline (`.st` source → parse → codegen → container → VM execution → verify results) in CI as a first-class gate
3. **No performance testing** - no benchmarks for compilation speed, VM execution throughput, or scan cycle timing
4. **The existing Windows E2E test only runs during weekly deployment** - not on every commit or PR

Meanwhile, CI on every commit is already a velocity concern. Adding more tests has to be balanced against cycle time.

## Current State Inventory

### What exists today

| Layer | Tests | Where they run |
|-------|-------|---------------|
| Parser unit tests | Extensive | Every PR/push (`cargo test`) |
| Analyzer unit tests | Extensive | Every PR/push |
| Codegen unit tests | 9 tests in `compile.rs` | Every PR/push |
| Codegen E2E tests | 8 tests in `codegen/tests/end_to_end.rs` — parse ST source → compile → VM → assert variables | Every PR/push |
| VM unit tests | `vm.rs` tests (6 tests), `scheduler.rs` tests (8 tests), `stack`, `value`, `variable_table` | Every PR/push |
| VM steel-thread integration | `vm/tests/steel_thread.rs` — hand-assembled bytecode → serialize → deserialize → VM → assert | Every PR/push |
| VM CLI tests | `vm/tests/cli.rs` — invoke `ironplcvm` binary with container files | Every PR/push |
| Container format tests | Serialization/deserialization round-trip | Every PR/push |
| Windows E2E smoke | Downloads release artifacts, installs compiler + VS Code, opens `.st` file, checks log file created | Weekly deployment only |

### What the VM currently supports

- Opcodes: `LOAD_CONST_I32`, `LOAD_VAR_I32`, `STORE_VAR_I32`, `ADD_I32`, `RET_VOID`
- Task types: Freewheeling, Cyclic (Event is stubbed)
- Scheduler: priority-based with watchdog detection
- State machine: Vm → VmReady → VmRunning → VmStopped / VmFaulted
- CLI: `ironplcvm run <file> [--scans N] [--dump-vars <path>]`

### CI timing and structure

- **On every PR/push:** Builds and tests on 5 platform targets (Win x86_64, Win ARM64, Linux x86_64 musl, macOS x86_64, macOS ARM64). Each runs `just ci` = compile + coverage (85% threshold) + lint (clippy + fmt). No explicit timeouts set.
- **Weekly deployment (Monday 19:00 UTC):** Full pipeline: version bump → build all platforms → prerelease → Windows E2E smoke → publish website → publish to Marketplace + Homebrew → cleanup.
- **Weekly dependency update (Sunday 19:00 UTC):** Updates deps, builds, auto-merges if passing.

---

## Brainstorm: Testing Tiers

The core idea is to tier tests by cost and frequency.

### Tier 1: On Every PR/Push (must be fast, < 15 minutes total)

These already run. The question is what we can add without blowing the time budget.

**Idea 1.1: Expand codegen E2E tests with a conformance suite**

The `codegen/tests/end_to_end.rs` pattern is very effective: write ST source → compile → run 1 scan in VM → assert variable values. These tests are pure Rust, run in-process, are fast (no I/O, no subprocess), and test the complete pipeline from source text to execution result.

This is the single highest-leverage investment. Each new language feature gets a test like:

```rust
#[test]
fn end_to_end_when_if_then_else_then_correct_branch_taken() {
    let source = "
PROGRAM main
  VAR x : INT; END_VAR
  IF TRUE THEN x := 1; ELSE x := 2; END_IF;
END_PROGRAM
";
    let vm = parse_and_run(source);
    assert_eq!(vm.read_variable(0).unwrap(), 1);
}
```

- **Cost:** Milliseconds per test. Hundreds of these can run in seconds.
- **What it validates:** Parser + codegen + container format + VM execution all agree.
- **What it cannot validate:** CLI behavior, installation, packaging, cross-platform native behavior, real-time scheduling.
- **Can be automated:** Yes, fully. This is a standard `cargo test` pattern.
- **Recommendation:** Make this the primary integration test mechanism. Create a `tests/conformance/` directory or expand `codegen/tests/end_to_end.rs` into a proper conformance suite organized by language feature.

**Idea 1.2: Golden file tests for the VM CLI**

The `vm/tests/cli.rs` already does this with `--dump-vars`. Expand to a pattern:
- Directory of `.st` source files, each with a companion `.expected` file containing expected variable dump
- A test harness that compiles each `.st` → `.iplc`, runs `ironplcvm run --scans 1 --dump-vars`, compares output
- Could also snapshot the `.iplc` binary to detect unintentional bytecode format changes

- **Cost:** Low (subprocess invocation, but fast since each program is tiny)
- **What it validates:** Full binary-level behavior including CLI argument handling, file I/O, container format serialization
- **Can be automated:** Yes, fully.

**Idea 1.3: Property-based / fuzz testing for the VM**

Use `proptest` or `cargo-fuzz` to generate random bytecode sequences and verify the VM never panics (only returns `Err(Trap)`).

- **Cost:** Can be time-bounded (e.g., run for 5 seconds in CI, longer locally)
- **What it validates:** VM robustness against malformed input
- **Can be automated:** Yes. The time-bounded variant fits in CI; longer runs can be nightly.
- **Limitation:** Finding *functional* bugs requires smarter generation (valid programs with known expected outputs). Random bytecode mostly tests error handling.

### Tier 2: Nightly or Weekly (can be slower, 30-60 minutes)

**Idea 2.1: Cross-platform E2E smoke test**

The current Windows smoke test installs VS Code + compiler and verifies the language server starts. Extend this pattern to Linux and macOS, but with a simpler scope since VS Code installation on headless Linux/Mac in CI is painful.

A simpler cross-platform E2E:
1. Download the platform-specific release artifact (tar.gz or exe)
2. Extract/install
3. Compile a `.st` file with `ironplcc`
4. Run the output with `ironplcvm run --scans 1 --dump-vars vars.txt`
5. Assert `vars.txt` contains expected values

This tests the packaging and binary compatibility without needing VS Code.

- **Cost:** Moderate (download artifacts, subprocess calls). 2-5 minutes per platform.
- **What it validates:** Packaging, binary startup, basic correctness on each OS.
- **Can be automated:** Yes. Add `partial_integration_test_unix.yaml`.
- **When to run:** Weekly deployment pipeline (already runs Windows E2E there). Could also run nightly as a separate workflow.

**Idea 2.2: Performance regression testing**

Use `criterion` benchmarks to detect performance regressions.

Benchmarks to consider:
- **Compilation throughput:** Time to compile a reference `.st` program (measures parser + analyzer + codegen)
- **VM execution throughput:** Instructions per second for a tight loop
- **Scan cycle time:** Time for one scheduling round with N tasks
- **Container serialization:** Time to serialize/deserialize containers of varying sizes

Two approaches for CI:

**Approach A: Threshold-based (simpler)**
- Run benchmarks in CI
- Fail if any benchmark exceeds a hard-coded threshold (e.g., "steel thread compilation must take < 50ms")
- Pro: Simple to implement, clear pass/fail
- Con: Thresholds are fragile across different CI runner hardware; needs tuning

**Approach B: Relative comparison (more robust)**
- Use `criterion` with `--save-baseline` on the main branch
- On PRs, compare against baseline and fail if regression > X%
- Pro: Hardware-independent (compares relative to same machine)
- Con: Requires baseline persistence across runs; CI runners may vary

**Approach C: Track and alert, don't gate (pragmatic)**
- Run benchmarks, upload results as artifacts
- Use GitHub Actions to post benchmark results as PR comments (e.g., `github-action-benchmark`)
- Human reviews trends; no automatic gating
- Pro: No false positives; builds understanding of performance over time
- Con: Requires human attention

- **Recommendation:** Start with Approach C (track and alert). Graduate to Approach A once you have stable numbers and know what thresholds make sense.
- **Can be automated:** Yes. `criterion` + `github-action-benchmark` is a well-trodden path.

**Idea 2.3: Multi-scan functional tests**

Some 61131-3 behaviors only manifest over multiple scans:
- Counters incrementing each scan
- Timers tracking elapsed time
- State machines transitioning
- Cyclic task scheduling at correct intervals

These tests run the VM for N scans (e.g., 100 or 1000) and assert on final variable state or intermediate states.

- **Cost:** Low to moderate depending on scan count
- **What it validates:** Temporal behavior, scheduler correctness, accumulated state
- **Can be automated:** Yes, using the same `parse_and_run` pattern but with multiple `run_round()` calls.
- **When to run:** These are fast enough for Tier 1 if N is small (10-100 scans). For larger N or timing-sensitive tests, Tier 2.

### Tier 3: Pre-release Gate (during deployment pipeline)

**Idea 3.1: Expanded platform smoke tests**

In addition to the existing Windows VS Code smoke test, add:
- **Linux:** Download tar.gz, compile + run a program, verify output
- **macOS:** Same pattern
- **Windows ARM64:** If ARM64 runners become available

These fit naturally into the deployment pipeline between "publish-prerelease" and "publish-release."

**Idea 3.2: IEC 61131-3 conformance test suite**

Build a curated set of `.st` programs that represent a "conformance matrix" of IEC 61131-3 features:
- Each program has an expected output (variable dump or exit code)
- Run all programs through the full pipeline on each platform
- Track which features pass (conformance percentage)
- Gate releases on "no regressions from last release"

This becomes increasingly valuable as language coverage grows. It serves both as a test and as documentation of what's implemented.

- **Cost:** Moderate (depends on suite size, but individual programs are fast)
- **Can be automated:** Yes. The test runner could be a simple script or a custom Rust test harness.
- **When to run:** Deployment pipeline, and optionally nightly.

### Tier 4: Manual / Ad-hoc (cannot or should not be automated)

**Idea 4.1: Embedded target testing**

Testing on actual embedded hardware (ARM Cortex-M, RISC-V, etc.) is hard to automate in standard CI. Options:
- **QEMU emulation:** Run the VM in QEMU for ARM/RISC-V targets. Can be automated but is slow and requires cross-compilation setup.
- **Self-hosted runners with real hardware:** Raspberry Pi or similar connected to GitHub Actions. High maintenance burden for a solo developer.
- **Defer until needed:** The VM currently has no `no_std` support and depends on `std` features (threads, filesystem, timing). Embedded testing is premature until `no_std` support exists.

- **Recommendation:** Defer embedded hardware testing. When `no_std` support is added, start with QEMU-based tests in a nightly workflow.

**Idea 4.2: Real-time performance validation**

Verifying that scan cycles meet hard real-time deadlines requires:
- Deterministic hardware (not CI VMs)
- Real-time OS or at least isolated CPU cores
- Statistical analysis over thousands of cycles

This cannot be meaningfully automated in cloud CI. It requires dedicated hardware.

- **Recommendation:** Defer. Use the `criterion` benchmarks from Idea 2.2 to catch gross regressions. Accept that real-time guarantees need dedicated hardware testing.

---

## Implementation Priority

Given you're a solo developer, here's a suggested order of investment:

### Phase 1: Highest leverage, lowest effort

1. **Expand `codegen/tests/end_to_end.rs` into a conformance suite** (Idea 1.1)
   - This is the single best investment. Every new opcode/feature gets an E2E test.
   - Runs on every commit, catches regressions immediately, near-zero additional CI time.
   - The pattern already exists — just add more test cases.

2. **Add `--scans` and `--dump-vars` based golden tests** (Idea 1.2)
   - Small expansion of existing `cli.rs` test pattern.
   - Catches binary/CLI regressions.

### Phase 2: Fill platform gaps

3. **Cross-platform CLI smoke test in deployment** (Idea 2.1 / 3.1)
   - Add a Linux and macOS step to the deployment pipeline.
   - Much simpler than the Windows VS Code smoke test — just compile + run + check output.
   - Validates packaging on all platforms before release.

### Phase 3: Performance visibility

4. **Add `criterion` benchmarks with tracking** (Idea 2.2, Approach C)
   - Create a `benches/` directory with key benchmarks.
   - Run in nightly/weekly workflow, track results over time.
   - No gating initially — just visibility.

### Phase 4: Deeper coverage

5. **Multi-scan behavioral tests** (Idea 2.3)
6. **Conformance matrix** (Idea 3.2)
7. **Fuzz testing** (Idea 1.3)

### Deferred

8. Embedded/QEMU testing (when `no_std` support exists)
9. Real-time performance validation (when hardware available)

---

## Structural Suggestions

### Test organization

```
compiler/
  codegen/
    tests/
      end_to_end.rs          # Existing: source → VM → assert (expand this)
      conformance/            # Future: organized by feature
        assignment.rs
        arithmetic.rs
        control_flow.rs
        ...
  vm/
    tests/
      cli.rs                  # Existing: binary invocation tests
      steel_thread.rs         # Existing: bytecode round-trip
    benches/                  # Future: criterion benchmarks
      vm_throughput.rs
      scan_cycle.rs
```

### New workflow: nightly or periodic

```yaml
# .github/workflows/nightly.yaml
on:
  schedule:
    - cron: '0 3 * * *'   # 3 AM UTC daily, or adjust frequency
  workflow_dispatch: {}

jobs:
  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - # setup rust, just
      - run: cargo bench --workspace
      - # upload results / post to PR / store as artifact

  extended-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - # setup rust, just
      - run: cargo test --workspace -- --include-ignored
      # This runs the ignored tests like fuzz/property tests
```

### Deployment pipeline additions

Add parallel Linux + macOS smoke tests alongside the existing Windows smoke test:

```
publish-prerelease → smoke-test-windows (existing)
                   → smoke-test-linux (new)
                   → smoke-test-macos (new)
                   → all pass → publish-website → publish-release
```

---

## What Cannot Be Automated (Summary)

| Area | Why | Mitigation |
|------|-----|------------|
| Real embedded hardware testing | Requires physical devices, not cloud runners | QEMU for CI; defer real hardware |
| Hard real-time deadline verification | CI VMs have non-deterministic timing | Criterion benchmarks catch gross regressions |
| VS Code extension UX testing | Visual/interactive testing | Existing Windows smoke test covers basic "does it start" |
| IEC 61131-3 spec interpretation | Requires human judgment on ambiguous spec language | Document decisions in conformance tests as executable specs |
| Cross-version compatibility matrix | Combinatorial explosion of compiler × VM versions | Test current version; consider backward compatibility tests when format stabilizes |

---

## Open Questions

1. **Coverage threshold:** The 85% line coverage requirement applies to all crates. As VM tests move to integration tests (which `cargo-llvm-cov` may not count toward crate coverage), should the threshold be adjusted or the measurement approach changed?

2. **Test data management:** As the conformance suite grows, should `.st` test programs live as inline strings in Rust tests (current approach) or as separate files? Inline is simpler; files allow sharing between test harnesses and are easier to review/update.

3. **CI runner budget:** Do you have GitHub Actions minutes constraints? The 5-platform matrix already consumes significant minutes. Adding nightly workflows adds more. If budget is tight, the nightly workflow could run on Ubuntu only and reserve multi-platform for weekly deployment.

4. **Container format stability:** Should golden `.iplc` files be checked in and used as regression tests for the binary format? This catches unintentional format changes but requires regeneration when intentional changes happen.

5. **Benchmark baseline storage:** If we adopt relative performance comparison, where do baselines live? Options: git branch, GitHub Actions cache, or external storage.
