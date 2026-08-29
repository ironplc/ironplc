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

The work is ordered so the coupling goes first: every reference to
`specs/plans/` is removed before any plan is deleted, so deletion is
mechanical rather than breaking.

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

Phase 1 — removing every reference to `specs/plans/` — is the prefactoring.
It removes the coupling that makes plan files load-bearing, so that deleting
them later is a mechanical change rather than a breaking one. Nothing else
needs simplifying: the remaining work is additive to steering documents and
subtractive from `specs/plans/`.

## The new process

**Every non-trivial change starts with a GitHub issue.** The issue is the
durable record of the work, and it exists so that work cannot be forgotten.
Because the plan file is deleted at merge, anything a plan described but did
not deliver would otherwise disappear with it. The issue is what makes that
impossible. It is not an archive of the plan text.

The current backlog shows this failure mode in its existing form: 1,074
unticked checkboxes across 108 plans, and 46 plans where no box was ever
ticked. That work was written down, never done, and never surfaced again,
because nobody re-reads a merged plan. Deleting plan files without issues
would convert a quiet failure into a silent one.

**Single-PR work:**

1. Open an issue describing the work.
2. Write the plan to `specs/plans/YYYY-MM-DD-short-description.md`, referencing
   the issue.
3. Commit it as the first commit on the branch. Review it as a file diff.
4. Implement, following the plan.
5. Any decision worth keeping lands as an ADR or a `specs/design/` update **in
   the same PR**.
6. Anything the plan described that the PR does not deliver goes back onto the
   issue **before** the plan file is removed.
7. `git rm` the plan file before merge.
8. Close the issue only when nothing is outstanding.

**Multi-PR work** (43 of 237 existing plans use phase/slice language, so
roughly one change in five):

- The issue holds the overall plan and slice breakdown and stays open across
  the PR series, tracking which slices remain.
- Each PR commits only **its own slice's** plan, reviews it, and deletes it
  before merge.

No plan content reaches `main` in either case, and no PR needs a second
surviving commit. Because the repository squash-merges, the add and the delete
cancel within the squashed commit. The plan's add-commit also remains viewable
on the PR page — GitHub retains `refs/pull/*/head` permanently (1,354 such
refs on this repository today) — so the reviewed text is recoverable, but the
issue, not the PR page, is what carries unfinished work forward.

**Citing plans is prohibited.** Code comments, workflows, `justfile`, design
documents and ADRs must cite ADRs or design documents. A plan is a snapshot of
intent that is deleted at merge; it is never a stable reference target.

## File map

**Phase 1 — reference removal:**

- 77 comment sites across 11 crates under `compiler/`
- `specs/design/debugger-support.md` (6), `vm-performance.md`,
  `partial-access-bit-syntax.md`, `ref-to.md`,
  `library-interfaces/tc2-math.md`
- `specs/adrs/0033-opcode-encoding-by-class-and-type.md` (2)
- `.github/workflows/deployment.yaml`,
  `.github/workflows/partial_upload_release_artifacts.yaml`
- `justfile` (line ~310)
- Create: ADRs and design-document sections where triage requires them

**Phase 2 — process documentation:**

- `specs/steering/development-standards.md` — `specs/plans/` section (line
  ~30), location table row (line ~44), split-document rule (line ~47),
  Planning Requirement section (line ~83)
- `specs/steering/steering-file-guidelines.md` — line ~492 routing rule
- `CLAUDE.md` — workflow step 2, critical rule 2
- `CURSOR.md` — workflow step 2, critical rule 2
- `CONTRIBUTING.md` — "Planning Non-Trivial Changes" section
- `.cursor/rules/ironplc-steering.mdc` — line ~27

**Phase 3 — deletion:**

- Delete `specs/plans/*.md` (237 files)
- Create: ADRs and design-document sections where triage requires them

## Tasks

### Phase 1 — Remove every reference to `specs/plans/`

Each of the 92 citation sites gets one of two outcomes:

- **Not necessary** — delete the reference. Either the surrounding comment
  stands on its own without it, or the whole comment goes. This is the default
  and is expected to cover most sites: a citation exists because writing the
  link was easier than writing the reason, not because the reason was
  load-bearing.
- **Necessary** — the rationale genuinely needs a home. Write or extend an ADR
  (`specs/adrs/`) or a design document (`specs/design/`) and point the
  reference there instead.

Nothing is left pointing at `specs/plans/` when this phase completes.

- [ ] Add the "do not cite `specs/plans/`" rule to
      `development-standards.md` as the first change, so no new citations
      appear while this phase is in progress
- [ ] Triage the 40 cited plans; for each citation record **not necessary** or
      the ADR/design document that will carry it
- [ ] Migrate the 12 `specs/design/` and `specs/adrs/` citations first — a
      design document citing a plan is the clearest inversion, and resolving
      these establishes the pattern for the code comments
- [ ] Migrate `compiler/` comments by crate, largest first: codegen (11),
      analyzer (10), parser (7), vm-cli (6), plc2plc (5), sources (4), dsl (3),
      mcp (2), project, ironplc-cli, benchmarks (1 each)
- [ ] Migrate the 2 `.github/workflows/` citations and the 1 `justfile`
      citation
- [ ] Resolve the already-dangling `specs/plans/twincat-status.md` citation in
      `compiler/dsl/src/common.rs` — the plan it names does not exist
- [ ] Confirm `grep -rn "specs/plans" --exclude-dir=.git --exclude-dir=plans .`
      returns only process-documentation mentions
- [ ] `cd compiler && just` passes

### Phase 2 — Process change

- [ ] Rewrite the `specs/plans/` section of `development-standards.md` to
      describe plans as branch-local artifacts deleted before merge
- [ ] Update the Planning Requirement section with the single-PR and multi-PR
      flows described above, including the requirement that every non-trivial
      change opens an issue and that undelivered work returns to the issue
      before the plan file is removed
- [ ] Update the "Choosing the Right Location" table
- [ ] Update `steering-file-guidelines.md` routing rule
- [ ] Update `CLAUDE.md`, `CURSOR.md`, `CONTRIBUTING.md`,
      `.cursor/rules/ironplc-steering.mdc`
- [ ] Verify no remaining document instructs a reader to keep a plan committed

### Phase 3 — Triage and delete the backlog

Phase 1 already extracted the rationale from the 40 cited plans. This phase
covers the remaining ~197 that nothing references.

- [ ] Review the uncited plans for content that should be a design document or
      ADR but is not captured anywhere; the 11 plans over 25 KB and the 42
      between 10–25 KB are the highest-yield candidates
- [ ] Write the resulting ADRs and design-document sections
- [ ] Delete `specs/plans/*.md`
- [ ] Decide whether `specs/plans/` remains as an empty staging directory
      (with a `README.md` stating the lifetime rule) or is removed entirely
- [ ] `cd compiler && just` passes

## Sequencing

Phase 1 is the prerequisite for everything else: while 92 sites point at
`specs/plans/`, no plan can be deleted without breaking them. It is ~92
individual judgment calls and is best split across several PRs, one per crate
or document group.

Phase 2 lands after the references are gone and before the bulk deletion, so
the rules already describe plans as ephemeral by the time the backlog is
removed. Keeping it separate from Phase 3 matters for review: Phase 2 is a
small, careful diff that should not be buried inside a 237-file deletion.

Phase 3 is the deletion, gated on its own triage pass.

One consequence of this ordering worth accepting deliberately: plans continue
to accumulate at roughly 58 per month during Phase 1, and some will be new
plans. The first task of Phase 1 — prohibiting new citations — is what keeps
the phase from becoming a moving target; the extra plan files themselves are
swept up by Phase 3 regardless of how many arrive.

## Note

Per the process this plan introduces, this file is deleted before its own PR
merges.
