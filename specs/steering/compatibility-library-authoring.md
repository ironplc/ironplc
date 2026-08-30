# Compatibility Library Authoring Policy

This steering file governs how **compatibility libraries** are authored so that
distributing them — including AI-generated shims — does not create copyright or
license problems, and so that "clean-room" provenance is *demonstrable through an
auditable record* rather than an unprovable claim.

> **Not legal advice.** This is an engineering risk-management policy. Decisions
> that redistribute third-party code (Tier C below) require review by the project
> owner and, where warranted, legal counsel.

## Applies To

Anyone — human or AI — adding or modifying a bundled compatibility library under
`compiler/sources/resources/libs/`. See the design in
[specs/design/compatibility-libraries.md](../design/compatibility-libraries.md).

## Why This Exists

IronPLC is MIT-licensed. Compatibility libraries reproduce vendor *interfaces*
(names, signatures) for interoperability and supply *bodies*. Interfaces and
math-dictated bodies carry little or no copyright; verbatim third-party source
(e.g. OSCAT) is license-encumbered and its terms can conflict with MIT.

Because much of the code is AI-generated, we **cannot prove a model never saw an
original** in training. So we do not rely on the classic "isolated implementer"
guarantee. Instead we demonstrate clean provenance by controlling the *inputs*,
clearing the *outputs*, and keeping the *record*.

## Non-affiliation

A compatibility library names a vendor and reproduces its interface as a matter
of interoperability, not endorsement. Bundled libraries and their distribution
carry this statement:

> IronPLC is an independent open-source project. It is not affiliated with,
> endorsed by, or sponsored by any third party.

The manifest `vendor` field is nominative (whose interface the library mirrors),
never a claim of affiliation. See
[Compatibility Library Format §Non-affiliation](../design/compatibility-library-format.md).

## Risk Tiers

Every bundled library is one of:

- **Tier A — facts / math-dictated.** Constants (`PI`, `e`) and IEC 61131-3
  standard behavior. A number is a fact; a math-dictated body has essentially one
  expression (idea/expression merge). Own authorship. Ships under MIT.
- **Tier B — clean-room interface shim.** Vendor names/signatures matched for
  interoperability, with bodies implemented as our own Rust VM intrinsics (or
  math-dictated ST), authored **from public documentation**. Ships under MIT.
- **Tier C — vendored third-party source** (e.g. OSCAT). Governed by the upstream
  license, **not** MIT. **Not distributed through this mechanism** — Tier C is a
  *separate distribution mechanism* with its own licensing. This mechanism
  **refuses** it: a `vendored` derivation or a non-permissive license is rejected,
  never bundled. (And it is never produced by feeding upstream source to an AI.)

## Allowed Inputs

When authoring Tier A/B (including any AI prompt), use only:

- IEC 61131-3 standard *behavior* (not verbatim text copied from the paid PDF).
- Public vendor API *documentation* you are licensed to read.
- The interface *signatures* themselves (names and types) — you may match these;
  matching the interface is the goal.

## Forbidden Inputs

Never feed to an AI tool, paste, or transliterate:

- Vendor implementation *source*, decompiled binaries, or exported `.library`
  files (the export's selection/arrangement is where thin interface copyright
  could bite — derive signatures from published docs instead).
- Any copyleft or otherwise encumbered source. This mechanism ships only Tier
  A/B; encumbered third-party source is never bundled here (it belongs to the
  separate Tier C distribution mechanism) and is never fed to a model.
- Verbatim text from the IEC 61131-3 standard document.

## Required Workflow (clean-room with AI)

1. **Spec from references.** Write a short interface/behavior spec from the
   allowed public references and **commit and merge it as its own non-squashed
   git entry** *before* implementation — a durable, squash-immune history record
   of what was authored from what.
2. **Implement from the spec only.** Generate the body from the spec. Prefer a
   *different medium* than the vendor's — a Rust VM intrinsic rather than ST — so
   the output structurally cannot be a copy of the vendor's source expression.
3. **Clearance.** If a licensed copy of the original is available, compare the
   output against it (comparison for clearance is not copying) and record the
   result.
4. **Record the references** used in the manifest (`references`) and commit them
   alongside the code. The reviewer and the spec commit are already in git
   history — do not duplicate them as manifest fields.

## Required Artifacts (per library)

- Manifest `references`: the public references the library was authored from —
  facts, not a legal classification. (No `derivation`, `license`, `reviewer`, or
  `attribution` field: the risk tier is a policy judgment, the reviewer and spec
  live in git history, and bundled Tier A/B ships under the repository's MIT
  license.)
- The clean-room spec, committed and **merged as its own non-squashed commit**.
- (Tier C is out of scope for this mechanism — see *Risk Tiers*.)

## Enforcement: automated vs. review

- **Automated (a conformance test — `sources_spec_req_cl_007_*` in
  `compiler/sources/src/spec_conformance.rs`).** Every
  manifest is well-formed and records a non-empty `references` list. The test
  verifies the record *exists and is well-formed* — it makes no legal judgment.
- **Review-only (cannot be tested).** That the declared provenance is *true* —
  that no forbidden input was used and clearance was actually performed. A
  reviewer confirms the record is *honest*. The test checks the shape; the human
  checks the truth.

## Reviewer Checklist

- The recorded `references` are public and within the allowed set; no forbidden
  input was used (attested).
- Bodies are own / math-dictated — not transliterations of vendor source.
- No Tier C content is being bundled here (encumbered third-party source belongs
  to the separate distribution mechanism).
- The clean-room spec was committed as its own non-squashed entry.
