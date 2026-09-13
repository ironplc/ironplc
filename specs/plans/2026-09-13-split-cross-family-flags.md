# Plan: split `--allow-cross-family-widening` into three flags

## Problem

`--allow-cross-family-widening` is one flag gating three different rules,
and its name describes only one of them. From `compiler/analyzer/src/type_compat.rs`:

| Site | Rule | Widening? |
|---|---|---|
| `:82` via `can_widen_cross_family_to` (strictly-wider arm) | `BYTE` → `INT`, target strictly wider | yes |
| `:82` via `can_widen_cross_family_to` (equal-width arm) | `UDINT` ↔ `DWORD`, both directions | no — same 32 bits, reinterpreted |
| `:60`, `:164` | bare integer literal → `BYTE`/`WORD`/`DWORD`/`LWORD` | no — literal typing (ADR-0028) |

Widening moves a value into a wider type. The `UDINT` ↔ `DWORD` rule moves
nothing: both types occupy the same 32-bit slot, so it is a reinterpretation,
and it runs in both directions. Literal typing is a third thing again. One
name covering all three masks two of them, and a user reading the flag cannot
tell what they are turning on.

## Decision

Three flags, one rule each:

* `--allow-cross-family-widening` — bit-string → integer, target strictly
  wider. Unchanged meaning; the name becomes accurate.
* `--allow-cross-family-conversion` — `UDINT` ↔ `DWORD` at equal width, both
  directions.
* `--allow-int-literal-to-bit-string` — a bare integer literal where a
  bit-string type is expected. Named after the existing
  `--allow-int-to-bool-initializer`.

All three are enabled in `Rusty`, `Codesys` and `TwinCat`, exactly the set
that enables the single flag today, so no dialect changes behaviour. Narrowing
the conversion flag to the dialect its evidence came from is a separate
decision, deliberately not taken here.

## Prefactor

The prefactor is the split itself: `ElementaryTypeName::can_widen_cross_family_to`
currently answers two unrelated questions in one `match`. Separating it into
`can_widen_cross_family_to` (strictly wider) and `can_convert_cross_family_to`
(equal width, both directions) is what makes the flag split drop in, so it
lands first, before any flag is added.

## Steps

1. Split the predicate in `compiler/dsl/src/common.rs`; keep both rules and
   their tests passing under the single existing flag.
2. Add the two new flags to `define_compiler_options!` in
   `compiler/parser/src/options.rs`, each with `[Rusty, Codesys, TwinCat]`.
3. Gate each of the three sites in `type_compat.rs` on its own flag.
4. Add the CLI args in `compiler/ironplc-cli/bin/main.rs` and wire them in
   `apply`.
5. Tests: cover each flag independently, including that enabling one does not
   enable another.
6. Docs: describe three flags in `enabling-dialects-and-features.rst` and
   `ironplcc.rst`, update the three per-dialect flag lists, and point
   `type-conversions.rst` at the right flag per rule.
7. Amend ADR-0031 to record the split.

## Out of scope

* Changing which dialects enable any rule.
* Whether the `UDINT` ↔ `DWORD` exception should exist at all (settled: it is
  correctly gated on an allow flag).
* Consolidating the duplicated flag descriptions behind a shared include.

Refs #1570
