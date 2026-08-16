# Plan: Share the container test builders (Stage C3)

## Goal

Stop hand-assembling the "steel thread" program (`x := 10; y := x + 32`) and its
container scaffolding in nine files. Publish one set of shared fixtures and
adopt it everywhere, without collapsing genuinely distinct scenarios into each
other and without touching the frozen golden `.iplc` binaries.

Stage C3 of
[`2026-08-06-reduce-cross-crate-test-duplication.md`](2026-08-06-reduce-cross-crate-test-duplication.md).
Test-infrastructure only — no production behavior changes.

## Design doc reference

No `specs/design/` doc governs test scaffolding. The container wire format is
`specs/design/container-format.md`; nothing here changes it.

## Architecture

### Where the builders live: `container`, not `vm`

The tracking plan proposed extending `vm`'s `test-support` feature. Working
through the dependency graph says otherwise:

```
container  ←  vm  ←  vm-cli
    ↑          ↑
    └── project└── codegen
```

`container` is a **dependency of** `vm`. Three of the adopting sites —
`container/src/container.rs`, `container/src/container_ref.rs`,
`container/src/builder.rs` — live inside `container` itself. Homing the
builders in `vm` would force `container` to take a dev-dependency on `vm`, a
`vm → container` / `container → vm` dev cycle. Cargo tolerates that, but it
inverts the layering for no gain: every fixture is built out of
`ContainerBuilder`, which `container` owns, and not one of them needs the VM.

So: **`container` owns the fixtures; `vm` re-exports them.** The re-export
keeps `ironplc_vm::test_support` the single import surface for `vm`'s own
tests, `codegen`, and `benchmarks`, so no existing consumer changes its import
path.

### Feature gating

`container` gains a `test-support` feature that implies `std` (the fixtures are
built from `ContainerBuilder`, which is already `std`-only). The module is
gated `#[cfg(all(feature = "std", any(test, feature = "test-support")))]` so it
never compiles into a normal build, and `container` stays `no_std`-clean with
`--no-default-features`.

`vm`'s existing `test-support` feature gains `ironplc-container/test-support`
so the chain works transitively for `codegen`. `vm` also takes an explicit
`ironplc-container` dev-dependency with the feature on, so `vm`'s own
`#[cfg(test)]` unit tests get the fixtures without relying on the
self-dev-dependency trick to propagate a transitive feature.

Under resolver 2 (workspace root sets `resolver = "2"`), dev-dependency
features are not unified into non-test builds, so `cargo build` never sees
`test-support`.

### What is shared vs. what stays distinct

Shared — the *scaffolding*:

- `steel_thread_bytecode()` and its constant pool, one definition.
- `single_function_container*` family, `timer_test_container` (moved out of
  `vm/tests/it/common/mod.rs`, which nothing outside that test binary can
  import).
- `container_bytes` / `round_trip` — serialize, and serialize-then-parse.
- `steel_thread_single_function_builder()` / `steel_thread_debug_builder()` —
  partially-applied `ContainerBuilder`s, so a caller that needs extras (source
  file table, line map, task entries) chains them on instead of restating the
  base.

Kept distinct — the *scenarios*. The divide-by-zero, fault-with-vars,
scan-divide-by-zero, cyclic-task, doorbell and debug-source-file-table
containers in `vm-cli/tests/cli.rs` each exist to drive a specific path. They
adopt the shared scaffolding where they are steel-thread-shaped and otherwise
keep their own bytecode.

The `single_function_container_*` variants are re-expressed over one private
`scan_scaffold` helper rather than copied per constant type — the previous
six near-identical 15-line bodies are exactly the kind of block
`cargo dupes` counts once they leave a test target.

### Frozen goldens

`compiler/vm-cli/resources/test/steel_thread.iplc` and
`debug_source_file_table.iplc` are not regenerated. `generate_golden_files`
stays `#[ignore]`d. The refactor keeps
`write_debug_source_file_table_container` byte-compatible with the checked-in
golden — same functions, constants, var names, source files and line map, in
the same order — verified by the existing golden-load test.

## File map

Created:

- `compiler/container/src/test_support.rs` — the shared fixtures.
- `specs/plans/2026-08-16-shared-container-test-builders.md` — this plan.

Modified:

- `compiler/container/Cargo.toml` — `test-support` feature.
- `compiler/container/src/lib.rs` — gated `pub mod test_support`.
- `compiler/container/src/container.rs` — adopt.
- `compiler/container/src/container_ref.rs` — adopt.
- `compiler/container/src/builder.rs` — adopt.
- `compiler/vm/Cargo.toml` — feature chain + container dev-dependency.
- `compiler/vm/src/test_support.rs` — re-export container fixtures; absorb the
  `run_and_read_*` / `run_and_expect_trap_i32` helpers.
- `compiler/vm/src/vm.rs` — adopt.
- `compiler/vm/tests/it/common/mod.rs` — becomes a re-export shim.
- `compiler/vm/tests/it/steel_thread.rs` — adopt.
- `compiler/vm/tests/it/debug_engine.rs` — adopt.
- `compiler/vm-cli/Cargo.toml` — dev-dependency feature.
- `compiler/vm-cli/tests/cli.rs` — adopt scaffolding across the seven writers.
- `compiler/project/Cargo.toml` — dev-dependency on container with the feature.
- `compiler/project/src/disassemble.rs` — adopt.

## Tasks

- [ ] Commit this plan.
- [ ] Add `test-support` feature and `test_support` module to `container`.
- [ ] Wire the feature chain through `vm`, `vm-cli`, `project`.
- [ ] Re-export from `vm/src/test_support.rs`; reduce `vm/tests/it/common/mod.rs`
      to a shim.
- [ ] Adopt in `container` (3 files), `vm` (3 files), `vm-cli`, `project`.
- [ ] `cd compiler && just` green, including `--features ironplc-vm-cli/dap`.
- [ ] Confirm test count unchanged and both frozen goldens still load.

## Verification

- `cd compiler && just` (compile, coverage ≥ 85%, lint, dupes) green.
- `cargo test --workspace --features ironplc-vm-cli/dap -- --list` count
  unchanged before/after — this deduplicates fixtures, not tests.
- `cargo build -p ironplc-container --no-default-features` still compiles, and
  `cargo tree -e features` shows no `test-support` in a plain `cargo build`.
- The `run_when_golden_container_file_then_ok` and
  `read_when_debug_source_file_table_golden_then_decodes_new_debug_fields`
  tests pass against the unchanged checked-in binaries.
