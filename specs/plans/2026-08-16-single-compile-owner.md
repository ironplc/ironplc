# Plan: Give the Compile Pipeline a Single Owner (Stage C1)

Stage C1 of [the cross-crate test-duplication plan](2026-08-06-reduce-cross-crate-test-duplication.md).
Tracked by [issue #1369](https://github.com/ironplc/ironplc/issues/1369).

## Goal

One `compile()` that runs parse → analyze → codegen, owned by `ironplc-project`
and called by all three front ends (`ironplc-cli`, `mcp`, `playground`). Each
front end then keeps only its own value-add — CLI exit codes and file writing,
MCP's JSON response shape and container cache, playground's wasm bindings and
base64 encoding — instead of re-composing the pipeline.

## Context

`ironplc-project::run_semantic_analysis` covers parse→analyze only. Codegen is
re-composed independently three times:

| Crate | Composes the pipeline at |
|---|---|
| `ironplc-cli` | `compiler/ironplc-cli/src/cli.rs:94` (`compile`) |
| `mcp` | `compiler/mcp/src/tools/compile.rs:66` (`build_response`) |
| `playground` | `compiler/playground/src/lib.rs:625` (`compile_inner`) |

`playground` does not depend on `ironplc-project` at all, so it reassembles the
whole pipeline from `sources` + `analyzer` + `codegen`. Because nobody owns
"compile", every front end tests it.

## The wasm question — resolved

The issue offered two shapes for the playground problem (`playground` targets
`wasm32`, `FileBackedProject` uses `std::fs`):

1. Feature-gate the file-backed half so `MemoryBackedProject` is wasm-clean and
   playground depends on `ironplc-project` with `default-features = false`.
2. Extract the pure pipeline into a `sources`-level (or new) function taking
   already-loaded sources, called by both `ironplc-project` and `playground`.

**Neither is needed.** Measured, not assumed:

```
$ rustup target add wasm32-unknown-unknown
$ cargo check -p ironplc-project --target wasm32-unknown-unknown
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.09s
```

`ironplc-project` already compiles for `wasm32-unknown-unknown` as it stands.
`std::fs` is not a compile-time barrier on that target — the calls compile and
fail at runtime — which is exactly why `playground` can already depend on
`ironplc-sources`, whose `Source::try_from_file_id` and project discovery are
just as filesystem-bound as `FileBackedProject`. Every `ironplc-project`
dependency (`analyzer`, `parser`, `dsl`, `problems`, `sources`, `container`,
`log`, `serde_json`) is already in playground's own dependency set or is
platform-neutral; `tempfile` is dev-only.

**Decision: playground depends on `ironplc-project` directly and calls the same
owned `compile()` the CLI and MCP call.** This is option 2's outcome — one pure
pipeline shared by everyone — without option 2's extra indirection layer and
without option 1's feature flag. Feature-gating would be dead weight: it buys
nothing today and would have to be maintained on every future `project` API.

The property this decision rests on is already enforced in CI: the
`Build Playground` job runs `just ci` in `playground/`, which runs
`wasm-pack build ../compiler/playground --target web`. That compiles the crate
— and now `ironplc-project` with it — for `wasm32-unknown-unknown`, so a
regression fails the build. No extra check is needed.

## Architecture

### The owned pipeline

New module `compiler/project/src/compile.rs`:

```rust
pub struct CompileOutput {
    /// Every diagnostic, in stage order: caller-supplied, then parse and
    /// analysis, then codegen.
    pub diagnostics: Vec<Diagnostic>,
    /// The generated container. `Some` only when every stage was clean.
    pub container: Option<Container>,
}

pub fn compile(
    project: &mut dyn Project,
    compiler_options: &CompilerOptions,
    source_lookup: &dyn SourceLookup,
    diagnostics: Vec<Diagnostic>,
) -> CompileOutput
```

`diagnostics` is the caller's already-collected set (the CLI's discovery and
output-conflict problems; empty for MCP and playground). Seeding it rather than
merging afterwards is what preserves the CLI's contract that codegen runs only
when *nothing* — including discovery — reported a problem, while analysis still
always runs (#1360).

The body is the composition all three front ends spell out today:

1. `diagnostics.extend(project.semantic())` — always, regardless of what the
   caller seeded.
2. Return with no container if `diagnostics` is non-empty.
3. Take the cached `analyzed_library()` / `semantic_context()`.
4. Run `ironplc_codegen::compile` with `CodegenOptions` derived from
   `compiler_options`; on error push the diagnostic and return no container.

`project` gains a dependency on `ironplc-codegen`. No cycle: `codegen` depends
on `dsl`/`analyzer`/`container`/`problems`, never on `project`.

### Two small `sources` / `project` additions playground needs

Playground compiles editor text that has no filename, so it cannot use the
path-extension file-type detection `Source::new` performs:

- `Source::with_file_type` / `SourceProject::add_source_with_file_type` /
  `MemoryBackedProject::add_source_with_file_type` — add a source whose
  [`FileType`] is supplied by the caller rather than derived from the file
  extension. Playground passes `FileType::from_content(source)`, exactly what
  `compile_inner` does today, so XML and TwinCAT editor content keeps working.

- `MemoryBackedProject::set_preparsed_libraries(Vec<Library>)` — compatibility
  libraries already parsed by the caller, injected ahead of user source. The
  browser fetches library text over HTTP (`REQ-CL-playground-001`); the
  registry's `LibraryRegistry::bundled()` reads them from disk and cannot work
  in wasm. `run_semantic_analysis` gains a `preparsed_libraries` parameter and
  treats them as additional activated libraries: registry-loaded first, then
  pre-parsed, then user source. `FileBackedProject` passes an empty slice.

### Deliberate behavior changes

Behavior preservation is the constraint, so both changes the unification forces
are called out here and covered by a test:

1. **Shadowing in playground.** `run_semantic_analysis` applies
   `remove_shadowed_functions` (`REQ-CL-analyzer-004`): a user `FUNCTION` with
   the same name as a library one shadows it instead of colliding. Playground's
   hand-rolled pipeline never did, so the same source succeeded in the CLI and
   errored in the playground. Going through the owned pipeline fixes that
   divergence; nothing regresses, because the only inputs affected are ones
   playground rejected.

2. **P9002 after a total parse failure.** When *every* source fails to parse,
   `analyze` is currently still called with zero libraries and returns
   `NoContent` (P9002) — a second, internal-flavored diagnostic stacked on top
   of the real syntax error. The CLI and MCP report both today; playground
   reports only the syntax error. Rather than propagate the noise to the
   playground UI, `run_semantic_analysis` skips analysis when parsing produced
   diagnostics and yielded no library at all. A project with genuinely no
   sources still reports P9002, which is the case that code is for. Partial
   failure (one file of several fails to parse) is unaffected: analysis runs on
   what did parse, as it does today.

One unreachable branch also changes shape: the CLI's `Err("Internal error:
analysis produced no artifacts")` becomes a P9998 diagnostic through the normal
`finish` path. It is unreachable by construction (clean analysis always caches
its artifacts) and is not worth two error channels.

## File map

**Created**

- `compiler/project/src/compile.rs` — the owned pipeline and its tests.

**Modified**

- `compiler/project/Cargo.toml` — add `ironplc-codegen`.
- `compiler/project/src/lib.rs` — export `compile`, `CompileOutput`.
- `compiler/project/src/project.rs` — `run_semantic_analysis` takes
  pre-parsed libraries and skips analysis after a total parse failure;
  `MemoryBackedProject::{set_preparsed_libraries, add_source_with_file_type}`.
- `compiler/sources/src/source.rs` — `Source::with_file_type`.
- `compiler/sources/src/project.rs` — `SourceProject::add_source_with_file_type`.
- `compiler/ironplc-cli/src/cli.rs` — `compile` calls the owned pipeline.
- `compiler/mcp/src/tools/compile.rs` — `build_response` calls it.
- `compiler/playground/Cargo.toml` — add `ironplc-project`.
- `compiler/playground/src/lib.rs` — `compile_inner` calls it.

## Tasks

- [x] Commit this plan.
- [x] Add `compile.rs` to `ironplc-project` with the owned pipeline.
- [x] Add the `sources` / `MemoryBackedProject` hooks playground needs.
- [x] Move `ironplc-cli::compile` onto the owned pipeline.
- [x] Move `mcp::tools::compile::build_response` onto it.
- [x] Move `playground::compile_inner` onto it; add the `ironplc-project`
      dependency.
- [x] Remove wrapper tests that only re-assert pipeline behavior, naming the
      surviving owner for each.
- [x] `cd compiler && just` green; compare per-file uncovered line counts
      against the captured `lcov.info` baseline.

## Verification

- `cd compiler && just` (compile, coverage ≥ 85%, clippy, fmt, dupes) green.
- `cargo check -p ironplc-playground --target wasm32-unknown-unknown` green, and
  the same check wired into playground CI.
- Per-file *uncovered* line counts compared before/after, not just total
  coverage percent — deleting covered test code lowers the percentage while
  losing nothing.
- For every deleted wrapper test, the surviving owning test is named in the PR
  description. Stages A and B established that "this is redundant" is a
  hypothesis until both sides are read.
- No `#[spec_test(REQ_…)]` test deleted or renamed (the build enforces this
  bidirectionally).
