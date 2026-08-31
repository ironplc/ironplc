# ADR Hygiene: Titles, Statuses and Records That Match What Was Built

Fixes [#1496](https://github.com/ironplc/ironplc/issues/1496). Every item below
is a case where reading the ADR directly misleads the reader — the point of
having ADRs at all.

## Prefactoring

The prefactor is item 8: `development-standards.md` currently says ADRs "are
permanent records — they are not updated after the decision is made," full stop.
Read literally that forbids every other fix in this plan, and it is why the
triage that found these problems deliberately left them unfixed. So the
convention lands **first**, in its own commit, and the ADR edits then follow the
rule it states rather than contradicting it.

## Phases

### 1. ADR lifecycle convention (prefactor)

`specs/steering/development-standards.md`, `specs/adrs/` section:

* Name the four statuses (`proposed`, `accepted`, `rejected`,
  `superseded by ADR-NNNN`) and say that the pull request that lands the work
  flips `proposed` → `accepted`.
* Carve out what may be corrected in place versus what needs a new ADR:
  - **Correct in place** — the record of a decision that was never true of the
    decision itself: a title or filename naming the wrong option, a status that
    never advanced, a cross-reference, a section describing a rejected option as
    if adopted.
  - **New superseding ADR** — changing what was decided.
  - **Dated postscript** — context that has since aged, appended, never a
    rewrite of the original Context.

### 2. ADR-0024 — title names the rejected option

* `git mv` to `0024-function-local-reinit-via-bytecode-prologue.md`; H1 to match.
* Move "Container Format Changes" and "Mitigating the Divergence Risk" out of
  More Information and under "Option A: Init Template Section" in Pros and Cons,
  in the conditional voice — they describe the option that was **not** chosen,
  and `container/src/header.rs` still has `reserved: [u8; 38]`.
* "Relationship to ADR-0014" says "the init template handles function locals";
  it is the bytecode prologue.
* Repoint `specs/design/compatibility-libraries.md` (2 links).

### 3. ADR-0011 — cited as the source for its own rejected option

ADR-0011 chose `Err(Trap::MissingReturn)`. No such variant exists;
`vm.rs` treats fall-off-end as `RET_VOID`, and
`bytecode-instruction-set.md:380` documents *that* while citing ADR-0011.

Resolve as a decision, not a status edit. The safety argument ADR-0011 made is
now answered statically: `container/src/verify.rs` models offset `len` as a
return site and checks it against `RET_VOID_DEPTH`, so an unbalanced fall-off
is rejected before execution. Write **ADR-0044** recording implicit `RET_VOID`
plus verifier enforcement, mark ADR-0011 `superseded by ADR-0044`, repoint the
design doc.

### 4. ADR-0022 — superseded in practice, unmarked

`status: superseded by ADR-0029, ADR-0031`, plus a Supersession section saying
what survives (narrowing and lossy conversions are still rejected).

### 5. ADR-0038 — shipped, still proposed; premise aged

`status: accepted`. Its Context says "no single real dialect enables both"
`REF_TO` and `REFERENCE TO`; `Dialect::Codesys` now enables both. Decision
unaffected — record it as a dated postscript per phase 1.

### 6. ADR-0030 — pre-template format, no date, stuck at proposed

Convert to front-matter (`status: accepted`, `date: 2026-08-24` — the date the
file landed), `## Context and Problem Statement`, options as a bulleted
Considered Options list with the prose moved under Pros and Cons.

### 7. ADR-0042 — mandates a mechanism that was never built

Rule 3 requires vendor natives to be unnamed builtins reached through a manifest
binding. That binding form does not exist and was rejected on security grounds
(manifest becomes an input to code *emission*; nothing guarantees the declared
signature matches the builtin's stack behaviour). `__TRUNC`/`__MOD` are
compiler-seeded intrinsics in the reserved `__` namespace instead.

The rationale survives at `stdlib_function.rs:1054`, so nothing is lost — but
the ADR still instructs a contributor to build the rejected mechanism. Amend
rule 3 and Confirmation item 3 to the `__`-namespace form, add a rejected-option
section for manifest bindings, flip to `accepted`.

### 8. ADR-0010 — Confirmation never wired up

Here `proposed` is accurate, so record what happened rather than flip it: an
Implementation Status section listing what landed (vm/vm-cli split,
`ironplc-container` `#![no_std]` + `std` feature) and what did not
(`ironplc-vm` still uses `std::time::Instant`; no cross-compile CI gate). File a
tracking issue for the remaining work and cite it.

## Out of scope

* Making `ironplc-vm` actually `no_std` — tracked by the issue from phase 8.
* Duplicate ADR numbers (0014, 0021, 0022 each used twice). Renumbering breaks
  every existing link; file separately.

## Verification

`cd compiler && just` (docs-only change; `just plan-citations` must pass after
this plan is `git rm`'d).
