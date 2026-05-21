# Introduce CharWidth Enum Implementation Plan

**Goal:** Introduce a typed `CharWidth { Narrow = 1, Wide = 2 }` enum in `ironplc-container` as a standalone, inert addition. No on-disk format changes, no callers added in this PR — the enum sits in place ready for follow-up WSTRING work to consume it.

**Architecture:** Pure additive. One new module (`compiler/container/src/char_width.rs`) defining the enum, one new variant on `ContainerError` for invalid byte values, and one re-export from `lib.rs`. The format version, header layout, and constant-pool wire format are all unchanged.

**Context:** Carved out of PR #1050 (WSTRING support) to make that review tractable. PR #1050 lands the enum mid-stream as part of a later refactor; landing it standalone first lets follow-up PRs use it from the start instead of going through a `u8` interim.

**Tech Stack:** Rust, `ironplc-container` crate (no_std-compatible)

---

### Task 1: Add the `CharWidth` enum module

**Files:**
- Add: `compiler/container/src/char_width.rs`

The enum has two discriminants matching the eventual on-disk encoding tag (`Narrow = 1`, `Wide = 2`) so `width as u8` round-trips. Helpers: `byte_width()`, `as_usize()`, `is_wide()`, `from_u8(u8) -> Result<Self, ContainerError>`. Unit tests cover valid bytes, invalid bytes returning `InvalidCharWidth`, byte-width matching the discriminant, and `is_wide` discrimination.

### Task 2: Add `ContainerError::InvalidCharWidth(u8)`

**Files:**
- Modify: `compiler/container/src/error.rs`

Add the variant, the `Display` arm, and a test for the message. Place the variant alongside the other invalid-tag variants (`InvalidConstantType`, `InvalidTaskType`, `InvalidFieldType`).

### Task 3: Wire up the module and re-export

**Files:**
- Modify: `compiler/container/src/lib.rs`

Add `mod char_width;` to the always-available section (the enum is `no_std`-compatible), and `pub use char_width::CharWidth;` alongside the other always-available re-exports.

---

## Deliberately out of scope

- `FORMAT_VERSION` bump (stays at 2)
- `STRING_HEADER_BYTES` change (stays at 4)
- `ConstType::WStr` variant
- Any caller in container, VM, codegen, or analyzer
- Problem code V9015 (`Trap::InvalidCharWidth`) — that's a VM-side concern landed with the runtime checks

## Verification

`cd compiler && just` must pass: compile, coverage ≥ 85%, clippy, fmt, dupes. The new module's own unit tests cover its surface; no integration tests are added because the enum has no callers yet.
