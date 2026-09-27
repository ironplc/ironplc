# Calendar functions: CONCAT_DATE, CONCAT_TOD, CONCAT_DT, DAY_OF_WEEK

## Goal

Add the standard functions that build a date, a time of day or a date and
time from its components, and the day of the week of a date: rows 2a, 3a,
4a and 8 of IEC 61131-3 Table 36 "Additional functions of time data types
CONCAT and SPLIT":

| Function | Inputs | Result |
|---|---|---|
| `CONCAT_DATE` | `YEAR`, `MONTH`, `DAY` : `ANY_INT` | `DATE` |
| `CONCAT_TOD` | `HOUR`, `MINUTE`, `SECOND`, `MILLISECOND` : `ANY_INT` | `TIME_OF_DAY` |
| `CONCAT_DT` | `YEAR`, `MONTH`, `DAY`, `HOUR`, `MINUTE`, `SECOND`, `MILLISECOND` : `ANY_INT` | `DATE_AND_TIME` |
| `DAY_OF_WEEK` | `IN` : `DATE` | `USINT`, 0 for Sunday to 6 for Saturday |

Each `ANY_INT` input takes its own integer type: the table's example of
`CONCAT_DT` passes a `USINT` day among literals. `YEAR` is of a type of at
least 16 bits (NOTE 1 of the table), so a `SINT` or `USINT` year is
rejected.

Today every one of them is P4017 ("Function is not declared") with
`ironplcc` 0.246.0 and `main` at `50a0528c`.

## Non-goals

- `SPLIT_DATE`, `SPLIT_TOD`, `SPLIT_DT`: they return their components through
  `VAR_OUTPUT` parameters, and a function call with an output argument is not
  implemented (P9999, `compiler/analyzer/src/rule_function_call_type_check.rs:269`).
  They follow once function outputs are supported; an issue records them.
- The long forms of Table 36: `CONCAT_DATE_LTOD`, `CONCAT_LDATE_TOD`,
  `CONCAT_LDATE_LTOD` (rows 1b to 1d), `CONCAT_LDATE` (2b), `CONCAT_LTOD`
  (3b), `CONCAT_LDT` (4b), and `SPLIT_LDATE`, `SPLIT_LTOD`, `SPLIT_LDT`.
  They take or produce long types, reachable only with
  `--allow-long-time-types`, and follow in a second PR, as the typed time
  functions derived their long forms. That PR settles whether
  `CONCAT_LTOD` also takes `MICROSECOND` and `NANOSECOND` (`SPLIT_LDT`
  gives them; NOTE 3 of the table lets the implementer add such inputs).
  `DAY_OF_WEEK` has a `DATE` input only.

## Architecture

The four names are IEC 61131-3 surface, so by ADR-0042 (rule 3) the compiler
seeds them. Their bodies are integer arithmetic, so no builtin opcode, no VM
change and no container format change are needed: the code generator emits
the instruction sequence inline, as `compile_time_arith.rs` does for
`ADD_DT_TIME` and `SUB_DT_DT`.

- **Analyzer.** The four signatures join `other_time_functions`
  (`compiler/analyzer/src/intermediates/stdlib_time_function.rs`), with
  `ANY_INT` inputs as `MUX` declares its selector
  (`compiler/analyzer/src/intermediates/stdlib_function.rs:519`).
- **Code generation.** A new module `compiler/codegen/src/compile_calendar.rs`
  with one routine per function, dispatched from `compile_function_call`
  (`compiler/codegen/src/compile_call.rs`) by one line beside the existing
  `time_arith_for` dispatch.
- **Representation** (ADR-0025): `DATE` and `DATE_AND_TIME` in `u32` seconds
  since 1970-01-01, `TIME_OF_DAY` in `u32` milliseconds since midnight.
  - `CONCAT_DATE`: the number of days since 1970-01-01 of the proleptic
    Gregorian date, times 86 400. The day count uses the civil-to-days
    computation of the Gregorian calendar (400-year eras, March-based years),
    written from its definition, in `i64` so that no intermediate overflows.
  - `CONCAT_TOD`: `((HOUR * 60 + MINUTE) * 60 + SECOND) * 1000 + MILLISECOND`.
  - `CONCAT_DT`: `CONCAT_DATE` of the date, plus the seconds of the time;
    the milliseconds are dropped, since `DATE_AND_TIME` stores whole seconds
    (as `CONCAT_DATE_TOD` documents).
  - `DAY_OF_WEEK`: `(days + 4) MOD 7`, 1970-01-01 being a Thursday (4)
    and 0 being Sunday; `DATE` is unsigned, so the remainder is never
    negative.

## Decisions for the design document

These are recorded in `specs/design/calendar-functions.md` with requirement
IDs, and need the maintainer's agreement before the implementation PR:

1. **Invalid components** (month 13, 30 February, hour 25, a year before
   1970 or after 2106). Options: (a) normalise, as the arithmetic does by
   itself (`CONCAT_DATE(2023, 2, 30)` is 2 March 2023); (b) trap with a new
   runtime error; (c) saturate. Recommendation (a judgement, to discuss): (a),
   documented, because it needs no new runtime error code; and, as a
   separate check, the analyzer could report a call whose components are
   all constants and invalid, as it already reports date literals outside
   their storage (P2039). No public source gives the standard's rule for
   invalid components; implementations differ: RuSTy returns 0 for
   components that do not form a date (`CONCAT_DATE(2024, 13, 45)`) and
   clamps to the `DATE` range (its test `concat_invalid_inputs.st`).

Settled by the table:

2. **Result type of `DAY_OF_WEEK`: `USINT`.** The table declares an
   `ANY_INT` result whose type the implementer specifies (NOTE 2), and its
   example assigns it to a `USINT`. `USINT` holds 0..6 and widens
   implicitly to every larger integer type (ADR-0029), so the example and
   an assignment to `INT` or `DINT` both compile; `INT` would reject the
   example as an implicit narrowing.
3. **Numbering of the days: 0 for Sunday, 1 for Monday, ..., 6 for
   Saturday**, as the table states.

Also for the design document: the four names are IEC 61131-3 surface
and, like the long forms of the typed time functions, are
registered in every dialect (ADR-0042: flags gate syntax, not names).

## Prefactoring

None needed, and why: the change adds four signatures to an existing table
and one dispatch line; the routines go into a new module, so no existing
function grows. `compile_call.rs` is already above the 1000-line limit
(1212 lines); this change adds one line to it and does not make the split
of that module part of this work — it is a separate, larger prefactor that
this plan should not hide.

## Design doc reference

`specs/design/calendar-functions.md` (new), requirement IDs
`REQ-CAL-analyzer-NNN` (signatures, constant checks) and
`REQ-CAL-codegen-NNN` (values, including 29 February, century years, the
limits of the `u32` range, and the day of the week of 1970-01-01).

## File map

- `specs/design/calendar-functions.md` — new, with the decisions above
- `compiler/analyzer/build.rs`, `compiler/codegen/build.rs` — list the design
- `compiler/analyzer/src/intermediates/stdlib_time_function.rs` — signatures
- `compiler/codegen/src/compile_calendar.rs` — new
- `compiler/codegen/src/compile_call.rs` — one dispatch line
- `compiler/codegen/src/lib.rs` — module
- `compiler/codegen/tests/it/end_to_end_calendar.rs` — new, and
  `compiler/codegen/tests/it/main.rs`
- `compiler/codegen/src/spec_conformance_*.rs` / analyzer conformance tests
- `docs/reference/standard-library/functions/{concat_date,concat_tod,concat_dt,day_of_week}.rst`
  — new pages in the format of `concat_date_tod.rst`, with playground examples
- `docs/reference/standard-library/functions/index.rst` — the four entries

## Tasks

- [x] Rows, parameter names, result types and day numbering from Table 36
- [ ] Open the issue for `SPLIT_DATE`, `SPLIT_TOD`, `SPLIT_DT` (blocked by
      function outputs)
- [ ] Design document with the decisions and requirement IDs; agreement of
      the maintainer on decision 1
- [ ] Tests first: the examples of the table (`CONCAT_DATE(2010, 3, 12)`,
      `CONCAT_TOD(16, 33, 12, 0)`, `CONCAT_DT(2010, 3, Day, 12, 33, 12, 0)`
      with `Day : USINT`, `DAY_OF_WEEK(DATE#2010-03-10)` into a `USINT`,
      which is 3); a `SINT` year rejected; end-to-end tests of known dates (1970-01-01, 2000-02-29,
      2024-02-29, 2100-03-01, 2106-02-07), times (00:00:00.000,
      23:59:59.999), days of the week, and normalised components
- [ ] Analyzer signatures and their conformance tests
- [ ] `compile_calendar.rs` and the dispatch line
- [ ] Documentation pages
- [ ] `cd compiler && just`; docs build without warnings
- [ ] Remove this plan
