# Guard the Dialect Set Against Silent Drift

## Context

The set of dialects has a single source of truth in
`compiler/parser/src/options.rs` — the `Dialect` enum, `Dialect::ALL`, and the
`cli_name()` / `display_name()` / `description()` methods. Several other places
re-enumerate the four (now five) dialects by hand. Most are **not** tied back to
that source of truth by any test or build guard, so adding a new `Dialect`
variant can silently drift: a picker or docs page ships an incomplete list with
no failure.

This is the same failure mode already fixed for the `--allow-*` feature flags
(GitHub issue #1235).

### Places coupled to the dialect set

**Already guarded (the model to follow):**

- **CLI `ValueEnum`** — `compiler/ironplc-cli/bin/main.rs` mirrors `Dialect::ALL`,
  guarded by the test `clap_dialect_value_variants_when_compared_then_matches_dialect_all`.

**Unguarded (this plan adds guards):**

- **VS Code settings schema** — `integrations/vscode/package.json`
  (`ironplc.dialect`): a `markdownDescription` plus three index-aligned parallel
  arrays (`enum`, `enumItemLabels`, `enumDescriptions`).
- **Editor settings reference** — `docs/reference/editor/settings.rst`
  (`:Values:` line + per-dialect bullets). Currently referenced by **no** guard.
- **Dialects explanation page** —
  `docs/explanation/enabling-dialects-and-features.rst` ("Supported Dialects").
  The existing Sphinx guard `docs/extensions/ironplc_flags.py` only checks
  `--allow-*` flags, not dialects.

## Approach

Keep `Dialect::ALL` as the single source of truth and add two guards that mirror
the patterns already in the repo. This is lower-risk than generating
`package.json`/`.rst` from the enum and matches how the `--allow-*` drift was
fixed. Generation from the enum remains a possible follow-up.

### Guard A — VS Code `package.json` (new Rust test in `ironplc-cli`)

`serde_json` is already a dependency of `ironplc-cli`. Add a test that reads
`integrations/vscode/package.json` (relative to `CARGO_MANIFEST_DIR`), parses
the `ironplc.dialect` block, and asserts:

1. `enum` equals `Dialect::ALL.iter().map(|d| d.cli_name())` (order-sensitive).
2. `enum`, `enumItemLabels`, and `enumDescriptions` all have the same length —
   catches a parallel array that drifts out of index alignment.
3. Every `cli_name()` appears as a substring of `markdownDescription`.

JSON cannot read the Rust enum, so this small comparison test is the pragmatic
guard. It runs inside `cd compiler && just`.

### Guard B — docs (extend the Sphinx guard)

Add dialect validation to `docs/extensions/ironplc_flags.py` alongside the
existing `--allow-*` check. Extract the dialect `cli_name` string literals from
the `fn cli_name` match arms in `options.rs` (the same source-of-truth file the
guard already reads), and assert each appears in **both**:

- `docs/explanation/enabling-dialects-and-features.rst`
- `docs/reference/editor/settings.rst`

The build fails if any dialect is missing from either page, mirroring what the
guard already does for `--allow-*` flags and closing the currently-unguarded
`settings.rst`.

## Acceptance

Adding a new `Dialect` variant fails:

- the new `ironplc-cli` Rust test (VS Code `package.json`),
- the docs build (both `.rst` pages),
- plus the pre-existing CLI `ValueEnum` test.

...rather than silently shipping an incomplete picker or docs.

## Out of scope

- Generating the dialect list from `Dialect::ALL` (docs snippet and/or VS Code
  enum). Listed as a possible follow-up in the issue; not done here.
- Any change to compiler/parser behavior.
