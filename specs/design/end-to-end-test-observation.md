# Design: End-to-End Test Observation

status: approved
date: 2026-10-03

## Overview

This design specifies how an end-to-end test observes a program after
running it. A test names a variable with an IEC access path, such as
`count`, `result`, `timer.Q` or `s.names[2]`, and gets back an IEC value. It
never uses a VM slot index, a data-region byte offset, or the bit pattern
that a backend stores.

The goal is that variable layout belongs to the backend. Codegen can
reorder, pack or re-represent variables, and a new backend can run the same
test bodies, without anyone editing the end-to-end tests.

This design builds on:

- **[ADR-0020](../adrs/0020-bytecode-test-strategy.md)**, which splits tests
  into categories: end-to-end for correctness, `compile_*` and
  `wire_format.rs` for encoding, and `vm/tests` for traps. This design adds
  the rule that end-to-end tests do not see layout.
- **[Variable Inspection Model](variable-inspection-model.md)**, which
  defines the debug-section layout tables and tree API that paths into
  aggregates resolve through.
- **[Variable Value Rendering](variable-value-rendering.md)** and
  **[Bytecode Container Format](bytecode-container-format.md)**, which
  define the `VAR_NAME` and `STRING_LAYOUT` sub-tables that resolve
  top-level names.

### Requirements and conformance

Testable claims carry `REQ-OBS-codegen-NNN` identifiers per
[Development Standards — Design Requirement](../steering/development-standards.md#design-requirement).
`OBS` stands for **Obs**ervation. The `codegen` crate owns every
requirement, because the end-to-end suite and its harness live in
`compiler/codegen/tests/it`.

No crate's `build.rs` registers this file yet. The PR that implements the
harness registers it in `compiler/codegen/build.rs` and lands a
`#[spec_test]` with a real assertion for each requirement. The claims in §5
wait on the Variable Inspection Model, so they do not carry requirement
markers yet. Registering this file therefore does not demand tests for
behaviour that cannot be built. The PR that implements §5 numbers them from
REQ-OBS-codegen-080.

## Problem

The suite is `compiler/codegen/tests/it/end_to_end_*.rs`, 146 files counted
on 2026-10-03. It addresses variables by where the VM puts them:

| Pattern | Count | Layout fact it encodes |
|---|---|---|
| `(slot, value)` tuples passed to `e2e_*!` / `assert_run_*` | ~1,000 in 746 calls | Slot order: globals first, then program locals in declaration order, with `VAR_EXTERNAL` skipped |
| `bufs.vars[N].as_*()` | 148 | The same, plus one 64-bit slot per variable |
| `read_variable*(VarIndex::new(N))` / `write_variable*` | 47 / 5 | The same |
| `FbStep::Write/Expect/Pulse` keyed by slot | 195 in 10 files | The same |
| `string_offset(&[254, 254])` + `read_string(&bufs.data_region, …)` | 32 / 82 | The data-region allocator's order and per-string size |
| `bufs.vars[0].as_i32() as usize`, then stride arithmetic | 19 in 4 files | An aggregate's slot holds its data-region base; structures round to whole slots |
| Tests compiled with the system-uptime option | 7 files | `__SYSTEM_UP_TIME`/`__SYSTEM_UP_LTIME` take slots 0–1 |

Comments such as `// timer=var0, result=var1` record the layout by hand,
and nothing checks them.

The value side of each assertion encodes representation too. A test picks
`as_i32()`, `as_i64()` or `as_f32()` for each slot. It reads a UDINT as
`as_i32() as u32` and asserts a BOOL as `1`. It asserts `T#5s` as `5000`,
the VM's count of milliseconds, and `D#2024-01-01` as `1_704_067_200`, its
count of seconds since 1970; 17 files assert dates and durations this way.
Each of these states how the VM stores a value, which is not the same as
what the value is.

Any change to layout or representation therefore changes hundreds of tests
that are not about layout. A second backend cannot run them at all.

## Design Goals

1. **Names, not places.** A test refers to a variable the way the program
   does.
2. **Values, not bits.** A test compares against the IEC value of the
   variable's declared type: a number, a string, a duration or a date,
   never the count a backend stores it as.
3. **One source of truth for layout.** The harness locates variables with
   the tables the debugger already uses. Test code contains no copy of the
   allocator's rules.
4. **Backend-neutral surface, one backend behind it.** The observation API
   says nothing about the VM. It stays a concrete type until a second
   backend exists.
5. **Small diffs to the tests.** Existing tests keep their shape, and only
   the key in each assertion changes. The exception is a date or duration,
   whose expected count becomes a value (§3).

## Scope

**In scope:** top-level program variables and globals, of every elementary
type and of `STRING`/`WSTRING`; reads after one or more scans; writes
between scans; the single-scan assertion macros; the function-block step
driver; and, once the Variable Inspection Model lands, paths into
structures, arrays and function-block instances.

**Out of scope:**

- Function-local variables. They do not outlive a call, so no test can
  observe them after a scan.
- Traps. Tests that match `Trap::*` (14 assertions in 11 files) do not
  depend on layout. A fault taxonomy that works for any backend belongs to
  the design that adds a second backend.
- Tests whose subject *is* the VM or the layout. §6 lists them. They keep
  using the VM, but they look slots up by name.
- Multiple program instances. A container holds exactly one program
  (`find_program` in `codegen/src/compile.rs`). Qualified paths such as
  `main.x` are reserved for when that changes.

## Design decisions

### Resolve names through the debug section

**Chosen:** the VM implementation finds a name in the container's `VAR_NAME`
sub-table, and finds string contents through `STRING_LAYOUT`. These are the
same tables that the debugger, `--dump-vars`, the MCP `run` tool and the
playground use. Paths into aggregates resolve through the Variable
Inspection Model's tree.

**Rejected: number the declarations by re-parsing the source in the
harness.** That builds a second copy of the allocator's rules, and the
first copy is the problem this design removes.

**Rejected: export a codegen-internal symbol table for tests.** It would be
a test-only API that no other consumer checks, and a second backend could
not supply one.

**Rejected: compare text rendered by `VariableRenderer`.** That ties ~1,000
assertions to the display rules, so a change from `1.5` to `1.50` would fail
all of them.

**Consequence:** the suite also checks debug info. If the debug table named
the wrong slot, the debugger would show a wrong value, and with this design
the end-to-end tests fail too. When a lookup fails, the message names the
slot that the name resolved to, so a debug-info bug is easy to tell apart
from a codegen bug.

### Values are decoded from the declared type

The backend decodes each variable from its own storage into a `Value`
chosen by the variable's declared type. Today the test chooses an accessor
for the slot. With this design, a representation change, such as packing a
`SINT` into one byte, changes the backend and no test.

### Dates and durations are values, not counts

**Chosen:** a duration, a date, a time of day and a date-and-time each read
as a value of the matching `time` crate type: `Duration`, `Date`, `Time`
and `PrimitiveDateTime`. These are the types the DSL's temporal literals
already carry (`DurationLiteral`, `DateLiteral`, `TimeOfDayLiteral`,
`DateAndTimeLiteral` in `dsl/src/time.rs`). They hold a value, not a
count in a unit, and they are precise to the nanosecond, finer than any IEC
temporal type. Tests write expectations with the crate's constructors and
macros: `Duration::seconds(5)`, `date!(2024-01-01)`, `time!(12:30)`,
`datetime!(2024-01-01 12:30)`.

The encoding of temporal types is expected to change: the unit, the width
and the epoch. Today `TIME` is a 32-bit count of milliseconds
([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md)) and `DATE` a count of
seconds since 1970
([ADR-0025](../adrs/0025-datetime-unsigned-representation.md)). Under this
design, only the VM's decoder knows those facts, so changing them changes
the decoder and no test.

**Rejected: an `Int` in a unit fixed by this design**, such as milliseconds
for `TIME` and seconds for `DATE`. Every temporal assertion would still
state a unit, and if that unit stopped matching the encoding, every
expectation would need converting.

**Rejected: IEC literal text parsed by the compiler**, such as
`iec("T#1.5s")`. It reads naturally, but the parser under test would build
the expectation as well as the program's value. A bug in how a literal is
parsed would then appear on both sides of the assertion and pass, and
catching such bugs is the job of `end_to_end_duration_fraction.rs` and
`end_to_end_daytime_fraction.rs`.

### The assertion helpers keep their shape

The typed families (`e2e_i32!`, `e2e_f64_near!`, …) keep their names and
the Rust types of their expected values. Only the key changes, from a slot
index to a name, so the rewrite of ~1,000 tuples is mechanical. A heterogeneous
`Value`-keyed macro would read better but would rewrite every assertion
twice.

### No backend trait yet

`Snapshot` and `Session` are concrete types with a surface that does not
mention the VM. They become a trait when a second backend lands, per
[Development Standards](../steering/development-standards.md#when-not-to-prefactor).
The design for that backend also decides where the suite lives (it is in
`codegen/tests` only because codegen was the only backend), and how a
backend marks tests it cannot run yet without skipping them silently.

## 1. The observation API

```rust
/// What a test can see of a program after one scan. Owns its buffers.
pub struct Snapshot { /* … */ }

/// A loaded program driven across several scans. Borrows its buffers.
pub struct Session<'a> { /* … */ }

impl Snapshot {
    pub fn read(&self, path: &str) -> Value;
}

impl Session<'_> {
    pub fn scan(&mut self, time_us: u64) -> Result<(), FaultContext>;
    pub fn read(&self, path: &str) -> Value;
    pub fn write(&mut self, path: &str, value: impl Into<Value>);
}

/// An IEC 61131-3 value as a test observes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i128),
    Real(f64),
    Str(String),
    Duration(time::Duration),              // TIME, LTIME
    Date(time::Date),                      // DATE, LDATE
    TimeOfDay(time::Time),                 // TIME_OF_DAY, LTIME_OF_DAY
    DateAndTime(time::PrimitiveDateTime),  // DATE_AND_TIME, LDATE_AND_TIME
}
```

The harness takes the analyzed `Library` and `SemanticContext` as input,
with a wrapper that accepts source text. The `Tc2_Math` and `Tc2_Utilities`
tests merge a bundled library before they analyze, so source text alone is
not enough.

`Value` implements `From` for `bool`, the Rust integer types, `f32` (which
widens exactly), `f64`, `&str` and the four `time` types. It also
implements `PartialEq` against the same types, so that
`assert_eq!(snapshot.read("result"), "Hello World")` and
`assert_eq!(snapshot.read("d"), date!(2024-01-01))` read naturally.

### Date and time helpers

The harness re-exports what a test needs to write a temporal expectation:
the types `Duration`, `Date`, `Time` and `PrimitiveDateTime`, and the
macros `date!`, `time!` and `datetime!`. All of them come from the `time`
crate that the DSL already depends on. Codegen's dev-dependencies enable
the crate's `macros` feature.

| Declared type | Expectation |
|---|---|
| `TIME`, `LTIME` | `Duration::seconds(5)`, `Duration::milliseconds(1500)`, `-Duration::hours(1)` |
| `DATE`, `LDATE` | `date!(2024-01-01)` |
| `TIME_OF_DAY`, `LTIME_OF_DAY` | `time!(12:30:15.5)` |
| `DATE_AND_TIME`, `LDATE_AND_TIME` | `datetime!(2024-01-01 12:30)` |

A constructor such as `Duration::milliseconds(1500)` names a quantity, not
an encoding: it equals `Duration::seconds_f64(1.5)`, and it stays correct
whatever unit a backend stores. When a temporal assertion fails, the
message prints both sides as IEC literals (`T#1s500ms`, `D#2024-01-01`), so
it reads in the program's terms.

## 2. Resolving a name

Within one container, a name means at most one variable. Globals and
program variables share the program's scope, and
`rule_program_var_hides_global` rejects a collision between them.

**REQ-OBS-codegen-001** `read` returns the same value for a name whatever that variable's position among the declarations: a program and the same program with its `VAR` block reversed read identically by name.

**REQ-OBS-codegen-002** A name matches without regard to case: `read("COUNT")` and `read("count")` read the same variable.

**REQ-OBS-codegen-003** A global is read by its bare name, including from a program that declares it `VAR_EXTERNAL`.

**REQ-OBS-codegen-004** Enabling the system-uptime globals does not change the value that any user-declared name reads.

**REQ-OBS-codegen-005** A function's local variable is never a candidate: a program variable `x` is the one read, even when a function the program calls declares a local `x`.

**REQ-OBS-codegen-006** Reading or writing a name the program does not declare fails the test with a message that lists the names it does declare.

In the VM, the candidates are the `VAR_NAME` entries whose `function_id` is
`GLOBAL_SCOPE`. An entry's `var_index` is the slot that holds a scalar.
`STRING_LAYOUT` gives a string's `data_offset`, and the header's char width
gives its encoding. No other table and no test code locates a variable.

## 3. Values

The `Value` a variable reads as depends only on its declared type.

### Numbers and strings

**REQ-OBS-codegen-020** A `BOOL` reads as `Value::Bool`.

**REQ-OBS-codegen-021** A signed integer reads as `Value::Int` holding its value: a `SINT` holding -1 reads as `Int(-1)`.

**REQ-OBS-codegen-022** An unsigned integer or bit string reads as `Value::Int` holding its value, without sign extension: a `UDINT` of all ones reads as `Int(4294967295)`, and an `LWORD` of all ones reads as `Int(18446744073709551615)`.

**REQ-OBS-codegen-023** A `REAL` reads as `Value::Real` holding its `f32` value widened to `f64`, and an `LREAL` reads as `Value::Real` holding its value unchanged.

**REQ-OBS-codegen-024** An enumeration reads as `Value::Int` holding its ordinal, and a subrange reads as `Value::Int` holding its value.

**REQ-OBS-codegen-025** `STRING` and `WSTRING` read as `Value::Str` holding the current content, and never the capacity or the bytes past the current length: a `STRING[10]` assigned `'hi'` reads as `Str("hi")`.

### Dates and durations

No requirement in this section names a unit, an epoch or a width. Each
compares the value read against the value the program's literal denotes.

**REQ-OBS-codegen-030** `TIME` and `LTIME` read as `Value::Duration` holding the duration: a `TIME` assigned `T#1.5s` reads as `Duration::seconds_f64(1.5)`, and an `LTIME` assigned `LTIME#2d` reads as `Duration::days(2)`.

**REQ-OBS-codegen-031** A negative duration reads as a negative `Value::Duration`: a `TIME` assigned `T#-5s` reads as `-Duration::seconds(5)`.

**REQ-OBS-codegen-032** `DATE` and `LDATE` read as `Value::Date` holding the calendar date: a `DATE` assigned `D#2024-01-01` reads as `date!(2024-01-01)`.

**REQ-OBS-codegen-033** `TIME_OF_DAY` and `LTIME_OF_DAY` read as `Value::TimeOfDay` holding the time of day: a `TIME_OF_DAY` assigned `TOD#12:30:15.5` reads as `time!(12:30:15.5)`.

**REQ-OBS-codegen-034** `DATE_AND_TIME` and `LDATE_AND_TIME` read as `Value::DateAndTime` holding the date and time: a `DATE_AND_TIME` assigned `DT#2024-01-01-12:30:00` reads as `datetime!(2024-01-01 12:30)`.

**REQ-OBS-codegen-035** A temporal value never converts to or from a number: an integer expectation fails against a `TIME` variable, and writing an integer to a `DATE` variable fails.

REQ-OBS-codegen-035 keeps the encoding out of the tests. If an integer
expectation could match a `TIME`, it would have to be in some unit, and
that unit is the thing this section avoids.

The VM's decoder is the only code that knows the current encodings
([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md),
[ADR-0025](../adrs/0025-datetime-unsigned-representation.md)), and any
backend converts its own encoding into these values. A value reads as
exactly what the backend holds. A backend that stores `TIME` in whole
milliseconds holds `T#1.5ms` as one millisecond, so an expectation of
1.5 ms fails there. That failure is correct: it reports what the program
can actually hold.

Some declarations still reach the debug table with tag `OTHER`: named
subranges, and the initializer kinds in the fallback arm of
`assign_variables`. For those, the VM falls back to reading the slot as a
signed integer, which is what the tests assert today. Recording the real
base type is a separate codegen fix.

## 4. Writes, scans and assertion helpers

**REQ-OBS-codegen-040** A value written by name between scans is the value the program reads for that variable in the next scan.

**REQ-OBS-codegen-041** Writing a value that the declared type cannot hold fails the test: writing 300 to a `USINT` fails.

**REQ-OBS-codegen-042** Variable values persist across `scan` calls: a program that increments `count` once per scan reads 5 after five scans.

**REQ-OBS-codegen-043** The typed assertion helpers and the `e2e_*!` macros take `(name, expected)` pairs.

**REQ-OBS-codegen-044** A typed helper fails when the value read does not convert to the expected Rust type without loss: an `i32` expectation fails against a `UDINT` holding 4294967295, and an `f32` expectation fails against an `LREAL`.

**REQ-OBS-codegen-045** A `BOOL` equals the integer expectation 1 when TRUE and 0 when FALSE, and a write of 1 or 0 to a `BOOL` sets TRUE or FALSE.

**REQ-OBS-codegen-046** The function-block steps that write, expect or pulse a variable address it by name.

**REQ-OBS-codegen-047** The typed helpers accept `Duration`, `Date`, `Time` and `PrimitiveDateTime` as the expected type, and the function-block steps carry a `Value`, so a timer's elapsed time is written and expected as a duration.

Function-block step tables are built with constructor functions that take
`impl Into<Value>`: `write("enable", 1)`, `run(0)`,
`expect("elapsed", Duration::seconds(3))` and `pulse("cu", 3, 0)`. That way
a table mixes numbers, booleans and durations without a conversion at each
step.

REQ-OBS-codegen-045 is deliberate. It keeps the rewrite of the existing
tests to the key only, and 0/1 is how IEC converts a `BOOL` (`BOOL_TO_INT`),
so it does not depend on storage.

```rust
e2e_i32!(end_to_end_when_add_expression_then_variable_has_sum, "…",
    &[("x", 10), ("y", 42)]);           // was &[(0, 10), (1, 42)]

#[case::in_reset_restarts(TON_ENABLE, &[
    write("enable", 1), run(0), expect("result", 0),   // was Write(1, 1), Run(0), Expect(2, 0)
    …
    expect("elapsed", Duration::seconds(3)),           // was Expect(3, 3000)
])]

assert_run::<Duration>(TON_ET, &[("elapsed", Duration::seconds(5))]);   // was (1, 5000)
assert_run_with::<Date>(SOURCE, &edition3(), &[("d", date!(2024-01-01))]);
// was (0, 1_704_067_200)

assert_eq!(snapshot.read("result"), "Hello World");
// was: read_string(&bufs.data_region, string_offset(&[254, 254]))
```

The VM's `scan` keeps the stack-balance check that the current harness runs
after every round, so every test continues to guard against stack leaks.

## 5. Paths into aggregates

This section waits on steps 1–3 of the
[Variable Inspection Model](variable-inspection-model.md#7-implementation-sequence).
Its claims get requirement markers, numbered from REQ-OBS-codegen-080, in
the PR that implements them.

A path is a name followed by any number of `.field` and `[i]` or `[i,j]`
steps, as IEC writes an access. It resolves through
`VariableRenderer`'s tree, so the debugger's expansion and the harness's
reads are one walk over one set of tables.

- `read("s.x")` reads field `x` of structure variable `s`.
- `read("a[2]")` reads the element whose index is 2 under the declared
  bounds, which is the second element of `ARRAY[1..3]`.
- `read("m[1,2]")` reads a multi-dimensional array element under its
  declared bounds.
- `read("timer.Q")` reads an output of a function-block instance, for both
  standard and user-defined function blocks.
- Path steps compose to any depth: `read("items[2].inner.values[1]")` reads
  the leaf that path names.
- A path that names an aggregate rather than a leaf fails the test with a
  message that names the path's type.

Until this section is in scope, the 19 aggregate-base reads in
`end_to_end_struct.rs`, `end_to_end_array_string.rs`,
`end_to_end_array_string_paren_length.rs` and `end_to_end_wstring.rs` keep
their slot arithmetic. The scalar results in those files, such as `result`,
move to names with everything else. The `read_max_length` checks on string
headers assert representation, and they become behaviour checks instead:
assign an over-long value and assert that it is truncated to the declared
length.

## 6. Tests that are not end-to-end tests

Some files in the suite test the VM or the layout, not the language. They
move out of the `end_to_end_` prefix and keep the VM, but they find slots by
name through `vm_var_index(&container, name)`:

| Test | Subject | Destination |
|---|---|---|
| `end_to_end_write_variable_raw.rs` | The embedder's raw read/write API | `vm_api_write_variable_raw.rs` |
| `vm_when_uptime_enabled_then_globals_shift_by_two` | That the uptime globals take the first two slots | A `compile_*` test |

## 7. Enforcement

**REQ-OBS-codegen-100** No `end_to_end_*.rs` file in `compiler/codegen/tests/it` mentions `VarIndex`, `.vars[`, `data_region` or `VmBuffers`, and a guard test fails if one does.

The slot-keyed helpers, `string_offset` and the test-local `read_string`
are deleted when the last caller migrates. Then no end-to-end test can call
them, so a new test cannot fall back on them.

## 8. Migration

Rewriting ~1,000 keys by hand risks swapping two names, and a swap can
still pass, for example when two variables are both expected to be `0`. So
the keys are captured from the compiler, not inferred from the source:

1. A temporary hook in the slot-keyed helpers records
   `(test name, slot, name)` for every assertion. It reads the name from the
   compiled container's `VAR_NAME` table and takes the test name from
   libtest's thread name.
2. A throwaway script rewrites each test's keys from that record. Each name
   comes from the same container whose slot the old assertion read, so the
   mapping is right by construction.
3. The suite then runs on the name-keyed helpers.

Dates and durations need one more step, because their expected values
change as well as their keys. The hook also records each variable's
declared type. For a temporal variable, the script converts the old count
from the VM's current unit into a value: `5000` on a `TIME` becomes
`Duration::seconds(5)`, using the largest unit that is exact, and
`1_704_067_200` on a `DATE` becomes `date!(2024-01-01)`. A call that
asserts temporal and other variables together is split into one call per
expected type. The script is the only place the old units appear, and it
is thrown away. The assertions that go through the generic helpers were
converted this way in the prefactor (step 1). What remains are those in
the multi-scan tests, the function-block steps and the direct slot reads.

Neither the hook nor the script lands on `main`.

Implementation order. Each step leaves the suite green:

1. Prefactors, with no change in behaviour. One generic assertion helper
   reads every variable after a scan, through the `SlotValue` readers in
   `common/slot_value.rs`. Those readers already read dates and durations
   as values (§3), so the helper-based temporal expectations are values,
   not counts. `codegen/tests/it/common/mod.rs` is split into `bc.rs`,
   `run.rs` and `assert.rs`.
2. Add `Value`, name resolution and `Snapshot`, and migrate the single-scan
   tuples (§2, §3, REQ-OBS-codegen-043 to 045).
3. Add `Session` and migrate the multi-scan tests, the function-block steps,
   the string reads, the remaining date and duration expectations and the
   §6 moves (REQ-OBS-codegen-040 to 042, 046, 047).
4. After the Variable Inspection Model steps 1–3, add paths (§5), delete the
   slot-keyed helpers, and add the guard test (§7).

## 9. Amendments to other documents

Each amendment lands in the PR that changes the behaviour:

| Document | Claim | Change |
|---|---|---|
| [ADR-0020](../adrs/0020-bytecode-test-strategy.md) | End-to-end tests "compile IEC 61131-3 source, execute in the VM, and assert variable values" | Amendment: they assert by name and IEC value, and layout belongs to `compile_*` tests |
| [Syntax Support Guide](../steering/syntax-support-guide.md) | End-to-End Execution Testing shows `bufs.vars[N].as_i32()` | Shows `read("name")` and name-keyed helpers |
| [Compiler Architecture](../steering/compiler-architecture.md) | Codegen test table and end-to-end template | Same |

## Open questions

1. **Aggregates: wait, or copy now?** This design waits for the Variable
   Inspection Model. The alternative is to rewrite the four affected files
   now so each copies the field into a scalar (`r := s.names[2]`) and
   asserts `r`. That is available sooner, but each test then exercises a
   read path it did not cover before.
2. **Enumerations: ordinal or value name?** The ordinal causes no churn.
   The name would be more neutral across backends.
