# Plans as Ephemeral Artifacts

**Goal:** Stop committing implementation plans as permanent repository
content. Keep the plan-review gate that makes plans valuable, move durable
rationale into ADRs and design documents where it belongs, and let plan files
be deleted before merge so `specs/plans/` stops accumulating.

**Architecture:** Plans keep their current authoring and review flow — a plan
file is still the first commit on a feature branch, still reviewed as a file
diff with line comments. Only its lifetime changes: the branch deletes the
file before merge. Because the repository squash-merges (0 merge commits in
history; every commit is PR-numbered), the add and the delete cancel within
the squashed commit and no plan content lands on `main`.

Durable content is redirected rather than discarded. Anything in a plan worth
keeping is an ADR (`specs/adrs/`) or a design document (`specs/design/`), both
of which already exist and already carry this load.

**Design doc reference:** None. This is a process change; no system design is
affected.

---

## Background

The evidence that plans are not functioning as durable documents:

| Measure | Value |
|---|---|
| Plan files in `specs/plans/` | 237 (2.3 MB, ~13% of tracked text) |
| Plans touched by exactly one commit | 228 of 237 |
| Lines added / deleted across all history | 42,837 / 95 |
| Checkboxes checked / unchecked | 458 / 1,074 |
| Plans with checkboxes where none were ever ticked | 46 |
| Plan lines inside code fences (duplicated implementation) | 23% |
| Plans added in August 2026 alone | 58 |

Plans are written once, never corrected, and their task state does not
describe what shipped. Meanwhile `specs/adrs/` (46 records) and
`specs/design/` (36 documents) hold the content that is actually re-read.

## Why plans currently cannot simply be deleted

92 citation sites across 40 distinct plan files point at `specs/plans/` from
code and documents that are not plans:

| Source | Sites |
|---|---|
| `compiler/**/*.rs` comments | 77 (11 crates; codegen 11, analyzer 10, parser 7, vm-cli 6, plc2plc 5) |
| `specs/design/` and `specs/adrs/` | 12 (`debugger-support.md` alone: 6) |
| `.github/workflows/` | 2 |
| `justfile` | 1 |

These exist because the rationale was never written anywhere durable, so the
plan became load-bearing by default. One citation
(`specs/plans/twincat-status.md`) is already dangling — the plan it names does
not exist. A design document citing an implementation plan is backwards and is
the clearest signal of the problem.

Migrating these citations is a precondition for deleting anything, and is the
prefactoring step below.

## Prefactoring

The citation migration (Phase 2) **is** the prefactoring: the existing
references must be moved onto durable documents before plan files can be
removed, otherwise deletion breaks 92 sites. No other simplification is
needed — the change is additive to steering docs and subtractive from
`specs/plans/`.

## The new process

**Single-PR work:**

1. Write the plan to `specs/plans/YYYY-MM-DD-short-description.md`.
2. Commit it as the first commit on the branch. Review it as a file diff.
3. Implement, following the plan.
4. Any decision worth keeping lands as an ADR or a `specs/design/` update **in
   the same PR**.
5. `git rm` the plan file before merge.

The squashed commit contains the implementation and any ADR or design change,
and no plan. The plan's add-commit remains viewable on the PR page — GitHub
retains `refs/pull/*/head` permanently (1,354 such refs on this repository
today), so the reviewed artifact survives without a manual archiving step.

**Multi-PR work** (43 of 237 existing plans use phase/slice language, so
roughly one change in five):

- A GitHub **issue** holds the overall plan and slice breakdown. Its job is
  coordination across the PR series, not archival.
- Each PR commits only **its own slice's** plan, reviews it, and deletes it
  before merge.

No plan content reaches `main` in either case, and no PR needs a second
surviving commit.

**Citing plans is prohibited.** Code comments, workflows, `justfile`, design
docs and ADRs must cite ADRs or design documents. A plan is a snapshot of
intent that is deleted at merge; it is never a stable reference target.

## File map

**Modify — process documentation:**

- `specs/steering/development-standards.md` — rewrite `specs/plans/` section
  (line ~30), the location table row (line ~44), the split-document rule (line
  ~47), and the Planning Requirement section (line ~83)
- `specs/steering/steering-file-guidelines.md` — line ~492 routing rule
- `CLAUDE.md` — workflow step 2, critical rule 2
- `CURSOR.md` — workflow step 2, critical rule 2
- `CONTRIBUTING.md` — "Planning Non-Trivial Changes" section
- `.cursor/rules/ironplc-steering.mdc` — line ~27

**Modify — citation migration:**

- 77 comment sites across 11 crates under `compiler/`
- `specs/design/debugger-support.md` (6), `vm-performance.md`,
  `partial-access-bit-syntax.md`, `ref-to.md`,
  `library-interfaces/tc2-math.md`
- `specs/adrs/0033-opcode-encoding-by-class-and-type.md` (2)
- `.github/workflows/deployment.yaml`,
  `.github/workflows/partial_upload_release_artifacts.yaml`
- `justfile` (line ~310)

**Create:** ADRs or design-doc sections as the triage requires.

**Delete:** `specs/plans/*.md` (237 files), after triage.

## Tasks

### Phase 1 — Process change

- [ ] Rewrite the `specs/plans/` section of `development-standards.md` to
      describe plans as branch-local artifacts deleted before merge
- [ ] Update the Planning Requirement section with the single-PR and multi-PR
      flows above
- [ ] Add the rule prohibiting citations of `specs/plans/` from any durable
      file
- [ ] Update the "Choosing the Right Location" table
- [ ] Update `steering-file-guidelines.md` routing rule
- [ ] Update `CLAUDE.md`, `CURSOR.md`, `CONTRIBUTING.md`,
      `.cursor/rules/ironplc-steering.mdc`
- [ ] Verify no remaining doc instructs a reader to keep a plan committed

### Phase 2 — Citation migration (prefactoring)

Each citation is a judgment call: fold the rationale into an existing ADR or
design doc, write a new one, or inline the explanation at the call site if it
is narrow enough not to warrant a document.

- [ ] Triage the 40 cited plans; record for each whether its rationale becomes
      an ADR, a design-doc section, or an inline comment
- [ ] Migrate `specs/design/` and `specs/adrs/` citations first (12 sites) —
      these are the most clearly wrong and unblock the rest
- [ ] Migrate `compiler/` comments by crate, largest first: codegen (11),
      analyzer (10), parser (7), vm-cli (6), plc2plc (5), sources (4), dsl (3),
      mcp (2), project, ironplc-cli, benchmarks (1 each)
- [ ] Migrate `.github/workflows/` (2) and `justfile` (1) citations
- [ ] Resolve the dangling `specs/plans/twincat-status.md` citation in
      `compiler/dsl/src/common.rs`
- [ ] Confirm zero remaining `specs/plans/` references outside `specs/plans/`
      itself

### Phase 3 — Backlog triage and deletion

- [ ] Review all 237 plans for content that should be a design document or
      ADR but is not yet captured anywhere; the 11 plans over 25 KB and the 42
      between 10–25 KB are the highest-yield candidates
- [ ] Write the resulting ADRs and design-doc sections
- [ ] Delete `specs/plans/*.md`
- [ ] Decide whether `specs/plans/` remains as an empty staging directory
      (with a `README.md` explaining the lifetime rule) or is removed entirely

### Verification

- [ ] `cd compiler && just` passes
- [ ] `grep -rn "specs/plans" --exclude-dir=.git .` returns only the intended
      process-documentation mentions

## Sequencing

Phase 1 is independent and can merge on its own — it stops the growth
immediately. Phase 2 is the prerequisite for Phase 3 and is best split across
several PRs, since it is ~92 individual judgment calls. Phase 3 is a single
delete once triage is complete.

## Note

Per the process this plan introduces, this file is deleted before its own PR
merges.
