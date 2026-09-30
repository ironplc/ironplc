# Keep the fractional seconds of time-of-day and date-and-time literals

Issue: #1921

## Goal

`TOD#10:00:00.250` stores 36,000,250 ms, as `T#1.25s` stores 1,250 ms. The
same holds for every spelling that shares the daytime grammar:
`TIME_OF_DAY#`, `TOD#`, `LTIME_OF_DAY#`, `LTOD#`, `DATE_AND_TIME#`, `DT#`,
`LDATE_AND_TIME#`, `LDT#`.

## Architecture

The parser rule `daytime()` builds a `time::Time` from `s.whole` and drops
`s.femptos`. Every later stage (codegen through `whole_milliseconds()` and
`seconds_since_epoch()`, plc2plc through `hmsm()`) already reads the
sub-second part of that `Time`, so the root fix is in that rule: build the
`Time` with `from_hms_nano`, keeping the fraction to the nanosecond (the
resolution `time::Time` holds).

Storage is unchanged (ADR-0025 and its amendment): `TIME_OF_DAY` and
`LTIME_OF_DAY` store milliseconds, `DATE_AND_TIME` and `LDATE_AND_TIME` store
seconds. A fraction finer than the stored unit is truncated, as ADR-0021
truncates a sub-millisecond `TIME` literal (`whole_milliseconds` truncates
toward zero). So `DT#2024-01-02-10:00:00.75` still stores the same second
count, and `TOD#10:00:00.0005` stores 36,000,000.

There is no separate constant-folding path for these literals: initial
values, statements, comparisons and CASE labels all lower through
`compile_constant` in `codegen/src/compile_expr.rs`.

`s.whole as u8` also wraps a second field above 255 into range
(`TOD#10:00:300` became `10:00:44`); `u8::try_from` rejects it like the
hour and minute fields.

plc2plc writes the fraction as `.{micro:0>2}`, which was harmless while the
fraction was always zero but would write 5 ms (5,000 us) as `.5000`, half a
second. The renderer writes the fraction from the nanosecond count with
trailing zeros removed, and no fraction when it is zero, so the round trip is
exact.

## Prefactoring

The daytime text (`hh:mm:ss.fraction`) is formatted twice in the plc2plc
renderer (time of day, date and time) and once, without a fraction, by each
literal's `Display`. Move the formatting into one DSL helper,
`daytime_text`, exposed on both literal types, and have the renderer use it,
preserving today's output, in its own commit. The fix then changes the
fraction format in one place.

## Design doc reference

- `specs/adrs/0025-datetime-unsigned-representation.md` (storage units)
- `specs/adrs/0021-time-32bit-ltime-64bit.md` (truncation of a sub-unit fraction)
- `specs/design/time-literals.md`: add a short section on daytime fractions.

## File map

- `compiler/parser/src/parser.rs` — `daytime()` keeps the fraction
- `compiler/dsl/src/time.rs` — `daytime_text`, `Display`, doc comments
- `compiler/plc2plc/src/renderer.rs` — use `daytime_text`
- `compiler/analyzer/src/rule_temporal_literal_range.rs` — message uses `daytime_text`
- `compiler/parser/src/tests/` — parser tests
- `compiler/codegen/tests/it/` — end-to-end tests for every spelling
- `compiler/plc2plc/src/tests/`, `compiler/resources/test/literal.st` — round trip
- `compiler/vm-cli/tests/cli.rs` — `--dump-vars` shows the milliseconds
- `docs/reference/language/data-types/elementary/*.rst` — truncation note
- `specs/design/time-literals.md`

## Tasks

- [ ] Prefactor: `daytime_text` in the DSL, used by the renderer
- [ ] Failing tests: parser, DSL, codegen end-to-end, plc2plc, vm-cli dump
- [ ] Fix `daytime()`; fraction format in `daytime_text`
- [ ] Docs and design doc
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, `cd specs && just`, docs build
