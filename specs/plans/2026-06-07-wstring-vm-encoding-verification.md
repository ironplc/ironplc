# Plan: PR C — VM data-driven string width + encoding verification

## Context

PR C of the WSTRING split, stacked on PR B1
(`claude/wstring-container-format-b1-phM0u`, the v3 container-format break).
Per the user's strategy the whole stack stays unmerged until a
DAP-coordinated format bump; C is built on B1 but not merged independently.

Design source: `specs/plans/2026-05-05-complete-wstring-support.md`
(Phases 2 + 3). ADR-0034 (operand typing + runtime encoding tags),
ADR-0035 (length-and-encoding-prefixed layout), ADR-0016 (UTF-16LE).

## Goal

Make the VM's string runtime **data-driven on `char_width`** (1 = narrow
STRING / Latin-1, 2 = wide WSTRING / UTF-16LE) and add the mandatory
**encoding-mismatch verification** (`Trap::EncodingMismatch`, V9014;
`Trap::InvalidCharWidth`, V9015 — both already exist from PR A).

After C the VM can correctly execute wide strings; it just has no
compiler-emitted source of them yet. Every wide path is proven with
hand-assembled synthetic bytecode in `vm/tests/it/`. PR D wires codegen to
actually emit wide strings end-to-end.

## Model (ADR-0035)

A string at data offset `O` with header `(max_len, cur_len, char_width=w)`:

- header is `STRING_HEADER_BYTES` (6) bytes: `[max_len u16][cur_len u16][char_width u16]`
- `max_len` / `cur_len` are **code units**, not bytes
- data lives at `O + STRING_HEADER_BYTES`, occupying `cur_len * w` bytes
- capacity is `max_len * w` bytes
- IEC string-function positions/lengths (LEFT/RIGHT/MID/FIND/…) are
  **code units**; a code-unit position `P` (1-based) maps to byte offset
  `(P-1) * w`

For `w = 1` every span equals today's byte math, so all existing STRING
behavior is unchanged.

## Non-goals (deferred)

- **No codegen changes, no opcode-format changes.** `STR_INIT` /
  `STR_INIT_ARRAY` keep their current operands and write `char_width = 1`
  (narrow). Adding a `char_width` operand + emitting real widths is PR D.
- No `iec_type_tag::WSTRING` wiring, no end-to-end WSTRING `.st` tests
  (PR E). C's wide coverage is synthetic-bytecode only.

## Where each operand's width comes from (operand typing, ADR-0034)

- **Constant** (`LOAD_CONST_STR`): the constant-pool entry's `const_type`
  → needs a new additive `ConstantPool` accessor returning the entry's
  `CharWidth` by index.
- **Variable / array element** (addressed by data offset): the
  data-region header's `char_width` field (populated by `STR_INIT`).
- **Temp buffer** (intermediate on the stack): a new
  `TempBufferSlot.encoding` field, recorded at allocation and carried
  through the slot table.

Each opcode reads its sources' widths, verifies they match (trap on
mismatch), scales byte spans by `w`, and writes `w` into any result
header / slot it produces.

## File map

| File | Change |
|------|--------|
| `compiler/container/src/constant_pool.rs` | Add `char_width(index) -> Result<CharWidth, _>` (and/or `get_str_with_width`) accessor; tests |
| `compiler/vm/src/string_ops.rs` | `CHAR_WIDTH_OFFSET`; `str_read_char_width`; header read/write helpers carry `char_width`; `TempBufferSlot.encoding`; `TempBufAllocator::alloc(width)` computes `max_len` in code units; encoding-verify helper |
| `compiler/vm/src/vm.rs` | Every string handler: resolve source width(s), verify, scale spans by `w`, write result width. Extract width-parameterized `do_*` helpers so dispatch arms stay short |
| `compiler/vm/tests/it/execute_string_ops.rs` | Add wide (`char_width = 2`) synthetic-bytecode tests + mismatch-trap tests for each opcode group |

## Implementation phases (sequential commits on one branch)

1. **Plan** — this file.
2. **Foundation** — `ConstantPool` width accessor; `string_ops.rs`
   `char_width` plumbing (offset const, read/write helpers, slot encoding,
   `alloc(width)`, a `verify_encoding(expected, actual)` helper). Narrow
   callers updated to pass `Narrow`; behavior unchanged. Crate tests green.
3. **Scalar ops** — `STR_INIT` (writes narrow width), `LOAD_CONST_STR`
   (width from pool → header + slot), `STR_STORE_VAR` (verify slot vs dest
   header, copy `cur_len * w`), `STR_LOAD_VAR` (width from header → slot),
   `LEN_STR` (returns `cur_len` code units — already correct once stores
   keep `cur_len` in units). Synthetic wide + mismatch tests.
4. **String functions** — `FIND/REPLACE/INSERT/DELETE/LEFT/RIGHT/MID/CONCAT`:
   verify all sources share `w`, interpret P/L as code units, scale byte
   spans by `w`, write `w` into the result. Width-parameterized `do_*`
   helpers. Synthetic wide tests.
5. **Arrays + conversions** — `STR_LOAD_ARRAY_ELEM` /
   `STR_STORE_ARRAY_ELEM` read the element header's `char_width`, scale the
   data copy by it, and `STORE` verifies the temp-buffer encoding against
   the element. `STR_INIT_ARRAY` stays narrow with a narrow stride: the
   array descriptor carries no `char_width` yet, so a per-array width and
   wide stride (`STRING_HEADER_BYTES + max_str_len * w`) land with the
   codegen work in **PR D**. `CMP_STR` (verify equal `w`, compare by code
   unit); `CONV_*_TO_STR` write narrow width (ASCII output is narrow by
   definition); `CONV_STR_TO_*` reject wide input. Synthetic tests.
6. **CI** — `cd compiler && just` green (compile, coverage ≥ 85%, lint).

> Implemented as three sequential commits on this branch: C1 (foundation +
> scalar/const/convert/compare), C2 (string functions), C3 (arrays).

## Verification

- All existing STRING end-to-end and `execute_string_ops` tests pass
  unchanged (narrow `w = 1` is byte-identical).
- New synthetic wide tests assert correct `cur_len` (code units), correct
  UTF-16LE byte spans, and correct results for each op group.
- New mismatch tests assert `Trap::EncodingMismatch` when a wide source is
  paired with a narrow destination (and vice versa), and
  `Trap::InvalidCharWidth` on a `char_width` header/operand of 0 or > 2.

## Risks

| Risk | Mitigation |
|------|-----------|
| Reinterpreting `cur_len`/`max_len` from bytes to code units changes narrow math | For `w = 1` `units == bytes`; assert via the unchanged existing test suite before adding wide tests |
| Missing a byte span that should scale by `w` | Phase-by-phase, each op group lands with a wide synthetic test that would fail if a span were unscaled |
| Temp-buffer capacity in code units vs bytes | `alloc(width)` sets `max_len = (cap - header) / width`; `cur_len * width <= max_len * width <= cap - header` holds by construction |
| Verification false positives on narrow programs | Narrow sources all carry `w = 1`; existing suite is the regression guard |
