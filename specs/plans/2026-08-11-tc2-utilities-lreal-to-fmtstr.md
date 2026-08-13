# Implementation Plan: `Tc2_Utilities` Compatibility Library (`LREAL_TO_FMTSTR`)

## Goal

Implement the `Tc2_Utilities` compatibility library — one function,
`LREAL_TO_FMTSTR` — exactly as specified by the clean-room behavior
specification at
[specs/design/library-interfaces/tc2-utilities.md](../design/library-interfaces/tc2-utilities.md),
which is the **sole input** to this implementation per the
[Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md).

## Design doc reference

- [specs/design/library-interfaces/tc2-utilities.md](../design/library-interfaces/tc2-utilities.md)
  — the behavior spec (signature, normative fixed-point formatting semantics,
  domain, test vectors, package contents, acceptance criteria)
- [specs/design/compatibility-libraries.md](../design/compatibility-libraries.md)
  — the library mechanism (`REQ-CL-*`), including `REQ-CL-analyzer-004`
  (user declaration shadows an activated library declaration)

## Architecture

- **Library package as data.** A new bundled package
  `compiler/sources/resources/libs/Tc2_Utilities/` (manifest + one `.st`
  file), following the existing `Tc2_Math` layout. `LREAL_TO_FMTSTR` is a
  fully-defined ST body — no VM builtin, no manifest bindings table. Per the
  spec's *Implementation medium*, the body uses the `__TRUNC` compiler
  intrinsic, the standard string functions (`CONCAT`, `MID`), `LIMIT`, `ABS`,
  and `LREAL_TO_LINT` (which truncates toward zero); all digit production is
  exact 64-bit `LINT` arithmetic.
- **Formal parameter names.** The vendor's documented names (`in`,
  `iPrecision`, `bRound`) are kept verbatim so vendor source using formal
  (named) argument passing resolves unchanged. `in` is not a reserved word in
  the IronPLC lexer, so no parser change is needed.
- **Reference-activated only.** No discovery-module implicit list entry — the
  existing activation channels (`.plcproj` reference resolution and
  `--library`) already cover `Tc2_Utilities` by name with zero new code.
- **No new mechanism.** The shadowed-function filter, registry loader,
  `.plcproj` reference resolution, and end-to-end harness all exist from the
  `Tc2_Math` increment; this change is a new data package plus tests.

## ST body shape (from the spec's normative steps)

1. Domain check: `NOT (in = in)` (NaN) `OR ABS(in) >= 9.223372036854775808E18`
   (the binary64 value 2^63; also catches ±∞) → return `''`.
2. `p := LIMIT(0, iPrecision, 15)`.
3. Split on `ABS(in)`: integer part via `__TRUNC`, fraction by exact
   subtraction.
4. Scale by `10^p` (computed by a `FOR` loop of exact multiplications),
   round (`+ 0.5` then `LREAL_TO_LINT`) or truncate (`LREAL_TO_LINT`).
5. Carry: fraction `n = 10^p` increments the integer part and zeroes `n`.
6. Render: sign (`in < 0.0`; negative zero renders unsigned), integer digits
   by exact 64-bit `/ 10^k` / `MOD 10` arithmetic with
   `MID('0123456789', 1, digit + 1)` character lookup, then `'.'` plus the
   fraction rendered to exactly `p` digits (rendering `n` at fixed weights
   `10^(p-1) … 10^0` left-pads with zeros).

**Digit rendering is unrolled, not looped.** The VM sizes its temporary
string-buffer pool by statically counting string-operation call sites and
releases buffers only on function return, so a `CONCAT`/`MID` inside a loop
allocates per iteration and exhausts the pool (`TempBufferExhausted` trap).
Unrolled — one block per decimal weight, 19 for the integer part (intVal <
2^63 < 10^19) and 15 for the fraction, each guarded so it executes at most
once per call — the runtime allocation count is bounded by the static site
count, which is exactly the model the pool is sized for.

## File map

| File | Change |
|---|---|
| `compiler/sources/resources/libs/Tc2_Utilities/library.toml` | New — manifest (`name`, `vendor`, `default_version`, `references`; no bindings) |
| `compiler/sources/resources/libs/Tc2_Utilities/1.0.0/Tc2_Utilities.st` | New — the `LREAL_TO_FMTSTR` ST body |
| `compiler/sources/src/libraries/mod.rs` | Loader unit tests: registry contains/loads `Tc2_Utilities` |
| `compiler/codegen/tests/it/end_to_end_tc2_utilities.rs` | New — every spec test vector end-to-end (exact string equality), shadowing end-to-end, formal-argument call |
| `compiler/codegen/tests/it/main.rs` | Register the new test module |
| `compiler/ironplc-cli/resources/test/twincat_tc2_utilities_solution/…` | New fixture — solution referencing `Tc2_Utilities` |
| `compiler/ironplc-cli/tests/cli.rs` | `.plcproj` activation test + dormant-by-default negative test |
| `compiler/plc2plc/src/tests/tc2_utilities_calls.rs` | Round-trip test for source calling `LREAL_TO_FMTSTR` |
| `compiler/plc2plc/src/tests/mod.rs` | Register the new test module |

## Tasks

- [x] Commit this plan
- [x] Author `library.toml` and `Tc2_Utilities.st` from the spec
- [x] Loader tests: bundled registry contains and loads `Tc2_Utilities`
      (criterion 1)
- [x] End-to-end vector tests (criterion 2): every vector, exact string
      equality; NaN/∞ constructed arithmetically per the spec
- [x] Shadowing end-to-end test (criterion 4)
- [x] `.plcproj` activation fixture + positive/negative CLI tests
      (criterion 3)
- [x] `plc2plc` round-trip test (criterion 5)
- [x] Full CI green (`cd compiler && just`) (criterion 6)
