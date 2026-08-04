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
`compiler/sources/resources/compat-libraries/`. See the design in
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

## Risk Tiers

Every bundled library is one of:

- **Tier A — facts / math-dictated.** Constants (`PI`, `e`) and IEC 61131-3
  standard behavior. A number is a fact; a math-dictated body has essentially one
  expression (idea/expression merge). Own authorship. Ships under MIT.
- **Tier B — clean-room interface shim.** Vendor names/signatures matched for
  interoperability, with bodies implemented as our own Rust VM intrinsics (or
  math-dictated ST), authored **from public documentation**. Ships under MIT.
- **Tier C — vendored third-party source** (e.g. OSCAT). Governed by the upstream
  license, **not** MIT. Quarantined, attributed, and added only after license
  review. Never produced by feeding upstream source to an AI to "reimplement."

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
- Any copyleft or otherwise encumbered source, **except** when it is being added
  deliberately as a license-reviewed Tier C library — and then it is vendored
  as-is, not fed to a model.
- Verbatim text from the IEC 61131-3 standard document.

## Required Workflow (clean-room with AI)

1. **Spec from docs.** Write a short interface/behavior spec from allowed inputs
   and commit it *before* implementation.
2. **Implement from the spec only.** Generate the body from the spec. Prefer a
   *different medium* than the vendor's — a Rust VM intrinsic rather than ST — so
   the output structurally cannot be a copy of the vendor's source expression.
3. **Clearance.** If a licensed copy of the original is available, compare the
   output against it (comparison for clearance is not copying) and record the
   result.
4. **Record provenance** in the manifest and commit it alongside the code.

## Required Artifacts (per library)

- Manifest provenance fields: `license`, `derivation`, `inputs` (docs cited),
  `attribution` (when the license requires it), `reviewer`.
- The committed spec doc (Tier B).
- For Tier C: the upstream `LICENSE` file, an attribution string, and placement
  in the quarantine location — never inside an MIT-licensed crate's own sources.

## Enforcement: automated vs. review

- **Automated (a conformance test — see the implementation plan).** Every
  manifest is well-formed and declares the required provenance; `derivation` and
  `license` are from the allowed sets; Tier C libraries are quarantined with a
  license and attribution. The test verifies the record *exists and is
  well-formed*.
- **Review-only (cannot be tested).** That the declared provenance is *true* —
  that no forbidden input was used and clearance was actually performed. A
  reviewer confirms the record is *honest*. The test checks the shape; the human
  checks the truth.

## Reviewer Checklist

- Inputs were within the allowed set; no forbidden input was used (attested).
- Bodies are own/ math-dictated/ properly vendored — not transliterations of
  vendor source.
- Tier C carries its upstream license + attribution and is quarantined from
  MIT-licensed crates.
- The manifest provenance fields are present and match what actually happened.
