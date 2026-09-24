# A temporal literal carries the type its prefix names

Issue: [#1560](https://github.com/ironplc/ironplc/issues/1560) (problem 2).
Follows PR [#1774](https://github.com/ironplc/ironplc/pull/1774) (merged), which
holds a count to the range its *storage* names; this holds a literal to the
range its *type* names, which is the half that needs the type to survive
parsing. Based on [#1780](https://github.com/ironplc/ironplc/pull/1780) for the
constructor this change adds a field to.

## Goal

`LDATE#`, `LDT#`, `LTIME#` and `LTOD#` name the 64-bit member of their family.
The parser discards which prefix was written, so every temporal literal resolves
to the 32-bit type. After this change a literal is the type its prefix names, a
semantic rule checks it against that type's range, and narrowing a long literal
into a short variable is a diagnostic.

## What this closes

**`LDATE#2200-01-01` is rejected although an `LDATE` holds it.** Issue #1560's
second problem. PR #1774 gave the 64-bit types their lowering path; the rule
still holds every date literal to the 32-bit ceiling because it cannot tell the
two apart.

**`ironplcc check` reports nothing for an out-of-range literal.** PR #1774's
check is in codegen, so `check` — what the editor integration runs — stays
silent and the failure appears only at build time. A rule fixes that, and the
codegen check then becomes an internal error, as the review of #1774 asked.

**`plc2plc` narrows a literal's type on round trip.** The renderer writes every
temporal literal with the short prefix, so `LDATE#2024-01-01` round-trips to
`DATE#2024-01-01` and `LTIME#30d` to `TIME#2592000000ms`. The second is now an
*out-of-range* `TIME` literal, so round-tripping an `LTIME` program is broken
until the renderer writes the prefix the literal names.

**The type checker compensates for the loss.** `type_compat::same_temporal_family`
makes `DATE` and `LDATE` interchangeable in *both* directions, and says why:

> Duration and date literals always resolve to the canonical short name
> regardless of the written form, so treat the two widths of a temporal family
> as interchangeable here.

That workaround is what lets a 64-bit value be assigned to a 32-bit variable
unchecked. Narrowing becomes a diagnostic; widening stays.

## Architecture

**Follow `CharacterStringLiteral`.** It carries `width: StringType`, and its doc
comment gives the reason, which transfers verbatim: *"The width belongs on the
literal and not only on the declaration it initializes because a literal also
appears in statement bodies, where there is no declaration to borrow it from."*
`xform_resolve_expr_types` reads `lit.width.keyword()` for a string while
hardcoding `"DATE"`, `"TIME"`, `"TIME_OF_DAY"` and `"DATE_AND_TIME"` for the
four temporal kinds. Closing that gap is the change.

**One struct per family with a width discriminant, not new literal types.**
STRING and WSTRING are one `CharacterStringLiteral` and one
`IntermediateType::String { char_width }`; the IEC-level types are already
distinct. Nothing wants an `LDateLiteral`.

**Conversion follows integers, not strings.** ADR-0034 makes STRING and WSTRING
mutually unconvertible because the *encodings* differ. The L-variants differ
only in width — ADR-0025's amendment established they "widen the storage rather
than raise the resolution" — so short to long widens and long to short narrows.

**The diagnostic follows a prefixed integer literal.** `rule_constant_range`
holds that "`INT#40000` is not an `INT` whatever it is stored into", so
`DATE#2200-01-01` stays out of range even when assigned to an `LDATE`, and the
message says to write `LDATE#`.

### Ranges after the change

| Type | Storage | Range |
|---|---|---|
| `TIME` | i32 ms | -2,147,483,648 to 2,147,483,647 ms (±24.8 days) |
| `LTIME` | i64 ms | ±292 million years |
| `DATE`, `DATE_AND_TIME` | u32 s | 1970-01-01 to 2106-02-07(-06:28:15) |
| `LDATE`, `LDT` | u64 s | 1970-01-01 to the parser's year 9999 ceiling |
| `TIME_OF_DAY`, `LTOD` | u32/u64 ms | 0 to 86,399,999 by construction |

`TIME_OF_DAY` and `LTOD` cannot exceed their storage, so they gain a type but no
range check.

## Prefactoring

**Done and split out.** Adding a width to `DurationLiteral` would have been
repeated at six construction sites, and `days`, `hours` and `minutes` were the
same function three times. Both were reshaped in
[#1780](https://github.com/ironplc/ironplc/pull/1780), which this branch is
based on, so the width field lands in one private constructor.

That reshaping also uncovered a defect unrelated to this change — `T#1.5h`
compiled to one hour and 1.8 milliseconds, and `T#1.5d` panicked the compiler —
which #1780 fixes. Nothing further is needed here.

**For the rest of this change, none.** The three remaining literal types have
one constructor each, `xform_resolve_expr_types` already reads a literal's own
type for the string case (the arm this change copies), and
`rule_date_literal_range` already keeps its bounds in one place by design.

## File map

| File | Change |
|---|---|
| `compiler/dsl/src/time.rs` | `TemporalWidth`; `width` on all four literals; `type_name()` per literal |
| `compiler/parser/src/parser.rs` | `duration()`, `time_of_day()`, `date()`, `date_and_time()` record which prefix matched |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | read the literal's type rather than hardcoding the short name |
| `compiler/analyzer/src/type_compat.rs` | `same_temporal_family` allows widening only |
| `compiler/analyzer/src/rule_date_literal_range.rs` | becomes `rule_temporal_literal_range`; bounds per type |
| `compiler/codegen/src/compile_expr.rs` | `within_storage` becomes an internal error |
| `compiler/plc2plc/src/renderer.rs` | write the prefix the literal names |
| `compiler/problems/resources/problem-codes.csv` | a code for narrowing a temporal literal |
| `specs/adrs/` | new ADR; dated amendment to ADR-0025 |
| `docs/reference/compiler/problems/` | P2038/P2039 ranges per type; the new code |
| `docs/reference/language/data-types/elementary/*.rst` | ranges per type |

## Tasks

- [ ] Prefactor: one private `DurationLiteral::new`
- [ ] Prefactor: collapse `days`/`hours`/`minutes`
- [ ] Fix the `hours` and `minutes` doctests
- [ ] `TemporalWidth` on the four literals; parser records the prefix
- [ ] `xform_resolve_expr_types` reads the literal's type
- [ ] `same_temporal_family` allows widening only; narrowing is diagnosed
- [ ] Rule checks each literal against its own type's range
- [ ] Codegen check becomes an internal error
- [ ] Renderer writes the literal's own prefix; round-trip tests
- [ ] ADR, ADR-0025 amendment, problem pages, type reference pages
- [ ] `cd compiler && just`, `cd specs && just`, docs build
- [ ] `git rm` this plan

## Known breakage to expect

- `apply_when_ldate_is_past_last_representable_then_error` and its `LDT` twin
  pin today's behaviour and become the passing case.
- Any source assigning a long literal to a short variable stops compiling. That
  is the point, but it is the part most likely to reach existing code, so the
  corpus and compatibility-library tests are the ones to watch.
