# Plan: diagnostics as data — one fold point per command

## Problem

The analyzer already follows the "go as far as you can, collect
everything" model: `stages::semantic()` runs every rule and concatenates
their diagnostics, `resolve_types()` recovers per-transform, and
`analyze()` returns `Ok((library, context))` with errors stored in the
context. But every layer above it converts diagnostics back into control
flow, each in its own way:

- `Project::semantic()` returns `Result<(), Vec<Diagnostic>>` — a shape
  that is just "is the vec non-empty" wearing a `Result` costume.
- The CLI's `create_project` prints its discovery diagnostics, throws
  the data away, and keeps only a `had_discovery_error: bool` (#1360)
  that each caller must remember to fold into its final result.
- Each CLI command (`check`, `echo`, `tokenize`, `compile`) hand-writes
  its own epilogue deciding which stringly-typed message wins
  ("Error during analysis" vs "Error enumerating or reading source
  files"), so the reported failure stage is an artifact of code order,
  not of what actually happened.
- `compile` duplicates the whole parse → load-libraries → analyze
  pipeline inline instead of using `Project::semantic()`, so the rule
  "codegen only runs on a clean project" has to be re-verified in two
  implementations.

## Design

Standard compiler-driver pattern: every stage returns its artifact plus
the diagnostics it produced; nothing prints or decides mid-pipeline; one
place per command prints everything once and derives pass/fail — and the
codegen gate — from the same collection.

### project crate

- `Project::semantic()` changes signature from
  `Result<(), Vec<Diagnostic>>` to `Vec<Diagnostic>` (empty = clean).
  The cached `semantic_context()` / `analyzed_library()` accessors are
  unchanged; their doc comments no longer need to explain the
  "may be `Some` even when `semantic()` returned `Err`" contract.
- `run_semantic_analysis` returns
  `(Vec<Diagnostic>, Option<SemanticContext>, Option<Library>)`.
- `tokenizer::tokenize_source` returns `Vec<Diagnostic>` instead of
  printing diagnostics through an injected `DiagnosticHandler` callback
  and returning `Result<(), String>`. Token dumps still go to stdout;
  the callback type is deleted. XML parse failures become an entry in
  the returned vec instead of an early `Err`.

### CLI (`ironplc-cli/src/cli.rs`)

- `create_project` returns `(FileBackedProject, Vec<Diagnostic>)`.
  It no longer prints and no longer decides — the `bool` from #1360
  disappears because "had a discovery error" is now just "discovery
  contributed diagnostics to the bag".
- Every command follows the same shape: accumulate stage diagnostics
  into one `Vec`, then finish through a shared helper that calls
  `handle_diagnostics` exactly once and returns `Ok(())` when the bag
  is empty or `Err("<Command> failed with N problem(s)")` otherwise.
  The count in the message is deliberate: it lets tests (and users)
  distinguish "discovery problem only" (1) from "discovery problem AND
  analysis ran and found a real bug" (2) without stage-specific magic
  strings.
- `check`: discovery diagnostics + `project.semantic()`.
- `echo`: discovery diagnostics + per-source parse/render diagnostics.
  The stray `print!("Syntax error")` marker interleaved into the echoed
  stream is removed — failures are reported through the diagnostic sink.
- `tokenize`: discovery diagnostics + per-source tokenize diagnostics.
  No longer stops at the first failing source.
- `compile`: rebuilt on the project API
  (`semantic()` + `analyzed_library()` + `semantic_context()`) instead
  of its duplicated inline pipeline. The gate is literal:
  `if diagnostics.is_empty() { codegen + write }`. The output-path
  conflict check (P6009) becomes a diagnostic in the bag; because the
  bag is then non-empty, codegen (and the truncating `File::create`)
  never runs. Filesystem failures creating/writing the output remain
  direct `Err` strings — they are infrastructure errors, not source
  diagnostics.

### LSP and MCP

- `lsp_project.rs::semantic_all` reads the returned vec directly.
- The eight MCP tools' `match project.semantic() { Ok(()) => vec![], Err(d) => … }`
  collapse to a direct call. No behavior change.

## Behavior changes (deliberate)

1. **`compile` writes no container when any diagnostic exists** —
   including discovery-only diagnostics such as P6011
   (referenced-but-unbundled library). Previously (#1360) it wrote the
   container and still exited non-zero. An artifact produced by a
   failing command is a trap for build scripts; when P6011 should stop
   blocking compilation the principled fix is diagnostic severities
   (make it a warning), not a codegen exception. The #1360 test that
   asserted the container was written now asserts it is not.
2. **Diagnostics print once, at the end of the command**, instead of
   interleaved per stage. Content is unchanged; ordering is now
   stage-independent (discovery diagnostics appear alongside analysis
   diagnostics in one report).
3. **Exit messages are uniform**: `"<Command> failed with N problem(s)"`
   replaces "Error during analysis" / "Error enumerating or reading
   source files" / "Tokenize error" / "Error echo source". The stage
   distinction those strings encoded was arbitrary whenever multiple
   stages had problems.
4. **`tokenize` and `echo` process every source** instead of stopping at
   the first failure — same continue-and-collect model as analysis.

## Files

- `compiler/project/src/project.rs` — trait + both impls + tests
- `compiler/project/src/tokenizer.rs` — return diagnostics, drop callback
- `compiler/ironplc-cli/src/cli.rs` — `create_project`, four commands,
  shared finish helper, tests
- `compiler/ironplc-cli/src/lsp_project.rs` — `semantic_all`
- `compiler/ironplc-cli/tests/cli.rs` — message/stdout assertions
- `compiler/mcp/src/tools/{check,compile,pou_lineage,pou_scope,project_io,project_manifest,symbols,types_all}.rs`

## Tests

- Project-crate tests flip from `is_ok()`/`is_err()`/`unwrap_err()` to
  `is_empty()`/`!is_empty()`/direct use of the vec.
- `check_when_plcproj_has_valid_and_missing_entries_then_semantic_error_still_surfaces`
  and `check_when_plcproj_references_unbundled_library_then_still_runs_analysis`
  now assert `Err("Check failed with 2 problem(s)")` — the count proves
  both the discovery diagnostic and the semantic diagnostic surfaced,
  which is a stronger observable than the stage-named string was.
- `compile_when_plcproj_references_unbundled_library_and_unused_then_still_produces_container`
  becomes `..._then_no_container_written`: still `Err`, and asserts the
  output file is empty (behavior change 1).
- Integration test `check_when_binary_encoded_then_error` asserts the
  new message prefix; `echo_when_syntax_error_file_then_err` asserts
  stderr diagnostics and empty stdout instead of the removed
  `"Syntax error"` stdout marker.

## Verification

`cd compiler && just` (build, coverage ≥85%, clippy, fmt) passes.
