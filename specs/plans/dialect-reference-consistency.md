# Consistent Dialect References in the Docs Website

## Context

The documentation website (`docs/`) should have exactly two canonical places
that describe IronPLC's dialects and how to select them:

- `docs/explanation/enabling-dialects-and-features.rst` — the dialects and
  flags reference.
- `docs/reference/editor/settings.rst` — the VS Code `ironplc.dialect` setting.

Throughout the rest of the site, individual feature/problem pages should **not**
enumerate which dialect presets (`rusty`, `codesys`, `twincat`,
`iec61131-3-ed3`) happen to enable a given feature. That duplication is
inconsistent from page to page and drifts as dialect definitions change. A
feature that is gated behind a `--allow-*` flag should name **the flag** and
then point the reader to the dialects-and-flags page.

For example, `P4033.rst` currently says the partial-access flag is "also
enabled by the `rusty` and `iec61131-3-ed3` dialect presets" and lists
`--dialect=iec61131-3-ed3` / `--dialect=rusty` as fixes. We want it to name
`--allow-partial-access-syntax` and link to the dialect page instead.

## Approach

### 1. New reusable include: `docs/includes/enabled-by-flag.rst`

reStructuredText `include` directives cannot take positional arguments, but the
including document's substitution definitions are in scope inside the included
file. The include therefore reads a `|flag|` substitution that each caller
defines immediately before the include:

```rst
.. |flag| replace:: ``--allow-bit-string-case-labels``
.. include:: /includes/enabled-by-flag.rst
```

The include contains the standard text (name the flag, mention dialects
generically, link to the dialects-and-flags page). This mirrors the existing
`requires-vendor-extension.rst` / `requires-edition3.rst` includes.

### 2. Trim the edition-3 include

`requires-edition3.rst` currently names the `rusty` dialect specifically. Trim
it to name only `--dialect iec61131-3-ed3` (the edition enabler) and point to
the dialect page, so the shared edition note stops enumerating vendor dialects.

### 3. Fix the violation pages

Replace per-page dialect enumerations with the flag + include (or, for
table cells where a directive can't be inserted, drop the `or --dialect X`
suffix and rely on the page's prose/"Enabling" section to link out):

- `reference/compiler/problems/P4033.rst` — `--allow-partial-access-syntax`
- `reference/compiler/problems/P4036.rst` — `--allow-mixed-located-var-declarations`
- `reference/compiler/problems/P4037.rst` — `--allow-constant-initializer-expressions`
- `reference/compiler/problems/P4041.rst` — `--allow-bit-string-case-labels`
- `reference/compiler/problems/P0004.rst` — `--allow-c-style-comments`
- `reference/extension-library/functions/sizeof.rst` — `--allow-sizeof`
- `reference/extension-library/variables/system-uptime.rst` — `--allow-system-uptime-global`
- `reference/extension-library/functions/isvalidref.rst` — `--allow-reference-to`
- `reference/extension-library/index.rst`
- `reference/language/data-types/derived/reference-types.rst`
- `explanation/references.rst`
- `reference/language/structured-text/bit-access.rst`

## Not in scope (legitimate references — left unchanged)

- The two canonical pages above, plus `reference/compiler/ironplcc.rst`
  (compiler-flag reference) — all required by the `ironplc_flags.py` doc guard.
- `trademarks.rst` — trademark attribution.
- `reference/language/edition-support.rst` — the edition reference page itself.
- `reference/compiler/problems/P0010.rst` — the canonical "enable Edition 3"
  page; editions have no `--allow-*` flag, and it already links to the dialect
  page.
- `reference/mcp/tools.rst` — documents the `dialect` API parameter's valid
  values ("for example …; call `list_options` for the authoritative list").
- `how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent.rst` — a passing
  example of what to tell an AI agent.
- `:dialect:` options on `playground*` example directives — these configure the
  runnable example.
- `CODESYS: <url>` "See also" citation links, and prose naming real PLC
  environments (CODESYS/TwinCAT/RuSTy) as the *origin* of a vendor extension —
  these describe provenance, not IronPLC dialect selection.

## Validation

`cd docs && just` (runs `sphinx-build -a -W -n`). The `ironplc_flags.py` guard
still passes because no flag or dialect is removed from the two canonical pages.
