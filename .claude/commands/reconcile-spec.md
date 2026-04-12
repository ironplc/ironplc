# Reconcile Spec with Implementation

Reconcile one small section of a design spec with its implementation. This adds
requirement IDs, fixes spec text to match code (code is king), and writes
spec_test functions. See [spec-conformance-testing.md](../../specs/design/spec-conformance-testing.md)
for the full design of the enforcement mechanism.

## Procedure

### Step 1: Understand the infrastructure

Read `specs/design/spec-conformance-testing.md` to understand:
- Requirement ID format (`**REQ-XX-NNN**`)
- Annotation rules (ID-first, one per line, bold markers)
- Enforcement mechanism (build.rs generates constants, proc macro references them)
- Test naming convention

### Step 2: Pick one small section

Read the target spec file. Find a section that describes testable behavior but
lacks `**REQ-XX-NNN**` markers. Pick **ONE** small, concrete subsection:
- A single struct/record format (e.g., one 4-byte entry)
- A single encoding table (e.g., one enum's value assignments)
- A single behavioral rule

Never pick an entire major section in one pass.

Find the highest existing requirement ID in the spec:

```bash
grep -oP '\*\*REQ-\w+-\d+\*\*' <spec-file> | sort -u | tail -1
```

If no IDs exist yet, determine the correct prefix from
`spec-conformance-testing.md` or propose a new one following the convention.

### Step 3: Find the implementation

Search the codebase for the types, constants, or functions described in the
chosen spec section. Use grep/glob to locate the source file(s) that implement
it. Read **only** the relevant source file(s) -- not the entire crate.

### Step 4: Find the spec conformance infrastructure

Search for the `build.rs` that references this spec file:

```bash
grep -rl '<spec-filename>' compiler/*/build.rs
```

If found, identify the crate and its conformance test module (look for
`spec_conformance` in its `lib.rs`). If not found, set up the infrastructure
by following the pattern in `spec-conformance-testing.md`:
- Add a `build.rs` that scans the spec markdown for `**REQ-XX-NNN**` markers
- Add a `spec_test_macro` dependency to `Cargo.toml`
- Add `spec_requirements` and `spec_conformance` test modules to `lib.rs`
- Add the `all_spec_requirements_have_tests` completeness meta-test

Use `compiler/container/build.rs` as a working reference implementation.

### Step 5: Compare spec vs code

For each struct/enum/constant in the implementation:
- Check field names, types, and byte sizes against the spec
- Check enum variant values against the spec encoding table
- Note any fields/variants in code but not in spec, or vice versa

**Code is king.** If the code differs from the spec, the spec must be updated
to match the code. Exception: if a spec feature is intentionally unimplemented,
use `#[ignore]` on the test.

### Step 6: Update the spec

- Assign the next available `**REQ-XX-NNN**` ID(s)
- Place each ID on its own line (ID-first) or in a table's Requirement column
- If code diverges from spec, update the spec text to match the code
- Keep changes minimal -- only touch the section being reconciled

### Step 7: Write spec_test functions

Add tests to the conformance test file. Follow the naming convention from
`spec-conformance-testing.md`:

```
{area}_spec_req_{id}_{brief_description}
```

Each test should verify the specific claim its requirement makes. Prefer
compile-time or structural assertions (size_of, constant values, round-trip
serialization) over behavioral tests when possible.

### Step 8: Run CI and commit

```bash
cd compiler && just
```

All checks must pass (compile, coverage, lint). If the
`all_spec_requirements_have_tests` meta-test fails, ensure every `**REQ-XX-NNN**`
in the spec has a matching `#[spec_test(REQ_XX_NNN)]`.

Commit message format:

```
spec: reconcile {section name} with implementation (REQ-XX-NNN through REQ-XX-MMM)
```

## Token Efficiency

- Find the highest existing ID with grep; do not re-read entire specs
- Read only the source file(s) for the chosen section
- Pick one small subsection per invocation
- Reference `spec-conformance-testing.md` for mechanism details rather than
  re-discovering them
