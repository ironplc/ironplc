# A temporal literal carries the type its prefix names

Issue: [#1560](https://github.com/ironplc/ironplc/issues/1560) (problem 2), plus
the same defect in the other three temporal families — see
[Scope](#scope-all-four-families).

## Goal

`LDATE#`, `LDT#`, `LTIME#` and `LTOD#` name the 64-bit member of their family.
The parser discards which prefix was written, so every temporal literal resolves
to the 32-bit type, and the compiler holds a 64-bit literal to a 32-bit range.
After this change a literal is the type its prefix names, is checked against
that type's range, and narrowing it to the short type is a diagnostic.

## What is broken today

**A 64-bit date literal is held to the 32-bit ceiling.** `LDATE#2200-01-01` is
rejected as P2038 although an `LDATE` holds it. This is issue #1560's second
problem.

**A `TIME` literal past 24.8 days is silently corrupted.** Not previously
recorded. `t : TIME := T#30d;` compiles clean in the *default* dialect and the
VM reads back **-1,702,967,296 ms**, about -19.7 days, because
`whole_milliseconds()` is truncated into the `i32` a `TIME` is stored in with
nothing checking the range. This is the same class of defect as #1560 and
reaches more users, since it needs no Edition 3 flag. Verified by running the
compiled program.

**`plc2plc` narrows a literal's type on round trip.** The renderer writes every
temporal literal with the short prefix, so `LDATE#2024-01-01` round-trips to
`DATE#2024-01-01` and `LTIME#30d` to `TIME#2592000000ms`. That second one
becomes an *out-of-range* `TIME` literal once the range check above exists, so
the renderer is not optional cleanup — round-tripping an `LTIME` program breaks
without it. The string renderer already gets this right
(`character_string_text(&node.width, …)`).

**The type checker compensates for the loss.** `type_compat::same_temporal_family`
makes `DATE` and `LDATE` interchangeable in *both* directions, and says why:

> Duration and date literals always resolve to the canonical short name
> regardless of the written form, so treat the two widths of a temporal family
> as interchangeable here.

That is a workaround for this defect, and it is what currently lets a 64-bit
value be assigned to a 32-bit variable unchecked.

## Architecture

**Follow `CharacterStringLiteral`.** It carries `width: StringType` and its doc
comment gives the reason, which transfers verbatim: *"The width belongs on the
literal and not only on the declaration it initializes because a literal also
appears in statement bodies, where there is no declaration to borrow it from."*
`xform_resolve_expr_types` then reads `lit.width.keyword()` for a string while
hardcoding `"DATE"`, `"TIME"`, `"TIME_OF_DAY"` and `"DATE_AND_TIME"` for the
four temporal kinds. Closing that gap is the whole change.

**One struct per family with a width discriminant, not new literal types.**
STRING and WSTRING are one `CharacterStringLiteral` and one
`IntermediateType::String { char_width }`; the IEC-level types are already
distinct (`ElementaryTypeName::{DATE, LDATE, …}`, `IntermediateType::Date { size }`).
Nothing wants an `LDateLiteral`.

**Take conversion from integers, not from strings.** ADR-0034 makes STRING and
WSTRING mutually unconvertible because the *encodings* differ. The L-variants
differ only in width — ADR-0025's amendment established they "widen the storage
rather than raise the resolution" — so the integer relationship applies: short
to long widens, long to short narrows. Narrowing is a diagnostic, not a silent
truncation.

**Take the diagnostic shape from a prefixed integer literal.**
`rule_constant_range` already holds that "`INT#40000` is not an `INT` whatever
it is stored into". So `DATE#2200-01-01` stays out of range even when assigned
to an `LDATE`, and the message can say to write `LDATE#` instead.

### Ranges after the change

| Type | Storage | Range |
|---|---|---|
| `TIME` | i32 ms | -2,147,483,648 to 2,147,483,647 ms (±24.8 days) |
| `LTIME` | i64 ms | ±292 million years |
| `DATE`, `DATE_AND_TIME` | u32 s | 1970-01-01 to 2106-02-07(-06:28:15) |
| `LDATE`, `LDT` | u64 s | 1970-01-01 to the parser's year 9999 ceiling |
| `TIME_OF_DAY`, `LTOD` | u32/u64 ms | 0 to 86,399,999 by construction |

`TIME_OF_DAY` and `LTOD` cannot exceed their storage, so they gain a type but no
range check. The unsigned families keep the epoch as a lower bound at both
widths.

## Prefactoring

`compile_time_count` range-checks against `u32` unconditionally, which is right
for one of the six storage shapes. It already receives `OpType` — a width *and*
a signedness, which `type_info` fills in correctly (`DATE` unsigned, `TIME`
signed) — so it can check the count against the range the operation type
actually names. That alone turns the `T#30d` truncation into a diagnostic and
makes the date guard width-correct, with no change to the type system. It lands
first, on its own.

## Design doc reference

No existing design document covers temporal literals. ADR-0025's amendment
states that "an LDATE or LDT *literal* is still bounded by the 2106 ceiling that
u32 seconds imposes", which this change makes false, so it needs a dated
amendment. The decision to type a literal by its prefix, and to allow widening
while rejecting narrowing, is a new one that spans parser, analyzer, codegen and
plc2plc — that wants an ADR.

## File map

### PR 1 — range-check against the operation type

| File | Change |
|---|---|
| `compiler/codegen/src/compile_expr.rs` | `compile_time_count` checks the count against the range `OpType` names |
| `compiler/problems/resources/problem-codes.csv` | a problem code for an out-of-range duration, or widen P2038's wording |
| `compiler/codegen/tests/it/end_to_end_date.rs` | the `T#30d` case and the width/signedness matrix |
| `docs/reference/compiler/problems/` | the new or amended page |

### PR 2 — the literal carries its type

| File | Change |
|---|---|
| `compiler/dsl/src/time.rs` | `TemporalWidth` on all four literals; `type_name()` per literal |
| `compiler/parser/src/parser.rs` | `duration()`, `time_of_day()`, `date()`, `date_and_time()` record which prefix matched |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | read the literal's type instead of hardcoding the short name |
| `compiler/analyzer/src/type_compat.rs` | `same_temporal_family` becomes directional widening |
| `compiler/analyzer/src/rule_date_literal_range.rs` | bounds per type; renamed for all four families |
| `compiler/plc2plc/src/renderer.rs` | write the prefix the literal names |
| `specs/adrs/` | new ADR; dated amendment to ADR-0025 |
| `docs/reference/language/data-types/elementary/*.rst` | ranges per type |
| `docs/reference/compiler/problems/P2038.rst` | the "64-bit types have the same range" section inverts |

## Tasks

- [ ] PR 1: range-check against `OpType`'s width and signedness
- [ ] PR 1: tests for every (width, signedness) boundary, including `T#30d`
- [ ] PR 2: `TemporalWidth` on the four literals; parser records the prefix
- [ ] PR 2: `xform_resolve_expr_types` reads the literal's type
- [ ] PR 2: `same_temporal_family` allows widening only
- [ ] PR 2: rule checks each literal against its own type's range
- [ ] PR 2: renderer writes the literal's own prefix; plc2plc round-trip tests
- [ ] PR 2: ADR, ADR-0025 amendment, problem pages, type reference pages
- [ ] `cd compiler && just` and `cd specs && just` on each
- [ ] `git rm` this plan

## Known breakage to expect

- `apply_when_ldate_is_past_last_representable_then_error` and
  `apply_when_ldt_is_past_last_representable_then_error` pin today's behaviour
  and become `rule_ok!`.
- Any source assigning a long literal to a short variable stops compiling. That
  is the point of the change, but it is the part most likely to reach existing
  code, so the corpus and compatibility-library tests are the ones to watch.
