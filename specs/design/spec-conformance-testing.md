# Design: Spec Conformance Testing

## Overview

This design describes how IronPLC links design specifications to conformance
tests, ensuring that the code matches the spec and vice versa. It covers the
bytecode container format and instruction set specifications.

## Problem

Design specs and implementation can drift apart silently. A spec may document
hex value `0x05` while the code uses `0x03`, or the spec may add a new feature
without any test verifying it. Without a formal link between spec claims and
tests, divergence is invisible until it causes a bug.

## Approach

Every testable claim in a design spec gets a **requirement ID** (e.g.,
`**REQ-CF-001**`). Each requirement has a corresponding conformance test
annotated with `#[spec_test(REQ_CF_001)]`. Two build-time guarantees enforce
the link:

1. **Validity (compile-time):** If a requirement is removed from the spec, any
   test referencing it fails to compile.
2. **Completeness (test-time):** If a requirement is added to the spec without
   a corresponding test, the `all_spec_requirements_have_tests` meta-test fails.

## Requirement Numbering

| Prefix   | Spec Document                          | Example      |
|----------|----------------------------------------|--------------|
| REQ-CF   | `bytecode-container-format.md`         | REQ-CF-001   |
| REQ-IS   | `bytecode-instruction-set.md`          | REQ-IS-001   |

IDs use three-digit zero-padded numbers. Gaps are allowed (IDs are never
reused). Ranges are grouped by spec section with room between groups for
future additions.

## Spec Annotation

Each testable claim gets its own line, with the requirement ID first:

```markdown
**REQ-CF-001** The file header is exactly 256 bytes.
```

For tables, the requirement ID goes in a dedicated first column:

```markdown
| Requirement | Offset | Field | Type | Description |
|-------------|--------|-------|------|-------------|
| **REQ-CF-002** | 0 | magic | u32 | `0x49504C43` ("IPLC" in ASCII) |
```

Rules:
- At most one requirement per line.
- ID-first makes the association unambiguous: the ID labels the text to its right.
- Visible when reading the spec and searchable with grep.

## Enforcement Mechanism

### build.rs (compile-time constant generation)

`compiler/container/build.rs` parses the spec markdown files, extracts all
`**REQ-XX-NNN**` bold markers, and generates a Rust module containing one
constant per requirement and a list of all requirement IDs:

```rust
// Auto-generated from specs/design/*.md — do not edit
#[allow(dead_code)] pub const REQ_CF_001: &str = "REQ-CF-001";
// ...
pub const ALL: &[&str] = &["REQ-CF-001", ...];
```

The build script re-runs whenever the spec files or the test source file
change (`cargo:rerun-if-changed`).

### #[spec_test] proc-macro attribute

The `spec_test_macro` crate provides a `#[spec_test(REQ_CF_001)]` attribute
that:

1. Adds `#[test]` to the function.
2. Injects `let _ = crate::spec_requirements::REQ_CF_001;` at the top of the
   function body. This references the build-script-generated constant — if
   the constant does not exist (requirement removed from spec), the test fails
   to compile.

### Completeness meta-test

The build script also scans all `.rs` files under `src/` for `spec_test(REQ_`
patterns and generates an `UNTESTED` constant listing any requirements without
tests. A meta-test asserts that `UNTESTED` is empty. Tests can live in any
file within the crate — there is no single-file restriction.

## Test Conventions

- **Naming:** `{area}_spec_req_{id}_{brief_description}`
  (e.g., `container_spec_req_cf_001_header_size_is_256_bytes`)
- **`#[ignore]`:** Used for features specified but not yet implemented.
  The requirement marker is still present, satisfying the completeness check.
- **Code is king:** When spec and code disagree, the spec is updated to match
  the code, not vice versa. Exception: intentionally unimplemented features
  get `#[ignore]` tests.

## Example

In `bytecode-container-format.md`:
```markdown
**REQ-CF-001** The file header is exactly 256 bytes.
```

In `spec_conformance.rs`:
```rust
#[spec_test(REQ_CF_001)]
fn container_spec_req_cf_001_header_size_is_256_bytes() {
    assert_eq!(std::mem::size_of::<FileHeader>(), 256);
}
```

Removing `**REQ-CF-001**` from the markdown causes a compile error.
Adding `**REQ-CF-050**` without a test causes `all_spec_requirements_have_tests`
to fail.
