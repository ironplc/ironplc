# Make Plans Branch-Local in the Process Documentation

**Goal:** Rewrite the six documents that still instruct a reader to commit a
plan permanently, so the written process matches the one Phase 1 cleared the
way for: a plan is reviewed as a file diff, then deleted before merge. Add a
CI check so the citation rule holds without anyone remembering it.

**Architecture:** No code changes beyond one `justfile` recipe. The substance
is in `development-standards.md`; the other five documents point at it or
restate its rule in a sentence, so they change to match.

**Issue:** #1464 (Phase 2)

**Design doc reference:** None. Process change.

---

## Prefactoring

The prefactoring already happened: Phase 1 removed all 92 `specs/plans/`
citations, which is what makes deleting plan files possible at all. Nothing
further needs simplifying.

## The CI check

Phase 1 cleared 92 citations, and four new ones appeared on `main` while it
ran — from #1463, #1466 and #1469, all merged *after* #1456 added the
prohibition. The rule was written in a steering file; the work that broke it
was not reading steering files.

So the rule gets an enforcement mechanism: a `just` recipe that fails when
`specs/plans/` is referenced from anywhere except `specs/plans/` itself and the
process documents that describe the rule. It joins the `default` recipe
alongside `dupes`, so `cd compiler && just` catches a new citation before it
reaches review.

This is the part of Phase 2 that decides whether Phase 1's result is permanent
or a snapshot. Without it the count refills; with it, it cannot.

**Assumption stated for review:** this was raised four times without a
decision. It is included because the alternative is a rule that has already
been shown not to hold, and it is one recipe — easy to drop from this PR if
unwanted.

## The process, as it will read

Every non-trivial change starts with a GitHub issue. Then:

1. Write the plan to `specs/plans/YYYY-MM-DD-short-description.md`,
   referencing the issue.
2. Commit it as the first commit on the branch. It is reviewed as a file diff.
3. Implement.
4. Any decision worth keeping lands as an ADR or a `specs/design/` update in
   the same PR.
5. Anything the plan describes that the PR does not deliver becomes a tracked
   issue before the plan file is removed.
6. `git rm` the plan file before merge.
7. Close the issue only when nothing is outstanding.

Because the repository squash-merges, the add and the delete cancel within the
squashed commit, so no plan content reaches `main`. The plan stays viewable on
the PR, and the issue — not the PR — carries anything unfinished.

For multi-PR work the issue holds the slice breakdown and stays open across the
series; each PR commits only its own slice's plan.

## File map

**Modify:**

- `specs/steering/development-standards.md` — the `specs/plans/` section, the
  location table, the split-document rule, and the Planning Requirement
- `specs/steering/steering-file-guidelines.md` — the routing bullet
- `CLAUDE.md` — workflow step 2, critical rule 2
- `CURSOR.md` — workflow step 2, critical rule 2
- `CONTRIBUTING.md` — "Planning Non-Trivial Changes"
- `.cursor/rules/ironplc-steering.mdc` — the non-negotiables bullet
- `compiler/justfile` — new `plan-citations` recipe, added to `default`

## Tasks

- [ ] Add the `plan-citations` recipe and wire it into `default`
- [ ] Verify it fails on a planted citation and passes on a clean tree
- [ ] Rewrite the `specs/plans/` section and Planning Requirement in
      `development-standards.md`
- [ ] Update the location table and the split-document rule
- [ ] Update `steering-file-guidelines.md`
- [ ] Update `CLAUDE.md`, `CURSOR.md`, `CONTRIBUTING.md`, `.cursor/rules/`
- [ ] Confirm no document still tells a reader to keep a plan committed
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process this documents, this file is deleted before its own PR merges.
Its content is reviewable in the commit that adds it.
