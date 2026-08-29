# Remove Codegen and VM Plan Citations

**Goal:** Remove the 7 `specs/plans/` citations in the codegen and VM
optimization cluster, repointing each at `specs/design/vm-performance.md` or
deleting it where the surrounding comment already stands alone.

**Architecture:** `specs/design/vm-performance.md` owns the optimization
catalogue. §11 already carries a "Status (partial)" note for the CMP_BR
superinstruction, which is the convention the TRUNC-elision citations should
follow.

**Issue:** #1464 (Phase 1)

**Design doc reference:** `specs/design/vm-performance.md` §11, §13

---

## Scope

7 citation sites across four plans:

| Plan | Sites |
|---|---|
| `2026-04-30-elide-for-loop-trunc.md` | 4 |
| `2026-05-02-cmp-br-superinstruction.md` | 1 |
| `2026-04-30-elide-for-loop-exit-jmp.md` | 1 |
| `2026-05-02-codegen-test-wire-format-split.md` | 1 |

## Prefactoring

`vm-performance.md` §13 "Layer 1: Abstract Interpretation with Richer Domains"
lists interval analysis as a formal method that *could* eliminate runtime
checks, in the future tense, alongside array-bounds and division-by-zero
elimination.

A narrow form has since shipped. `for_loop_trunc_can_be_elided` in
`compile_stmt.rs` performs a local interval check over constant FOR-loop bounds
and elides the per-iteration `TRUNC` when every visible value of the control
variable stays inside the declared narrow type's range. That is interval
analysis applied to one specific check.

Four of this slice's citations describe that optimization. Repointing them to a
section that presents the technique as unbuilt would be misleading, so §13
gains a status note first — matching the convention §11 already uses for
CMP_BR.

The note records the limitation the benchmark comment already observes: the
elision covers the loop's own init and increment, not narrow stores in the
body.

## Triage

- **Repoint to §13** — the four TRUNC-elision citations, once the status note
  exists.
- **Repoint to §11** — the CMP_BR citation in the benchmark; §11 already
  carries the matching status note.
- **Delete** — the FOR-loop exit-JMP citation and the wire-format test citation.
  Both comments are self-contained: the first explains predicate inversion and
  the saved dispatch in the sentence that precedes the link, and the second is
  a three-point description of what the wire-format suite pins.

Exit-JMP elision gets no design-document section. It is a local codegen shape
decision fully explained where it is made, not a catalogue entry.

## Deferred work

None needing an issue. Two limitations appear in this cluster, both already
recorded in durable places:

- Narrow stores in a FOR-loop body are not covered by TRUNC elision — captured
  in the §13 status note added here.
- CMP_BR's remaining work (F32/F64, var-var comparisons, CASE-selector fusion,
  complex-condition WHILE) is already listed in §11 of the design document.

Neither is a defect; both are the documented edge of an optimization.

## File map

**Modify — design:**

- `specs/design/vm-performance.md` — §13 Layer 1 status note

**Modify — citations:**

- `compiler/codegen/src/compile_stmt.rs` (2)
- `compiler/codegen/tests/it/compile_loops.rs`
- `compiler/codegen/tests/it/end_to_end_loops.rs`
- `compiler/codegen/tests/it/wire_format.rs`
- `compiler/benchmarks/tests/profile_for_loop.rs` (2)

## Tasks

- [ ] Add the shipped-interval-analysis status note to `vm-performance.md` §13
- [ ] Repoint the 4 TRUNC-elision citations to §13
- [ ] Repoint the CMP_BR citation to §11
- [ ] Delete the exit-JMP and wire-format citations
- [ ] Confirm no citation in this cluster remains
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process introduced in #1456, this file is deleted before its own PR
merges. Its content is reviewable in the commit that adds it.
