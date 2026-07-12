# Plan: Actionable C-style comment guidance + full dialect selection in the playground

## Problem

C-style comments (`//`, `/* */`) are the third most common error playground
users hit (problem code P0004). IronPLC supports them via the
`--allow-c-style-comments` flag (enabled by the `rusty` and `codesys`
dialects), but the diagnostic gives no hint on how to proceed, and the
playground only exposes two of the four dialects.

A key project principle is that **there is no "IronPLC dialect."** So the fix
must *not* silently default-enable C-style comments (that would amount to
inventing a lenient IronPLC dialect). Instead we make the path to a solution
discoverable: a better diagnostic, and a playground that can select any
supported dialect.

## Approach

Two independent, composable changes:

1. **Actionable diagnostics.** Add a general-purpose "help note" channel to the
   shared `Diagnostic` type (distinct from labels, which by convention describe
   *what is at a location*, not how to fix it). Render the help note in all
   three consumers: CLI (codespan `with_notes`), LSP (appended to the message),
   and the playground (new `help` field on `DiagnosticInfo`). Attach a static,
   dialect-agnostic help note at the P0004 site telling the user to either
   convert to IEC comment syntax `(* *)` or choose a dialect that supports
   C-style comments. The note text is intentionally generic — the error site
   does not (and need not) enumerate dialects.

2. **Full dialect selection in the playground.** Replace the two-option dialect
   dropdown with all of `Dialect::ALL` (ed2, ed3, rusty, codesys), using each
   dialect's `cli_name()` as the option value. Rewrite `compiler_options_from`
   to resolve the dialect string through `Dialect::from_str`, honoring explicit
   selections honestly. Preserve backward compatibility: legacy `2003`/`2013`
   values and the empty string keep working, and the empty string continues to
   map to `rusty` so the ~127 existing doc embeds that rely on the lenient
   default do not regress.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/diagnostic.rs` | Add `help: Vec<String>` field, `with_help()` builder, and `help()` accessor to `Diagnostic`. |
| `compiler/ironplc-cli/src/cli.rs` | Map `diagnostic.help()` to codespan `.with_notes()`. |
| `compiler/ironplc-cli/src/lsp_project.rs` | Append help notes to the LSP diagnostic message. |
| `compiler/parser/src/rule_token_no_c_style_comment.rs` | Attach the static help note (`(* *)` or supported dialect) to the P0004 diagnostic. |
| `compiler/playground/src/lib.rs` | Add `help` to `DiagnosticInfo`; rewrite `compiler_options_from` to resolve via `Dialect::from_str` with legacy + empty-string handling. |
| `playground/index.html` | Dropdown lists all four dialects with `cli_name` values. |
| `playground/src/app.ts` | Widen dialect type; map legacy `2003`/`2013` URL params to `cli_name`; update badge logic. |
| `playground/tests/e2e.spec.ts` | Update any assertions tied to the old dialect option values. |

## Decisions

- **Default stays lenient (`rusty`).** Flipping the empty-string default to
  strict `ed2` would break the majority of the 127 doc embeds that use vendor
  syntax without declaring a dialect. The improved diagnostic covers strict
  dialects, the CLI, and the LSP — where the default *is* strict.
- **Diagnostic is dialect-agnostic at the error site**, per the request. Help
  text: convert to `(*` … `*)`, or select a dialect that supports C-style
  comments.

## Tasks

- [ ] Write plan (this file)
- [ ] Add `help` note support to `Diagnostic` + render in CLI and LSP
- [ ] Attach static help note at the P0004 site
- [ ] Add `help` to playground `DiagnosticInfo`
- [ ] Rewrite `compiler_options_from` + expand the playground dropdown to all dialects
- [ ] Update `app.ts` URL-param/badge handling and e2e tests
- [ ] Run full CI (`cd compiler && just`) and playground build/tests
