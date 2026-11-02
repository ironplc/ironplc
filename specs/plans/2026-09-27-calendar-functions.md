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
seeds them. What `CONCAT_DATE`, `CONCAT_TOD` and `CONCAT_DT` do with
components that do not form a value of their result type is left to the
implementer and differs between targets, so it is a behavior policy
(ADR-0049, decision 1 below): each of the three compiles to `BUILTIN` with a
func_id that encodes the selected alternative, as `STRING_TO_<numeric>` does
(`specs/design/behavior-policies.md`). `DAY_OF_WEEK` has a `DATE` input,
which is always valid, so it has no policy; its body is integer arithmetic
that the code generator emits inline, as `compile_time_arith.rs` does for
`ADD_DT_TIME` and `SUB_DT_DT`.

- **Analyzer.** The four signatures join `other_time_functions`
  (`compiler/analyzer/src/intermediates/stdlib_time_function.rs`), with
  `ANY_INT` inputs as `MUX` declares its selector
  (`compiler/analyzer/src/intermediates/stdlib_function.rs:519`). A new rule
  rejects a call whose components are all constants and do not form a
  value, whatever the policy, as P2039 does for date literals.
- **Options.** A policy `policy_calendar_invalid` (`--policy-calendar-invalid`)
  with the alternatives `trap` (the default) and `zero`, declared in
  `compiler/container/src/policy.rs` and `define_compiler_options!`, and
  selected by the dialect presets (decision 1).
- **Container.** A func_id block per function: `CONCAT_DATE` and `CONCAT_TOD`
  take two func_ids each (`trap`, `zero`), `CONCAT_DT` two more. The
  components are widened to `i64` by the code generator, so one func_id per
  alternative serves every `ANY_INT` input type.
- **VM.** One handler per function, validating the components before
  computing the value; under `trap`, a new `V4xxx` trap naming the function
  and the components; under `zero`, the result type's zero.
- **Code generation.** A new module `compiler/codegen/src/compile_calendar.rs`
  that widens the arguments, emits the `BUILTIN` with the func_id for the
  selected policy, and emits `DAY_OF_WEEK` inline; dispatched from
  `compile_function_call` (`compiler/codegen/src/compile_call.rs`) by one
  line beside the existing `time_arith_for` dispatch.
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

1. **Invalid components** (month 13, 30 February, hour 25) **and valid
   dates outside the `u32` seconds range** (before 1970, after
   2106-02-07 06:28:15): a behavior policy, `policy_calendar_invalid`, with
   `trap` (default) and `zero`. The standard gives no rule for invalid
   components and the surveyed targets disagree, so by ADR-0049 rule 4 the
   default traps:
   - RuSTy's tests document that `CONCAT_DATE` and `CONCAT_TOD` "yield 0 for
     unrepresentable inputs and clamp to the DATE range"
     (`tests/lit/single/stdlib_overflow/README.md`);
   - CODESYS `CAA_DTUtil` documents `DT#1970-01-01-00:00` (`TOD#00:00`) for
     invalid inputs of `DTU.DTConcat` (`DTU.TODConcat`), with an error
     output;
   - Fernhill documents the valid ranges but not the result outside them;
   - OSCAT's `SET_DATE` documents that days roll over (30 February is
     1 or 2 March) and that a month outside 1..12 is January;
   - TwinCAT, Siemens S7-1200/1500 and matiec have none of the three.

   Normalisation is therefore not offered: no surveyed target documents it
   as a whole (ADR-0049 rule 5). The `rusty` dialect selects `zero`; so does
   `codesys` if the maintainer wants its preset to follow `CAA_DTUtil`
   (`twincat` has no such function to follow). As for `STRING_TO_*`, the
   failure policy is one choice, not one per cause: under `zero` a valid
   date outside the range is 0 too, where RuSTy clamps; the docs of the
   preset state the divergence. A non-trapping validity check (a library
   function, as the ADR's companion rule asks) is a follow-up, recorded in
   an issue.

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

`compiler/container/src/builtin.rs` is 980 lines on `main`, and the three
func_id blocks with their `declare_builtins!` rows and tests would take it
past the 1000-line limit. The prefactor, in its own commit before any new
func_id: move the `str_to_num` block module (lines 572-672) and its tests
into `compiler/container/src/builtin/str_to_num.rs`, re-exported under the
same path, so no caller changes. The calendar blocks then go into
`compiler/container/src/builtin/calendar.rs` beside it.

`compile_call.rs` is already above the limit (1212 lines); this change adds
one line to it and does not make the split of that module part of this
work: it is a separate, larger prefactor that this plan should not hide.

## Design doc reference

`specs/design/calendar-functions.md` (new), requirement IDs
`REQ-CAL-analyzer-NNN` (signatures, constant checks) and
`REQ-CAL-codegen-NNN` (values, including 29 February, century years, the
limits of the `u32` range, and the day of the week of 1970-01-01),
`REQ-CAL-container-NNN` (the func_id blocks) and `REQ-CAL-vm-NNN` (each
alternative of the policy). `specs/design/behavior-policies.md` gains the
policy's row in its table of policies and its presets.

## File map

- `specs/design/calendar-functions.md` — new, with the decisions above;
  `specs/design/behavior-policies.md` — the policy and its presets
- `compiler/analyzer/build.rs`, `compiler/codegen/build.rs`,
  `compiler/container/build.rs`, `compiler/vm/build.rs` — list the design
- `compiler/analyzer/src/intermediates/stdlib_time_function.rs` — signatures
- `compiler/analyzer/src/rule_calendar_constant_components.rs` — new, the
  constant check, and its problem code in
  `compiler/problems/resources/problem-codes.csv` and
  `docs/reference/compiler/problems/P####.rst`
- `compiler/container/src/policy.rs` — `CalendarInvalid`
- `compiler/container/src/builtin/str_to_num.rs` — moved (prefactor);
  `compiler/container/src/builtin/calendar.rs` — new, the func_id blocks
- `compiler/parser/src/options.rs` — the policy row and the presets
- `compiler/vm/src/calendar.rs` — new, the handlers; `compiler/vm/src/builtin.rs`
  — dispatch; `compiler/vm/src/error.rs` — the new trap and its `V4xxx`
- `compiler/codegen/src/compile_calendar.rs` — new
- `compiler/codegen/src/compile_call.rs` — one dispatch line
- `compiler/codegen/src/lib.rs` — module
- `compiler/codegen/tests/it/end_to_end_calendar.rs` — new, and
  `compiler/codegen/tests/it/main.rs`
- `compiler/codegen/src/spec_conformance_*.rs` / analyzer, container and VM
  conformance tests
- `docs/reference/standard-library/functions/{concat_date,concat_tod,concat_dt,day_of_week}.rst`
  — new pages in the format of `concat_date_tod.rst`, with playground
  examples and the result under each alternative of the policy
- `docs/reference/standard-library/functions/index.rst` — the four entries
- `docs/reference/runtime/problems/V4xxx.rst` — the new trap

## Tasks

- [x] Rows, parameter names, result types and day numbering from Table 36
- [ ] Open the issue for `SPLIT_DATE`, `SPLIT_TOD`, `SPLIT_DT` (blocked by
      function outputs)
- [ ] Agreement of the maintainer on decision 1 (the policy, its default and
      which presets select `zero`)
- [ ] Open the issue for the non-trapping validity check
- [ ] Prefactor: move `builtin::str_to_num` into its own file
- [ ] Design document with the decisions and requirement IDs
- [ ] Tests first: the examples of the table (`CONCAT_DATE(2010, 3, 12)`,
      `CONCAT_TOD(16, 33, 12, 0)`, `CONCAT_DT(2010, 3, Day, 12, 33, 12, 0)`
      with `Day : USINT`, `DAY_OF_WEEK(DATE#2010-03-10)` into a `USINT`,
      which is 3); a `SINT` year rejected; end-to-end tests of known dates (1970-01-01, 2000-02-29,
      2024-02-29, 2100-03-01, 2106-02-07), times (00:00:00.000,
      23:59:59.999), days of the week, invalid components and dates outside
      the range under `trap` (the `V4xxx`, through `execute()`) and `zero`,
      and the rejected constant call
- [ ] Analyzer signatures and their conformance tests
- [ ] The policy, the func_id blocks and the VM handlers
- [ ] `compile_calendar.rs` and the dispatch line
- [ ] Documentation pages
- [ ] `cd compiler && just`; docs build without warnings
- [ ] Remove this plan
