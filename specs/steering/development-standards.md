# IronPLC Development Standards

This steering file defines the core development standards and patterns for the IronPLC project, a Rust-based PLC compiler implementing the IEC 61131-3 standard.

> **Note**: This file provides detailed implementation guidance for AI-assisted development. For development workflow, setup instructions, and contribution processes, see the main [CONTRIBUTING.md](../../CONTRIBUTING.md) and component-specific contributing guides.

## Project Structure

IronPLC consists of four primary components:

1. **Compiler** (`compiler/`) - The core Rust compiler with multiple crates
2. **VS Code Extension** (`integrations/vscode/`) - Language server and IDE integration
3. **Documentation Website** (`docs/`) - Sphinx-based documentation
4. **Interactive Playground** (`playground/` frontend, `compiler/playground/` WASM crate) - Browser-based IEC 61131-3 editor and runner at playground.ironplc.com, embedded in documentation pages via custom Sphinx directives

**Critical**: The build will fail if the compiler, VS Code extension, and documentation website get out of sync. Always ensure version numbers, problem codes, and language features are synchronized across those three components.

## Component-Specific Standards

This file holds the cross-component process and conventions that apply to *all*
work. Standards specific to one component live in their own steering files, which
load only when you touch that component's files:

- **[compiler-standards.md](compiler-standards.md)** — Rust coding standards for `compiler/**` (module structure, testing, error handling, performance, `unsafe`)
- **[doc-standards.md](doc-standards.md)** — documentation website standards for `docs/**` (quadrants, writing style, RST roles)
- **[extension-standards.md](extension-standards.md)** — VS Code extension standards for `integrations/vscode/**`

## Specs Directory Structure

The `specs/` directory contains internal technical documentation organized into four folders:

### `specs/adrs/` — Architecture Decision Records

Trade-off analyses that capture **why** a particular approach was chosen over alternatives. ADRs are numbered (`0000-topic.md`) and are permanent records — they are not updated after the decision is made.

### `specs/design/` — Design Documents

Specifications that describe **what** to build: architecture, formats, interfaces, algorithms, data structures, and component interactions. A design document answers "what does this system look like?" without prescribing the step-by-step work to get there.

### `specs/plans/` — Implementation Plans

Work breakdowns that describe **how** to implement: phased task lists, specific code changes, file modifications, and verification steps. A plan document answers "what steps do I follow to build this?" Plans reference the design they implement.

**A plan is branch-local.** It is committed as the first commit on a feature branch so it can be reviewed as a file diff, and deleted from that branch before merge. Because the repository squash-merges, the add and the delete cancel within the squashed commit, so no plan content reaches `main`. The plan stays viewable on the pull request.

Plans are the one document type in `specs/` that is not durable. Anything worth keeping — a decision, a constraint, a piece of rationale — must land somewhere durable in the same pull request: `specs/adrs/`, `specs/design/`, or a code doc comment where the reasoning is local to the code (see [Choosing the Right Location](#choosing-the-right-location)).

**Never cite a plan from anywhere else.** Code comments, workflows, the `justfile`, design documents and ADRs must cite an ADR or a design document, never `specs/plans/`. A plan is deleted before its own pull request merges, so a reference to one is either already dead or about to be. `just plan-citations` enforces this and runs as part of `just`.

### `specs/steering/` — AI Steering Files

Guidance for AI assistants working with the codebase (conventions, patterns, workflows). See [steering-file-guidelines.md](./steering-file-guidelines.md).

### Choosing the Right Location

| Question | Location |
|----------|----------|
| Why did we choose approach X over Y? | `specs/adrs/` |
| What should the container format look like? | `specs/design/` |
| Why is *this function* written this way? | a doc comment on it |
| What are the steps to implement the container format? | `specs/plans/` (deleted before merge) |
| How should AI assistants name tests? | `specs/steering/` |

**A decision may live in a code comment.** Rationale whose reader is the next
person editing that function or file belongs next to the code, not in `specs/`
— a deliberate divergence from the obvious implementation, why a branch exists,
why a simpler shape was not used. Moving it to a design document makes it worse:
further from the code it constrains, and easier to leave behind when the code
moves.

Reserve `specs/` for what a code comment cannot hold: a decision that constrains
a subsystem, spans crates, or picks between alternatives someone will re-litigate.

**Important**: Plan and design documents must **never** be placed in `docs/`. The `docs/` directory is exclusively for the public Sphinx documentation website. All internal technical documents (plans, designs, ADRs, steering files) belong in `specs/`.

### Design Requirement

Compiler design documents in `specs/design/` **must** include **requirement IDs** for every testable claim. Every ID carries a **mandatory crate slug** naming the crate that owns its conformance test — `**REQ-<AREA>-<crate-slug>-<NNN>**`:

```markdown
**REQ-CF-container-001** The file header is exactly 256 bytes.
```

For tables, add a Requirement column as the first column:

```markdown
| Requirement | Offset | Field | Type | Description |
|-------------|--------|-------|------|-------------|
| **REQ-CF-container-002** | 0 | magic | u32 | `0x49504C43` ("IPLC" in ASCII) |
```

Rules:
- At most one requirement per line
- ID-first: `**REQ-<AREA>-<crate-slug>-<NNN>**` followed by the testable claim
  - `<AREA>` is an uppercase code grouping requirements by design section (`CF`, `EN`, …)
  - `<crate-slug>` is the lowercase owning-crate slug (`CARGO_PKG_NAME` minus `ironplc-`; may contain hyphens, e.g. `vm-cli`)
- The unslugged form (`**REQ-CF-001**`) is **not** valid; a listed doc containing one panics the build
- One design doc may distribute requirements across several crates by giving each requirement the owning crate's slug (e.g. `REQ-EN-codegen-001` and `REQ-EN-container-061` in the same doc)
- IDs use three-digit zero-padded numbers grouped by section with gaps for future additions
- IDs are never reused; gaps are allowed

Each requirement **must** have a corresponding conformance test annotated with `#[spec_test(REQ_<AREA>_<crate_slug>_NNN)]` in the owning crate. The build system enforces this bidirectionally: removing a requirement from the spec causes a compile error; adding a requirement without a test causes that crate's completeness meta-test to fail.

The completeness half is weaker than it reads. A requirement counts as tested when its marker appears anywhere in the crate, so an empty or `#[ignore]`d body satisfies it without asserting anything. Write a real assertion; a marker on an empty test is worse than no marker, because it reports the requirement as covered.

**Fix divergence opportunistically.** When a design document and the code disagree, reconcile that section as part of whatever work brought you there, rather than scheduling an audit of everything. `/project:reconcile-spec` does one section at a time.

See [Spec Conformance Testing](../design/spec-conformance-testing.md) for the full enforcement mechanism, and [ADR-0043](../adrs/0043-spec-conformance-tests-over-a-workflow-framework.md) for why this mechanism rather than a spec-driven-development framework.

## AI Development Process

A person is accountable for all changes. We use a custom process for all non-trivial features and changes to ensure human review.

### Required Steps

**Planning**

1. AI researches the code base and desired changes
2. AI creates a **plan branch**, writes the plan to `specs/plans/YYYY-MM-DD-short-description.md`, and creates a PR for the plan.
3. A person reviews and provides feedback on the plan until the plan is approved. This PR is never merged.

**Prefactoring**
4. AI creates one or more **prefactor branches**, implements any pre-factoring, and creates PRs for prefactors.
5. A person reviews, provides feedback and merges the prefactor PRs.

**Core Change**
6. If the change requires one or more **core change PRs**, then AI creates a GitHub issue detailing the planned work. The issue is the durable record so that we complete all work in the plan.
7. AI creates one or more **core change branches**, implements the changes, and creates PRs for the changes.
8. A person reviews, provides feedback and merges the core change PRs.

**Cleanup**
9. AI discards the plan branch.
10. If there was an associated GitHub issue, then AI closes the GitHub issue.

AI can help write code but all  **must** use the following process so that someone can review.

### Planning Document

A plan document should include:

- **Goal** — a concise statement of what the change accomplishes
- **Architecture** — brief summary of the technical approach
- **Prefactoring** — the simplifications to make *before* adding the new
  behaviour, or an explicit statement that none is needed and why (see
  [Prefactoring](#prefactoring))
- **Design doc reference** — link to `specs/design/` doc if one exists
- **File map** — which files will be created or modified
- **Tasks** — ordered steps with checkboxes (`- [ ]`) for tracking progress

Name plan files with a date prefix: `YYYY-MM-DD-short-description.md` (e.g., `2026-04-01-planning-requirement.md`).

### Prefactoring

**Prefactoring** is refactoring done *before* new behaviour is added: reshape the
existing code so the new behaviour drops in, then add it. It is the opposite
order from the more familiar "make it work, then clean it up" — and it is the
order this project uses.

Every change **must** start by looking for related prefactoring opportunities t
prevent complexity creep and avoid the need for premature abstractions.

#### Signals that a change needs prefactoring

Look for these while reading the code you are about to modify. Any one of them
means stop and reshape first:

- The new behaviour needs a new `match` arm or `if` in **more than one place** —
  the distinction wants to be a type or a data table, not repeated branching
- You would copy an existing function and change a few lines of it
- The new tests would duplicate an existing test's setup wholesale, or you would
  need a combinatorial matrix of tests to cover how the new flag interacts with
  the existing ones
- The module would cross the 1000-line limit (see
  [compiler-standards.md](compiler-standards.md#code-organization)) once the change
  lands
- A similar bug could occur rather than being prevented at compile time

#### How to prefactor

1. **Change the shape, not the behaviour.** The existing tests must pass
   unchanged. If they have to be edited to accept the prefactoring — beyond
   mechanical renames — the commit is not behaviour-preserving; split it.
2. **Commit the prefactoring separately.** A reviewer can then read a diff that
   provably changes nothing, followed by a smaller diff that adds the feature.
   Either can be reverted alone.

#### When *not* to prefactor

Prefactoring is a tool for reducing the cost of the change in hand, not a
licence to rewrite:

- **No speculative generality.** Do not build an abstraction for a case nobody
  has asked for. Extract a shared shape when the second or third caller arrives,
  not the first.
- **No unbounded rewrites.** If the reshaping is far larger than the feature,
  write it up as its own plan and change, and land the feature the simple way
  in the meantime — with a note saying so.
- **Not for one-line fixes.** Typos, dependency bumps, and single-line bug fixes
  stay single-line.

### What good looks like

A well-prefactored change shows up as a *smaller* feature diff and *fewer* new
tests than the same feature added on top of the old shape — because there are
fewer distinct paths to cover, not because anything went untested. The coverage
gate (see [common-tasks.md](common-tasks.md#coverage-requirements)) still applies unchanged.

## Code Organization

### Avoid Duplication

**Do not repeat content that has a single source of truth.** This applies to
prose and configuration as much as to code: a fact stated in two places
drifts, and a reader has no way to tell which copy is stale.

In Rust, factor shared behaviour into a function, trait, or shared crate
rather than copying it between crates.

In the documentation website, the mechanisms for sharing are, in order of
preference:

1. **`.. include::` a file in `docs/includes/`** for a paragraph or admonition
   that appears verbatim on more than one page. Reach for this as soon as the
   second copy appears — most of the duplication found in `docs/` was text
   that already had an include, added after the copies.
2. **A substitution** (`.. |name| replace::`) when the shared text varies by a
   word or two between pages, such as a flag name or a keyboard shortcut.
3. **A cross-reference** (`:doc:`, `:ref:`) when the reader should be sent to
   the authoritative page instead of being shown the text again.

Duplication that is acceptable, and should be left alone:

- **Generated content.** Problem summaries come from the compiler via
  `problem-summary`; the compiler is the source of truth.
- **Parallel reference pages.** Sibling entries such as `CTU`/`CTD` or
  `TON`/`TOF` describe symmetric behaviour in symmetric sentences. Each page
  must stand alone for a reader who arrives from search.
- **Worked examples repeated across quadrants.** The same program may appear
  in an explanation and in a reference page; the reader of either should not
  have to navigate away to see it.

`cd docs && just duplicates` checks this. See
[docs/CONTRIBUTING.md](../../docs/CONTRIBUTING.md) for what the check can and
cannot currently see.

## Steering Files for AI Assistants

IronPLC uses **steering files** to guide AI assistants in working with the codebase. These files follow a specific two-file pattern:

- **Pointer files** in `.kiro/steering/` - Lightweight references loaded automatically by Kiro
- **Detailed docs** in `specs/steering/` - Complete guidance that works with any AI system

### Creating and Maintaining Steering Files

When creating or updating steering files:

1. **Use the two-file pattern** - Create detailed doc in `specs/steering/`, pointer in `.kiro/steering/`
2. **Keep pointers minimal** - 3-5 lines with a reference to the detailed doc
3. **Make detailed docs self-contained** - Should work when copied to any AI system
4. **Update CLAUDE.md** - Add references to new steering files
5. **Choose appropriate inclusion** - `always`, `fileMatch`, or `manual`

For complete guidance on steering files, see [steering-file-guidelines.md](./steering-file-guidelines.md).

## Build System Integration

IronPLC uses `just` as its command runner. The full command reference — per
component, coverage, packaging, and troubleshooting — lives in
[common-tasks.md](common-tasks.md). Do not restate it here.

### Git Workflow and Pre-PR Quality Gate

**NEVER commit or push directly to `main`.** Create a feature branch and open a
pull request so CI validates every change before it reaches `main`. Before
creating any PR, run and pass the full pipeline:

```bash
cd compiler && just
```

See [common-tasks.md](common-tasks.md#critical-pre-pr-requirements) for what this
runs and how to fix failures. The clippy-suppression rule lives in
[compiler-standards.md](compiler-standards.md#code-quality).

### Version Management
**Version numbers are generated and incremented automatically** — never edit them
manually (see [common-tasks.md](common-tasks.md#version-management)).

### Synchronization Checks
The build system enforces synchronization between components:
- Version numbers must match across all components
- Problem codes must be documented
- Examples in docs must have corresponding tests

### Cross-Platform Support
The compiler, extension, and playground all target Windows, macOS, and Linux.
Component-specific cross-platform guidance (e.g. the compiler's `just` recipes)
lives in the relevant [component standards file](#component-specific-standards).
