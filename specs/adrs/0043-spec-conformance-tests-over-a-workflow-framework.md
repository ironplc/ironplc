# Spec Conformance Tests Over a Workflow Framework

status: accepted
date: 2026-04-10

## Context and Problem Statement

IronPLC keeps its architecture in `specs/` — ADRs for decisions, design
documents for what to build. Prose and code drift: a design document can
describe a field width, an opcode value or an error path that the
implementation stopped honouring, and nothing notices. By 2026-04 that had
already happened at least once in a wire format, where the container header's
`flags` bit 0 meant one thing in the design document and another in
`header.rs`.

Two shapes of answer were available. Adopt an existing spec-driven-development
framework, of which spec-kit was the most prominent, or build a mechanism that
ties individual claims in the design documents to individual tests.

## Decision Drivers

* **The failure mode is divergence, not disorganisation.** The documents exist
  and are written; what is missing is any signal when one stops being true.
* **Enforcement has to run in CI**, in the same command a contributor already
  runs, or it will not hold.
* **The specifications are implementation specifications, not models.** They
  describe a compiler and a bytecode VM that exist, in the vocabulary of the
  code.
* **Toolchain weight.** The compiler is Rust and its CI is `cargo`. A second
  language runtime in the build is a real cost.

## Considered Options

1. Adopt spec-kit.
2. Adopt a formal-methods tool — TLA+ or Alloy.
3. Generate code from the specifications.
4. Build requirement markers in the design documents, tied to tests.

## Decision Outcome

**Chosen: option 4** — requirement IDs embedded in the design documents,
bound to tests by an attribute macro, enforced bidirectionally.

**Spec-kit is a workflow framework, not a consistency framework.** It
structures how a feature is specified before it is built. It does not verify
that code still matches the specification afterwards, which is the whole of
the problem here. Its costs were also real: a Python dependency in a Rust
toolchain, and disruption to the existing document templates.

Its constitution concept had no gap to fill. **IronPLC's steering files
already function as the project's constitution** — the standing architectural
principles and development standards every change is held to. A second one
would have been a duplicate.

### What this decision rules out

* **Formal methods (TLA+, Alloy).** IronPLC's specifications are
  implementation specifications, not mathematical models. Requirement-bound
  tests get most of the value for a small fraction of the cost, and the
  artefacts stay readable by contributors who do not know a specification
  language.
* **Generating code from the specifications.** Code generation from Markdown
  is fragile, and it inverts the relationship: the specification would become
  a build input rather than a description that can be checked.
* **A big-bang audit.** Divergence is fixed opportunistically — when the
  surrounding code is being touched anyway — rather than by stopping to
  reconcile every document at once. `/project:reconcile-spec` exists to do
  exactly one section at a time.

## More Information

The mechanism itself — the `REQ-<AREA>-<crate-slug>-<NNN>` marker format, the
`#[spec_test(...)]` attribute, and the per-crate completeness meta-tests — is
specified in [cross-crate-spec-conformance.md](../design/cross-crate-spec-conformance.md)
and in the Design Requirement section of
[development-standards.md](../steering/development-standards.md).
[ADR-0037](0037-mandatory-crate-slug-in-requirement-ids.md) made the crate slug
mandatory.

Note that the completeness half of the enforcement is weaker than it reads: a
requirement counts as tested when the marker appears anywhere in the crate,
so an empty or ignored test body satisfies it.
