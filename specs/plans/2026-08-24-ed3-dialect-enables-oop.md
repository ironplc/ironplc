# Plan: `--dialect iec61131-3-ed3` enables object-oriented syntax

Issue: #1427

## Motivation

Object-oriented programming is the headline addition of IEC 61131-3:2013,
but the dialect named after that edition cannot parse any of it.
`allow_fb_inheritance` is tagged `[Rusty, Codesys, TwinCat]` — `Iec61131_3Ed3`
is absent — so `--dialect iec61131-3-ed3` demotes `EXTENDS`, `IMPLEMENTS`,
`INTERFACE`, `END_INTERFACE`, `ABSTRACT`, `METHOD`, `END_METHOD`, `THIS`, and
`SUPER` to identifiers and reports P0002 on any use of them. A user who wants
standards-compliant Edition 3 has to select a vendor dialect, which also turns
on C-style comments, optional semicolons, `SIZEOF`, cross-family widening, and
a dozen other non-standard extensions.

This is the change #1290 made for the other Edition 3 features
(`specs/plans/2026-08-05-per-feature-edition3-flags.md`), applied to OOP —
that plan explicitly scoped OOP out as follow-up work.

## Decision

Tag `allow_fb_inheritance` with `Iec61131_3Ed3`. Every keyword the flag gates
is standard Edition 3 syntax, so the whole group belongs to the edition preset;
no per-keyword split is needed to make the dialect correct.

**Not doing:** splitting `allow_fb_inheritance` into `allow_interfaces` /
`allow_methods` / `allow_abstract` (issue #1427 step 1). The split buys
finer-grained control over a set of features that a *standards* dialect wants
in full and that every vendor dialect also enables in full — no dialect in the
matrix would select a proper subset. It is a separable refactor with its own
CLI/LSP/MCP/doc surface; keeping it out keeps this change to one annotation
plus the documentation it invalidates.

## Changes

### Compiler

- `compiler/parser/src/options.rs` — add `Iec61131_3Ed3` to the
  `allow_fb_inheritance` dialect tags; widen its description to name all the
  syntax it gates (it reads as `EXTENDS`/`IMPLEMENTS` only today, but the
  demotion group in `xform_demote_keywords.rs:60` is nine keywords).
- `compiler/parser/src/options.rs` tests — add `allow_fb_inheritance` to the
  `ed3_dialect_enables_edition3_descriptors` expected set (the assertion is
  exact, so the tag change fails the test until it is updated).
- `compiler/parser/src/tests/fb_inheritance.rs` — a regression test that the
  Ed. 3 dialect preset parses `EXTENDS`, `INTERFACE`, `METHOD`, and `THIS^`,
  which is the user-visible bug in the issue.

No change to `xform_demote_keywords.rs`, the CLI flag, the LSP key, or the MCP
option keys: the flag itself is unchanged, only which dialects turn it on.

### Documentation

1. `docs/reference/language/edition-support.rst` — add the object-oriented
   keywords to the Edition 3 feature table (omitted entirely today).
2. `docs/explanation/enabling-dialects-and-features.rst` — list
   `--allow-fb-inheritance` under the `iec61131-3-ed3` dialect; fix the
   flag paragraph's "Enabled by `--dialect=rusty` and `--dialect=codesys`"
   (omits `twincat`) and its stale "not yet semantically supported" claim.
3. `docs/reference/compiler/ironplcc.rst` — same two fixes.
4. `docs/explanation/object-orientation.rst` — drop the claim that IronPLC
   "does not parse" `METHOD`, `THIS`, and `SUPER` (stale since #1386 and
   #1403) and narrow the blanket "parses but does not analyze or execute"
   note to what is still true.
5. `docs/reference/language/object-orientation/method.rst` — new page;
   `METHOD` parses, resolves (static dispatch), and executes, and had no
   reference page.
6. `docs/reference/language/object-orientation/{index,extends,this-and-super,
   interface,implements,abstract}.rst` — correct per-keyword support rows;
   plain `EXTENDS` field inheritance is fully resolved and no longer emits
   P9999.

## Validation

`cd compiler && just` (compile, coverage ≥85%, clippy, fmt) plus a docs build.
