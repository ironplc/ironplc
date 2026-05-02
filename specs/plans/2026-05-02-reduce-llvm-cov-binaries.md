# Reduce llvm-cov build size and link time

## Goal

Cut `compiler/target/` size and `cargo llvm-cov` link time without changing
test behavior or coverage. A recent llvm-cov build produced ~15 GB in
`target/`, and the build/test loop is correspondingly slow.

## Architecture

Two compounding causes, addressed independently and additively:

1. **Default debug info is full DWARF** (`debug = 2`) for `dev` and `test`
   profiles. `cargo llvm-cov` only needs source/line tables to map coverage
   hits back to source — full DWARF (type info, variable info, inline scopes)
   is dead weight.

2. **Integration tests fan out into one binary per file.**
   `compiler/codegen/tests/` had 142 `.rs` files and `compiler/vm/tests/` had
   32 — Rust compiles each as its own crate root, statically linking the
   entire dependency graph (codegen → analyzer → parser → vm, …) plus the
   test harness, plus llvm-cov instrumentation. That's 174 independent link
   steps. The test files are stateless, share helpers in `common/mod.rs`, and
   have no process-level side effects, so the per-file isolation Rust
   provides was paying for nothing.

The two changes are independent and bisectable.

### Change 1: line-tables-only debug profiles

Add to `compiler/Cargo.toml`:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.test]
debug = "line-tables-only"
```

Sufficient for `cargo llvm-cov` source mapping; typical binary-size reduction
is 60–80% with zero behavior change.

### Change 2: consolidate integration tests into one binary per crate

Layout for codegen (vm is identical):

```
compiler/codegen/tests/
  it/
    main.rs          # crate root: declares all submodules
    common/
      mod.rs         # shared helpers + e2e_* macros
    compile_abs.rs
    compile_add.rs
    ... (142 files)
```

`tests/it/main.rs` is wired up via a single `[[test]]` entry in
`codegen/Cargo.toml`:

```toml
[[test]]
name = "it"
path = "tests/it/main.rs"
```

`main.rs` is the crate root, so `mod compile_abs;` resolves to
`tests/it/compile_abs.rs` without `#[path]` attributes on every declaration.
Per-file imports change from `mod common; use common::X;` to
`use crate::common::X;`. Macros declared in `common/mod.rs` reference
`$crate::common::...` so they expand correctly inside sibling submodules.

## File map

- `compiler/Cargo.toml` — add `[profile.dev]` and `[profile.test]` blocks.
- `compiler/codegen/Cargo.toml` — add `[[test]] name = "it" path = "tests/it/main.rs"`.
- `compiler/vm/Cargo.toml` — same.
- `compiler/codegen/tests/it/main.rs` — new (single binary's crate root).
- `compiler/codegen/tests/it/common/mod.rs` — moved from `tests/common/mod.rs`;
  e2e macros switched to `$crate::common::…` paths.
- `compiler/codegen/tests/it/*.rs` — 142 files moved from `tests/`; per-file
  `mod common;` removed and `use common::X` rewritten to `use crate::common::X`.
- `compiler/vm/tests/it/main.rs` — new.
- `compiler/vm/tests/it/common/mod.rs` — moved from `tests/common/mod.rs`.
- `compiler/vm/tests/it/*.rs` — 32 files moved + import rewrites.

## Tasks

- [x] Add `[profile.dev]` and `[profile.test]` to `compiler/Cargo.toml`.
- [x] Verify workspace builds clean under the new profile.
- [x] Move 142 codegen test files + `common/` into `tests/it/`.
- [x] Create `tests/it/main.rs` with `#[macro_use] mod common;` and one
      `mod` line per file.
- [x] Rewrite `mod common;` / `use common::…` / `common::…` in moved test
      files to `crate::common::…`.
- [x] Switch e2e macros in `common/mod.rs` to `$crate::common::…`.
- [x] Add `[[test]] name = "it" path = "tests/it/main.rs"` to
      `codegen/Cargo.toml`.
- [x] Repeat the consolidation for `compiler/vm/tests/`.
- [x] Run `cd compiler && just` and confirm compile + coverage (≥ 85%) +
      lint + dupes all pass.

## Verification

- `cargo test --test it` runs the full suite as a single binary in each
  crate. Codegen: 1004 passing. VM: 142 passing.
- Full pipeline (`cd compiler && just`) green.
- `target/` size after change: ~6.7 GB (down from ~15 GB), with the bulk of
  the savings coming from far fewer integration-test binaries plus
  line-tables-only DWARF.

## Risks and mitigations

- **Macro path resolution.** `common/mod.rs` defines `e2e_i32!`, `e2e_i64!`,
  `e2e_f32!`, etc. Moved to `$crate::common::…` so they resolve
  unambiguously when expanded inside sibling submodules.
- **Developer ergonomics.** `cargo test --test compile_abs` becomes
  `cargo test --test it compile_abs::` (filter by module path within the
  single `it` binary). Same outcome, slightly different invocation.
- **No behavior change.** Test functions are unchanged; only their crate
  layout differs. `cargo test` still emits one line per `#[test] fn`.
