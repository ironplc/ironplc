# Design: Time Literals

## Overview

This design specifies the syntax and parsing semantics of IEC 61131-3 duration literals (time literals) in IronPLC. Duration literals denote intervals of time and may be assigned to `TIME` (Edition 2, 32-bit) or `LTIME` (Edition 3, 64-bit) variables.

The design builds on:

- **[ADR-0021: TIME as 32-bit and LTIME as 64-bit with Millisecond Precision](../adrs/0021-time-32bit-ltime-64bit.md)** — storage width, unit, and sub-millisecond truncation semantics
- **[ADR-0022: IEC 61131-3:2013 Compiler Flag for LTIME and Future Features](../adrs/0022-edition-3-compiler-flag.md)** — `allow_iec_61131_3_2013` gate for `LTIME`

## Design Goals

1. **Standard conformance** — IEC 61131-3 is a case-insensitive language for keywords and literal markers; time literals must accept every case combination.
2. **Edition coverage** — a single grammar spans Edition 2 (`T#` / `TIME#`) and Edition 3 (adds `LTIME#`); Edition 3 constructs are gated by the existing compiler flag.
3. **Testability** — every syntactic claim is a numbered requirement that a parser test can cite by ID.
4. **Narrow scope for precision** — precision and truncation belong to ADR-0021; this document only requires that the parser preserve the input value without loss.

## Scope

**In scope:** The grammar of duration literals: prefixes (`T`, `TIME`, `LTIME`), unit suffixes (`d`, `h`, `m`, `s`, `ms`), compound forms, the underscore visual separator, and the optional leading sign. Case-insensitive parsing for all tokens in the duration literal.

**Out of scope:**

- Sub-millisecond precision handling (owned by ADR-0021)
- Timer function blocks (TON, TOF, TP)
- Date, time-of-day, and date-and-time literals (separate grammar productions)
- Microsecond (`us`) and nanosecond (`ns`) unit suffixes — not defined by IEC 61131-3 for duration literals
- Build-time enforcement of the REQ-TL → test link (see Future Work)

---

## 1. Grammar

A duration literal has the form:

```
duration   = prefix '#' ['-'] interval
prefix     = 'T' | 'TIME' | 'LTIME'                    (case-insensitive)
interval   = scalar | compound
scalar     = fixed_point unit
compound   = integer unit ['_'] { integer unit ['_'] } [fixed_point trailing_unit]
unit       = 'd' | 'h' | 'm' | 's' | 'ms'              (case-insensitive)
```

Units in a compound interval appear in strictly descending magnitude order.

## 2. Prefix

**REQ-TL-001** A duration literal begins with one of the prefixes `T`, `TIME`, or `LTIME`, followed by `#`.

**REQ-TL-002** The prefix is recognized case-insensitively. `T#`, `t#`, `TIME#`, `time#`, `Time#` are equivalent; likewise every case variant of `LTIME#`.

**REQ-TL-003** The `LTIME` prefix is only accepted when `CompilerOptions::allow_iec_61131_3_2013` is `true`. When the flag is `false`, a post-tokenization validation rule rejects the `Ltime` token (per [ADR-0022](../adrs/0022-edition-3-compiler-flag.md)).

## 3. Unit Suffixes

**REQ-TL-010** The complete set of supported unit suffixes is exactly `d` (days), `h` (hours), `m` (minutes), `s` (seconds), and `ms` (milliseconds). Other unit names such as `us` or `ns` are rejected as parse errors.

**REQ-TL-011** Unit suffixes are recognized case-insensitively. Every case variant of a unit is accepted and produces the same `DurationLiteral`. For example, `T#5S`, `T#5s`, `T#500Ms`, `T#500MS`, `T#500mS`, and `T#500ms` are all valid; `T#1H30M30S` equals `T#1h30m30s`.

**REQ-TL-012** When scanning an interval, `ms` is matched before `m` so that a token like `100ms` is not misread as minutes followed by a stray unit.

## 4. Interval Forms

**REQ-TL-020** An interval may be a single scalar: an integer or fixed-point number followed by a unit (e.g., `T#1.5s`, `T#500ms`, `T#2D`).

**REQ-TL-021** An interval may be a compound sequence of parts whose units appear in strictly descending magnitude order (`d` > `h` > `m` > `s` > `ms`). All leading parts are integers; the trailing part may be fixed-point. Example: `T#1d2h30m15s500ms`. *(Specified but not currently implemented — see Future Work.)*

**REQ-TL-022** A compound interval may include `_` between adjacent parts as a visual separator. The separator is optional and has no semantic effect. Example: `T#1d_2h_30m` equals `T#1d2h30m`. *(Specified but not currently implemented — see Future Work.)*

**REQ-TL-023** An optional `-` sign between `#` and the first part negates the interval. Example: `T#-5s` represents negative five seconds.

## 5. Precision and Storage

**REQ-TL-030** The parser preserves the full literal value in a `DurationLiteral` without precision loss. Type-specific truncation — TIME to `i32` milliseconds and LTIME to `i64` milliseconds — happens in codegen as specified by [ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md).

## 6. Edition Coverage

| Edition | Prefixes available | Notes |
|---------|-------------------|-------|
| IEC 61131-3 Edition 2 (1993) | `T#`, `TIME#` | REQ-TL-001 through REQ-TL-002 and REQ-TL-010 through REQ-TL-023 apply |
| IEC 61131-3 Edition 3 (2013) | `T#`, `TIME#`, `LTIME#` | All REQs apply; REQ-TL-003 requires `allow_iec_61131_3_2013` to enable `LTIME#` |

Unit suffix case-insensitivity (REQ-TL-011) applies in both editions.

## 7. Test Mapping

Parser tests link to requirements via the existing `{area}_spec_req_{id}_{description}` naming convention (see [spec-conformance-testing.md](spec-conformance-testing.md)). Tests live in `compiler/parser/src/tests.rs`.

| Requirement | Test function |
|---|---|
| REQ-TL-002 | `duration_spec_req_tl_002_prefix_case_insensitive` |
| REQ-TL-010 | `duration_spec_req_tl_010_unsupported_unit_rejected` |
| REQ-TL-011 | `duration_spec_req_tl_011_unit_suffix_uppercase_accepted` |
| REQ-TL-011 | `duration_spec_req_tl_011_unit_suffix_mixed_case_accepted` |
| REQ-TL-012 | `duration_spec_req_tl_012_ms_matched_before_m` |
| REQ-TL-021 | `duration_spec_req_tl_021_compound_interval` (ignored — see Future Work) |
| REQ-TL-022 | `duration_spec_req_tl_022_compound_with_underscore` (ignored — see Future Work) |
| REQ-TL-023 | `duration_spec_req_tl_023_negative_duration` |

REQ-TL-001 and REQ-TL-020 are covered by existing parser tests for basic duration literals and fixed-point durations (e.g., `parse_program_when_fixed_point_duration_then_ok` in `compiler/parser/src/tests.rs`). REQ-TL-003 is covered by existing LTIME gating tests for [ADR-0022](../adrs/0022-edition-3-compiler-flag.md). REQ-TL-030 is an invariant verified in codegen and DSL tests rather than parser tests.

## 8. Implementation

The duration literal grammar lives in `compiler/parser/src/parser.rs`, section marked `// B.1.2.3.1 Duration`. A single PEG helper rule `dt_sep` matches identifier tokens whose text equals a given unit name (case-insensitively, per REQ-TL-011):

```rust
rule dt_sep(val: &str) -> &'input Token =
    [t if t.token_type == TokenType::Identifier && t.text.eq_ignore_ascii_case(val)]
```

The prefix productions `T` / `t` / `D` / `d` previously listed as explicit alternatives collapse to a single `dt_sep("T")` and `dt_sep("D")` respectively, since `dt_sep` is now case-insensitive.

## 9. Future Work

- **Compound intervals (REQ-TL-021, REQ-TL-022).** The grammar at `compiler/parser/src/parser.rs` defines compound intervals via rules like `days() = fixed_point d | integer d [_] hours()`, but pom's ordered-choice semantics always commit to the first alternative (`fixed_point d`) before reaching the compound branch, making compound intervals unreachable today. Fixing this requires restructuring the grammar (for example, adding a dedicated compound production before the scalar alternatives, or using `pom`'s longest-match combinator). Tests for REQ-TL-021 and REQ-TL-022 exist but are marked `#[ignore]` pending that grammar fix.
- **Build-time REQ-TL enforcement.** The `#[spec_test(REQ_XX_NNN)]` macro infrastructure in `compiler/container/build.rs` currently scans only `bytecode-container-format.md` and `bytecode-instruction-set.md`. Extending it (or adding analogous machinery to `compiler/parser/`) to scan `time-literals.md` would turn the Test Mapping table into a compile-time check. Today humans verify the mapping by reading this document.
- **Warning for sub-millisecond literals in TIME.** ADR-0021 notes that sub-millisecond durations truncate to zero for 32-bit TIME; a future analyzer pass could emit a diagnostic.
