# Time Literal Case-Insensitive Unit Suffixes

## Goal

Make IEC 61131-3 time literal unit suffixes (`d`, `h`, `m`, `s`, `ms`) case-insensitive so that `T#5S`, `T#100MS`, `T#1H30M`, `T#1D_2H` all parse. Today the parser only accepts lowercase unit suffixes, despite a code comment claiming otherwise (`compiler/parser/src/parser.rs:341`) and despite IEC 61131-3 being a case-insensitive language throughout.

## Architecture

Single-line change to the PEG grammar helper `dt_sep` in `compiler/parser/src/parser.rs`: replace the case-sensitive `t.text.as_str() == val` comparison with `t.text.eq_ignore_ascii_case(val)`. This also allows simplifying two existing call sites that listed both cases explicitly (`dt_sep("T") / dt_sep("t")`, `dt_sep("D") / dt_sep("d")`).

A new design document `specs/design/time-literals.md` specifies time literal syntax across IEC 61131-3 Editions 2 and 3, introducing `REQ-TL-NNN` requirement IDs. Tests adopt the existing `{area}_spec_req_{id}_{description}` naming convention from `specs/design/spec-conformance-testing.md`, linking each test to the requirement it verifies. ADR-0021 (TIME/LTIME width) and ADR-0022 (Edition 3 gating) remain unchanged.

## File Map

| File | Change |
|------|--------|
| `specs/design/time-literals.md` | **New** — design doc with REQ-TL-001 through REQ-TL-030 |
| `compiler/parser/src/parser.rs` | Make `dt_sep` ASCII case-insensitive; drop redundant `dt_sep("t")` / `dt_sep("d")` alternatives |
| `compiler/parser/src/tests.rs` | Add REQ-TL-### tests |
| `docs/reference/language/data-types/elementary/time.rst` | Remove `us` microsecond claim (not supported by parser); add case-insensitivity note |

## Tasks

- [x] Create plan
- [ ] Create `specs/design/time-literals.md` with REQ-TL-### requirements
- [ ] Change `dt_sep` rule to `eq_ignore_ascii_case` and simplify redundant alternatives
- [ ] Add tests for REQ-TL-002, REQ-TL-010, REQ-TL-011, REQ-TL-012, REQ-TL-021, REQ-TL-022, REQ-TL-023
- [ ] Remove `us` microsecond claim from `time.rst`
- [ ] Run `cd compiler && just` — all checks pass
