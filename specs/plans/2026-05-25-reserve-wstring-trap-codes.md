# Reserve WSTRING Trap Codes (V9014, V9015) Implementation Plan

**Goal:** Reserve the two runtime trap variants that the encoding-aware string opcodes will raise once the WSTRING wire format and VM opcodes land. This PR is purely additive — no callers, no behavior changes — so follow-up patches can return these traps from the start instead of going through `Trap::InvalidInstruction` or similar placeholders.

**Architecture:** Pure additive. Two new `Trap` variants (`EncodingMismatch { expected, actual }` and `InvalidCharWidth(u8)`), their `Display` arms, two new rows in `problem-codes.csv` (which the existing `build.rs` turns into `v_code()`/`exit_code()` arms), and two new problem-code documentation pages. The `ContainerError::InvalidCharWidth(u8)` counterpart is already on `main` from PR #1070, so no container change is required.

## Design Doc Reference

- ADR-0034 — STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags (defines the defense-in-depth encoding-mismatch check)
- ADR-0035 — Length-and-Encoding-Prefixed String Memory Layout (defines the `char_width` byte in the string header / constant-pool entry / temp-buffer slot whose unknown values raise V9015)

## File Map

| File | Change |
|------|--------|
| `compiler/vm/src/error.rs` | Add `Trap::EncodingMismatch { expected: u8, actual: u8 }` and `Trap::InvalidCharWidth(u8)`; add their `Display` arms and unit tests |
| `compiler/vm/resources/problem-codes.csv` | Add `V9014,EncodingMismatch,...,struct` and `V9015,InvalidCharWidth,...,true` rows (consumed by `build.rs`) |
| `docs/reference/runtime/problems/V9014.rst` | New problem-code page for the encoding-mismatch trap |
| `docs/reference/runtime/problems/V9015.rst` | New problem-code page for the unrecognized `char_width` trap |
| `docs/reference/runtime/problems/index.rst` | Add V9014 + V9015 entries (the Sphinx extension regenerates this file at build time; checking in the updated copy keeps the repo consistent without requiring a docs build) |

## Tasks

- [ ] Add `Trap::EncodingMismatch { expected: u8, actual: u8 }` and `Trap::InvalidCharWidth(u8)` variants to `compiler/vm/src/error.rs`
- [ ] Add `Display` arms for both variants
- [ ] Add unit tests for `Display`, `v_code()`, and `exit_code()` for both variants
- [ ] Add V9014 and V9015 rows to `compiler/vm/resources/problem-codes.csv`
- [ ] Write `docs/reference/runtime/problems/V9014.rst`
- [ ] Write `docs/reference/runtime/problems/V9015.rst`
- [ ] Update `docs/reference/runtime/problems/index.rst`
- [ ] `cd compiler && just` — full CI green

## Out of Scope

- Any caller that raises either trap (lands with the VM string opcode PR that follows this one)
- The `STRING_HEADER_BYTES` 4 → 6 / `FORMAT_VERSION` 2 → 3 bump (lands in the format-bump PR)
- The `ConstType::WStr` variant and constant-pool wire-format change (lands in the format-bump PR)

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Adding an unused enum variant trips `dead_code` lint | The existing `Trap` enum is matched exhaustively in `Display`, `v_code`, and `exit_code`, so each new variant has a use site; `cargo clippy` will catch any remaining issue |
| V9015 row format ("true" vs "struct") mismatched against the variant shape | `InvalidCharWidth(u8)` is a tuple variant, so the CSV column is `true`; verified by the test that exercises `v_code()` and `exit_code()` |

## Verification Strategy

- `cargo test -p ironplc-vm` — new tests pass
- `cd compiler && just` — full pipeline green
