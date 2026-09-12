# Derive the docs section for every problem-code URL

Fixes [#1711](https://github.com/ironplc/ironplc/issues/1711).

## Goal

No channel may hardcode or re-derive the `reference/<section>/` part of a
problem-code documentation URL. `docs_section` in `ironplc-dsl` is the single
authority, and a test makes a wrong section impossible to ship.

## Background

`docs_section` (`compiler/dsl/src/diagnostic.rs`) maps `P → compiler`,
`V → runtime`, `E → editor`. Four places build these URLs and only two of them
ask `docs_section`:

| Channel | Where | Section comes from |
|---|---|---|
| CLI | `compiler/ironplc-cli/src/cli.rs` | `docs_section` |
| MCP | `compiler/mcp/src/tools/explain_diagnostic.rs` | `docs_section` |
| LSP / extension | `compiler/ironplc-cli/src/lsp_project.rs` | **hardcoded `compiler`** — the bug |
| Playground | `playground/src/app.ts` | **its own inline regex**, missing the `E` arm |

The VS Code extension has a fifth copy (`integrations/vscode/src/problemUrl.ts`)
that cannot call Rust, but it already agrees with `docs_section` and already has
a test that walks the docs tree. It stays as it is.

## Architecture

**One builder, not three.** The three Rust channels each format the same URL
with a different `channel=` value and, for two of them, the same
`&file=`/`&line=` suffix. Collapse that into `Diagnostic::help_url` and the free
function `problem_help_url` in `ironplc_dsl::diagnostic`, beside `docs_section`.
After this, no Rust file outside `diagnostic.rs` mentions the docs host at all,
so the LSP bug is not just fixed but unrepresentable.

**The playground gets the mapping from the compiler.** `app.ts` cannot call
Rust, but it already receives compiler-derived data over the worker `ready`
message (`version`, `dialects`). Add a `doc_sections()` WASM export returning
the prefix → section map that `docs_section` implements, thread it through
`ready`, and have `renderDiagnostics` look the section up instead of testing
prefixes itself. This deletes the fourth copy rather than correcting it.

## Prefactoring

The shared-builder extraction above *is* the prefactor: it is a behaviour-preserving
move for the CLI and MCP channels, committed on its own, and it is what makes the
LSP fix a one-line call-site change instead of a second copy of the format string.

## Design doc reference

None. The rationale is local to the functions involved and lands in their doc
comments.

## File map

Modified:

- `compiler/dsl/src/diagnostic.rs` — add `problem_help_url` + `Diagnostic::help_url`; tests
- `compiler/ironplc-cli/src/cli.rs` — call the shared builder
- `compiler/mcp/src/tools/explain_diagnostic.rs` — call the shared builder
- `compiler/ironplc-cli/src/lsp_project.rs` — call the shared builder (the fix)
- `compiler/playground/src/lib.rs` — add the `doc_sections()` export; tests
- `playground/src/worker.ts` — forward `doc_sections()` on `ready`
- `playground/src/types/messages.d.ts` — `ReadyResponse.docSections`
- `playground/src/app.ts` — look the section up instead of re-deriving it

## Tasks

- [ ] Add `problem_help_url` and `Diagnostic::help_url` to `diagnostic.rs`
- [ ] Move the CLI and MCP channels onto it (no behaviour change)
- [ ] Move the LSP channel onto it — fixes the hardcoded section
- [ ] Add `doc_sections()` to the WASM crate and consume it in the playground
- [ ] Tests:
  - [ ] `problem_help_url` builds the documented section for `P`/`V`/`E` and
        `unknown` for an unmapped prefix, per channel
  - [ ] walking `docs/reference/*/problems/`, every documented code's built URL
        points at the directory its page actually lives in
  - [ ] no `.rs` file outside `diagnostic.rs` contains the docs host — the guard
        that keeps a future channel from minting its own URL
  - [ ] LSP `map_diagnostic` on a `V####` diagnostic links to `runtime`
        (regression test for #1711)
  - [ ] `doc_sections()` covers every prefix `docs_section` maps and agrees with it
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
