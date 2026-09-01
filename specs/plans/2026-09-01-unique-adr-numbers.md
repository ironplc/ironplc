# Plan: Enforce Unique ADR Numbers and Renumber the Three Collisions

Issue: https://github.com/ironplc/ironplc/issues/1585

## Problem

Three numbers in `specs/adrs/` name two decisions each (0014, 0021, 0022), so
~40 bare `ADR-00NN` citations in prose and code comments do not resolve. The
numbering rule is written down nowhere, which is how two branches took the same
number in the first place.

## Approach

Chosen resolution (per the issue's first option, as directed): renumber the
*later* ADR of each pair — later by the `date:` in its own front matter — to the
next unused number, and fix every citation that meant it.

| Number | Keeps it (earlier) | Renumbered (later) | New |
|---|---|---|---|
| 0014 | `vm-error-code-categories` (2026-03-04) | `variable-initialization-in-bytecode` (2026-03-05) | 0045 |
| 0021 | `time-32bit-ltime-64bit` (2026-03-11) | `flat-variable-table-for-function-calls` (2026-03-12) | 0046 |
| 0022 | `edition-3-compiler-flag` (2026-03-11) | `exact-type-matching-for-function-arguments` (2026-03-12) | 0047 |

The highest existing number is 0044, so 0045–0047 are the next three free.

## Prefactoring

None needed. The check is a new, self-contained `just` recipe alongside the
existing `plan-citations` recipe; there is no existing code the change has to be
squeezed into.

## Steps

1. **Test first.** Add a `adr-numbers` recipe to `compiler/justfile`, modelled on
   `plan-citations`: inline bash, `git ls-files` over `specs/adrs/`, fail on a
   repeated four-digit prefix. Wire it into `default` so `just ci` (and therefore
   the `compiler-quality` CI job) runs it. Demonstrate it failing on the three
   collisions before anything is renamed.
2. **Renumber.** `git mv` the three later files, then update citations:
   - link form (`../adrs/00NN-slug.md`) — mechanical, the slug disambiguates;
   - bare `ADR-00NN` in prose and one code comment — each read in context to tell
     which of the two decisions was meant.
3. **Write the rule down.** Add the numbering rule (and the check that enforces
   it) to the `specs/adrs/` section of
   `specs/steering/development-standards.md`, so the next author knows to take
   the next unused number.
4. Re-run the recipe: passes.
5. `cd compiler && just` for the full gate.

## Citations to update

- → **ADR-0045**: `0024-function-local-reinit-via-bytecode-prologue.md` (3 sites).
- → **ADR-0046**: `specs/design/adr-and-pointer-to.md`,
  `specs/design/bytecode-instruction-set.md` (2 of its 5 ADR-0021 sites — the
  other 3 mean TIME), `specs/design/user-defined-function-calls-design.md`,
  `0024-function-local-reinit-via-bytecode-prologue.md`,
  `compiler/codegen/src/compile_fn.rs`.
- → **ADR-0047**: `specs/design/user-defined-function-calls-design.md`,
  `0028-literal-type-inference-across-numeric-families.md`,
  `0029-implicit-integer-widening.md`,
  `0031-expanded-implicit-type-widening.md`.

Unchanged (they mean the ADR that keeps its number): the ADR-0014 citation in
`specs/design/vm-error-codes.md`; the TIME-related ADR-0021 citations in
`bytecode-instruction-set.md`, `time-literals.md`, `0025`, `0030`, and
`0022-edition-3-compiler-flag.md`; the Edition-3 ADR-0022 citations in
`ref-to.md`, `time-literals.md`, and `0036-no-ironplc-dialect.md`.

## Not delivered

Nothing. The issue also floats a linter for bare citations under the
*keep the numbers* option; that option is not the one taken, so no such linter is
needed.
