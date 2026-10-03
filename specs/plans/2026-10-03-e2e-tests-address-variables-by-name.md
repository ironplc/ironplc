# End-to-End Tests Address Variables by Name

## Goal

Every `end_to_end_*.rs` test observes a program's state through IEC names and
access paths (`count`, `result`, `timer.Q`, `s.names[2]`). No test uses a VM
slot index or a data-region byte offset. Codegen can then change how variables
are laid out, and another backend can run the same test bodies, without editing
the tests.

## Background: what the tests depend on today

The suite is `compiler/codegen/tests/it/end_to_end_*.rs`: 146 files, counted
on 2026-10-03.

| Pattern | Count | Layout fact it relies on |
|---|---|---|
| `(slot, value)` tuples passed to `e2e_*!` / `assert_run_*` | ~1,000 tuples in 746 calls | Slot order: globals first, then program locals in declaration order, with `VAR_EXTERNAL` skipped |
| `bufs.vars[N].as_*()` | 148 | Same, plus one 64-bit slot per variable |
| `vm.read_variable*(VarIndex::new(N))` / `write_variable*` | 47 / 5 | Same |
| `FbStep::Write/Expect/Pulse` keyed by slot | 195 in 10 files | Same |
| `string_offset(&[254, 254])` + `read_string(&bufs.data_region, …)` | 32 / 82 | The data-region allocator's order and per-string size (`STRING_HEADER_BYTES + max_len`) |
| `bufs.vars[0].as_i32() as usize` then stride arithmetic | 19 in 4 files | An aggregate's slot holds its data-region base; struct slot rounding (`3 * ceil((4+10)/8)`) |
| Tests compiled with the system-uptime option | 7 files | `__SYSTEM_UP_TIME`/`__SYSTEM_UP_LTIME` occupy slots 0–1 |

Many files also carry comments like `// timer=var0, result=var1`. These record
the layout by hand, and nothing checks that they are right.

The value side of each assertion also depends on representation. A test picks
`as_i32()`, `as_i64()` or `as_f32()` for each slot. It writes
`as_i32() as u32` to read a UDINT, and it asserts a BOOL as `1`. Each of
these says how the VM stores a value, which is not the same as what the
value is.

69 of the 146 files use only the tuple helpers. Everything else mixes in
one or more of the hand-written patterns.

## Architecture

### 1. Tests observe through names and IEC values

The shared harness gets one way to look at a program after loading it. It
takes the analyzed program in, and it gives names and values out. Nothing in
this API exposes a slot or a byte offset.

```rust
/// What a test can see of a program after one scan.
pub struct Snapshot { /* owns the container and buffers */ }

/// A loaded program driven across several scans.
pub struct Session<'a> { /* borrows them */ }

impl Snapshot  { pub fn read(&self, path: &str) -> Value; }
impl Session<'_> {
    pub fn scan(&mut self, time_us: u64) -> Result<(), FaultContext>;
    pub fn read(&self, path: &str) -> Value;
    pub fn write(&mut self, path: &str, value: impl Into<Value>);
}

/// An IEC 61131-3 value as a test observes it, whatever the backend's storage.
pub enum Value { Bool(bool), Int(i128), Real(f64), Str(String) }
```

The input is the analyzed `Library` and `SemanticContext`, with a thin
wrapper that takes source text. The `Tc2_Math`/`Tc2_Utilities` tests merge a
bundled library before they analyze, so the harness cannot take only source.

`Value` is the contract that every backend decodes its own storage into:

| Declared type | `Value` |
|---|---|
| `BOOL` | `Bool` |
| Every integer and bit-string type | `Int`, holding the exact value: a UDINT of all ones is `4294967295`, not `-1` |
| `REAL`, `LREAL` | `Real` (REAL widens to f64 exactly) |
| `TIME`, `LTIME` | `Int` milliseconds ([ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md)) |
| Date and time-of-day types | `Int` in [ADR-0025](../adrs/0025-datetime-unsigned-representation.md) units |
| Enumeration, subrange | `Int` (the ordinal, or the base-type value) |
| `STRING`, `WSTRING` | `Str`, holding the decoded content |

### 2. The tuple helpers keep their shape; only the key changes

```rust
// before
e2e_i32!(end_to_end_when_add_expression_then_variable_has_sum, "…", &[(0, 10), (1, 42)]);
// after
e2e_i32!(end_to_end_when_add_expression_then_variable_has_sum, "…", &[("x", 10), ("y", 42)]);
```

The typed families (`e2e_i32!`, `e2e_f64_near!`, …) keep their names and
the Rust types of their expected values. Each one reads a `Value` and
converts it with a checked conversion. A conversion that would lose
information fails the test, for example a UDINT's `Int(4294967295)` checked
against an `i32`, or an `LREAL` read by an `f32` helper. `Bool` still
compares as 0/1 against integer expectations. That keeps the rewrite to the
key only, and 0/1 is how IEC converts a BOOL, so it says nothing about
storage.

```rust
// before: timer=var0, enable=var1, result=var2, elapsed=var3.
Write(1, 1), Run(0), Expect(2, 0), … Expect(3, 0)
// after
Write("enable", 1), Run(0), Expect("result", 0), … Expect("elapsed", 0)

// before
let result_offset = string_offset(&[254, 254]);
assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
// after
assert_eq!(snapshot.read("result"), "Hello World");
```

### 3. The VM resolves names from the container's debug section

The VM implementation of `read`/`write` finds a variable using the same
tables that the debugger, `--dump-vars`, the MCP `run` tool and the
playground already use:

- **Name → slot and type tag.** It uses the `VAR_NAME` entries (tag 2) whose
  `function_id` is `GLOBAL_SCOPE`. Those entries cover the program's
  variables and the globals; a container holds exactly one program
  (`find_program` in `codegen/src/compile.rs`). Names match without regard
  to case, as IEC identifiers do. An unknown name panics and lists the names
  that do exist.
- **String content.** It uses `STRING_LAYOUT` (tag 4) plus the header's char
  width.
- **Paths into aggregates** (`s.x`, `arr[2]`, `timer.Q`). It uses the tree
  API in [Variable Inspection Model](../design/variable-inspection-model.md)
  §3. That API is not built yet; see §5.

Alternatives considered:

- **Re-parse the source in the helper to number the declarations.** This
  is a second copy of the allocator's rules, and that copy is the problem
  this plan removes.
- **Export a codegen-internal symbol table for tests.** This adds a
  test-only API that no other consumer checks, and a second backend could
  not supply one.
- **Compare text rendered by `VariableRenderer`.** This ties ~1,000
  assertions to the display rules, so a change from `1.5` to `1.50` would
  fail all of them.

As a result, the suite also checks the debug section. If the debug table
named the wrong slot, the debugger would show a wrong value, and with this
change the end-to-end tests fail as well. Today nothing fails. When a lookup
fails, the error names the slot it resolved to, so a debug-info bug is easy
to tell apart from a codegen bug.

There is a known gap. Some declarations reach `assign_variables` with type
tag `OTHER` and an empty type name (`compile_setup.rs`, the `_ =>` arm), and
named subranges also carry `OTHER`. For these, the VM decodes the slot as a
signed integer. That is exactly what the tests assert today. Recording the
real base type is a separate fix.

### 4. Tests about the VM stay on the VM, but still look slots up by name

Some tests in this suite are about the VM and its API, not about the
language:

- `end_to_end_write_variable_raw.rs` tests the embedder's raw read/write
  API. Rename it `vm_api_write_variable_raw.rs`. It gets its `VarIndex` from
  a `vm_var_index(&container, "fieldbus_in")` helper.
- `vm_when_uptime_enabled_then_globals_shift_by_two` asserts a layout fact,
  so it moves to a `compile_*` test, where layout belongs.
- `assert_stack_balanced` stays inside the VM's `scan`, so it still runs on
  every test.
- The 14 assertions on `Trap::*` in 11 files do not depend on layout, and
  this plan leaves them alone. A fault category that works for any backend
  is for the plan that adds a second backend to decide.

### 5. Aggregates come last

Top-level scalars and strings can be resolved from today's debug section.
That covers everything except the 19 reads of an aggregate's base offset, in
`end_to_end_struct.rs`, `end_to_end_array_string.rs`,
`end_to_end_array_string_paren_length.rs` and `end_to_end_wstring.rs`.

Those reads wait for steps 1–3 of the Variable Inspection Model (§7 of that
design). Then `read("s.names[2]")` walks the same type tree that the
debugger expands, and no second copy of the layout exists. Assertions in
those files on scalar results such as `result` move in the earlier PRs.

The `read_max_length` checks on string headers assert representation. They
are rewritten to assert behaviour instead: assign an over-long value and
check that it is truncated to the declared length.

### 6. One backend, with an API that does not assume it

Only the VM backend exists today. `Snapshot` and `Session` are therefore
concrete types whose public API assumes nothing about the VM. They become a
trait when a second backend arrives
([Development Standards](../steering/development-standards.md#when-not-to-prefactor):
extract the shared shape when the second caller arrives). The plan for that
backend also decides where the suite should live. It sits in `codegen/tests`
because codegen was the only backend. That plan also decides how a backend
marks tests it cannot run yet, without silently skipping them.

### 7. Enforcement

When the migration is done:

- Delete the helpers keyed by slot, as well as `string_offset` and the
  test-local `read_string`.
- Add a guard test that fails if any `tests/it/end_to_end_*.rs` file
  mentions `VarIndex`, `.vars[`, `data_region` or `VmBuffers`.
- Record the rule in ADR-0057, "End-to-end tests observe variables by name".
  It complements [ADR-0020](../adrs/0020-bytecode-test-strategy.md): `end_to_end` covers values,
  `compile_*` covers layout and encoding, and `vm/tests` covers traps.

### Migration mechanics

Rewriting ~1,000 tuple keys by hand risks swapping two names, and a swap
like that can still pass, for example when two variables are both expected
to be `0`. So the keys are captured, not written by hand:

1. Add a temporary hook. With `IRONPLC_E2E_CAPTURE=<file>` set, the old
   slot-keyed helpers append `(test name, slot, name)` to the file. The
   name comes from the compiled container's `VAR_NAME` table, and the test
   name comes from libtest's thread name.
2. A throwaway script rewrites each test's keys from that file. Each name
   comes from the same container whose slot the old assertion read, so the
   mapping is right by construction rather than guessed from the source.
3. Run the suite on the name-keyed helpers.

Neither the hook nor the script lands on `main`. The script goes in the PR
description.

## Prefactoring

`codegen/tests/it/common/mod.rs` is 999 lines, one short of the module
limit, and the new resolver and `Value` do not fit in it. Split it with no
change in behaviour:

- `common/bc.rs`: the instruction builders and `assert_bytecode!`
- `common/run.rs`: parse, compile and run, and the FB driver
- `common/assert.rs`: `assert_run_*` and the `e2e_*!` macros

## Design doc reference

- [Variable Inspection Model](../design/variable-inspection-model.md): the
  layout tables and tree API that aggregate paths resolve through
- [Variable Value Rendering](../design/variable-value-rendering.md):
  `VariableRenderer`, which remains the only renderer
- [ADR-0020](../adrs/0020-bytecode-test-strategy.md): the three test
  categories this plan builds on

## File map

| File | Change |
|---|---|
| `compiler/codegen/tests/it/common/{bc,run,assert}.rs` | New, split out of `common/mod.rs` (prefactor) |
| `compiler/codegen/tests/it/common/observe.rs` | New: `Snapshot`, `Session`, `Value`, name resolution from the debug section |
| `compiler/codegen/tests/it/common/assert.rs` | Helpers and macros keyed by name; `FbStep` keyed by name |
| `compiler/codegen/tests/it/end_to_end_*.rs` | Keys rewritten; slot comments deleted |
| `compiler/codegen/tests/it/vm_api_write_variable_raw.rs` | Renamed from `end_to_end_write_variable_raw.rs` |
| `compiler/codegen/tests/it/compile_*.rs` | Receives the uptime slot-layout test |
| `compiler/codegen/tests/it/end_to_end_guard.rs` | New guard test (last PR) |
| `specs/adrs/0057-end-to-end-tests-observe-variables-by-name.md` | New ADR |
| `specs/steering/syntax-support-guide.md` | "End-to-End Execution Testing" uses names, not `bufs.vars[N]` |
| `specs/steering/compiler-architecture.md` | Codegen test table and end-to-end template |

## Tasks

### Prefactor PR 1: split the shared harness

- [ ] Move `bc`, the run helpers and the assertion helpers into
  `common/bc.rs`, `common/run.rs` and `common/assert.rs`
- [ ] `cd compiler && just` passes with no test edits

### Core PR 2: names for the single-scan tuple helpers

- [ ] `Value`, with checked conversions to the typed expectations
- [ ] Resolve names via `VAR_NAME`/`STRING_LAYOUT`; panic on an unknown name
  and list the known ones
- [ ] `Snapshot`; `assert_run_*` and `e2e_*!` take `(&str, T)`
- [ ] Capture and rewrite the ~1,000 tuples; the suite passes
- [ ] List each expectation that encoded representation (for example an
  unsigned value asserted as a negative `i32`) and fix it to the IEC value
- [ ] ADR-0057, `status: proposed`

### Core PR 3: multi-scan tests, FB drivers and strings

- [ ] The `parse_and_run_rounds` closure gets a `&mut Session`; `FbStep` is
  keyed by name
- [ ] Migrate the 148 `bufs.vars[N]`, the 47 `read_variable*` and the 5
  `write_variable*` sites; delete the `varN` comments
- [ ] Replace `read_string(&bufs.data_region, string_offset(…))` with
  `read("name")`
- [ ] Rewrite the `read_max_length` checks as truncation checks
- [ ] Rename `vm_api_write_variable_raw.rs`, add `vm_var_index`, and move the
  uptime layout test to `compile_*`
- [ ] Update the steering guides

### Prerequisite (own plan): Variable Inspection Model steps 1–3

### Core PR 4: aggregate paths and enforcement

- [ ] `read`/`write` resolve `a.b[i]` through the `VariableRenderer` tree
- [ ] Migrate the 19 aggregate-base reads in the four files listed in §5
- [ ] Delete the slot-keyed helpers, `string_offset` and the test-local
  `read_string`
- [ ] Add the guard test; set ADR-0057 to `accepted`

A tracking issue lists PRs 2–4 and the inspection-model prerequisite.

## Open questions for review

1. **Aggregates: wait or copy?** This plan waits for the inspection model.
   The alternative is to rewrite the four files now so each copies the
   field into a scalar (`r := s.names[2]`) and asserts `r`. That is
   available sooner, but each test then also exercises a read path it did
   not cover before.
2. **Enumerations as ordinal or name?** This plan uses the ordinal because
   it causes no churn. A name would be more neutral across backends, but
   then every enum assertion has to change.
3. **TIME as an `Int` of milliseconds, or as its own `Duration` variant?**
   This plan uses milliseconds because it matches today's expectations.
