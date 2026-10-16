# Counters of `INT` Saturate at the Bounds of `INT`

Fixes [#2332](https://github.com/ironplc/ironplc/issues/2332).

## Goal

The standard counters of `INT` (`CTU`, `CTD`, `CTUD`, also spelled `CTU_INT`,
`CTD_INT`, `CTUD_INT`) count in 16 bits on the VM and saturate at -32768 and
32767, as IEC 61131-3 §2.5.2.3.3 says and as every other counter width already
does. A `CTD` loaded with -32768 and counted down holds -32768, and a `CTU`
after 40,000 rising edges holds 32767, read as a field and as an output, by
both code generation routes.

With every field of a VM block then holding a value of its declared type, the
bytecode bridge's narrowing of a store read from a standard block's field
(`reads_standard_block_field`) is redundant and is removed.

The fix is in the VM now; it does not wait for lowering to generate the
standard blocks ([#2288](https://github.com/ironplc/ironplc/issues/2288),
ADR-0060).

## Architecture

**VM.** `CounterWidth` gains `I16`. `counter_value!(i16, 2, i64)` implements
`CounterValue` for it: `PV` and `CV` are read from the field's first two bytes,
counting saturates at `i16::MIN` and `i16::MAX`, and `CV` is written
sign-extended into the 8-byte slot (REQ-CW-vm-004's rule). `at_width!` runs the
generic `ctu_at`/`ctd_at`/`ctud_at` at `i16`, so no counter is copied.

Three new `FB_CALL` type ids continue the sequence in `CTU`, `CTD`, `CTUD`
order:

| Counts in | `CTU` | `CTD` | `CTUD` |
|---|---|---|---|
| signed 16-bit (`CTU`, `_INT`) — **new** | `0x002C` | `0x002D` | `0x002E` |
| signed 32-bit (`_DINT`) | `0x0020` | `0x0021` | `0x0022` |
| unsigned 32-bit (`_UDINT`) | `0x0023` | `0x0024` | `0x0025` |
| signed 64-bit (`_LINT`) | `0x0026` | `0x0027` | `0x0028` |
| unsigned 64-bit (`_ULINT`) | `0x0029` | `0x002A` | `0x002B` |

Names in `container/src/fb_type.rs`: `CTU_INT`, `CTD_INT`, `CTUD_INT`. The
existing `CTU`/`CTD`/`CTUD` constants keep `0x0020`–`0x0022`; their doc
comments say they count in a signed 32-bit integer for `_DINT` only. (Renaming
them to `CTU_DINT` etc. would be clearer but touches every use; it is not
needed for the fix and is left out.)

`CounterWidth::of` names every counter id and drops its wildcard arm. Since it
takes a `u16`, not an enum, it cannot be exhaustive without some fallback; it
returns `Option<CounterWidth>` (`None` for an id that is not a counter), and
the `FB_CALL` dispatch in `vm/src/vm.rs`, which already matches the counter
ids, adds the three new ids to its `CTU`/`CTD`/`CTUD` arms and turns `None`
into `Trap::InvalidFbTypeId`, the trap it raises today for an unknown type id. A new counter id
that is not listed in `of` then traps rather than silently counting in 32 bits.

**Container format version.** `specs/design/bytecode-container-format.md`
bumps `format_version` for a change to the header layout or a section's
semantics. Adding `FB_CALL` type ids changes neither: the code section's
encoding is unchanged and existing ids keep their meaning. The `_UDINT`,
`_LINT` and `_ULINT` ids (`0x0023`–`0x002B`) were added the same way, with no
bump and no entry in the version history. Format version 4 is still marked "in
development". So `FORMAT_VERSION` stays 4. **Reviewer question:** a container
compiled before this change still names `0x0020` for a counter of `INT` and
would keep counting in 32 bits; if that should be rejected, the alternative is
to bump to 5 and add a row to the version history.

**Codegen.** `standard_fb_layout` in `codegen/src/compile_call.rs` maps
`Ctu(Int)`, `Ctd(Int)`, `Ctud(Int)` to the new ids and `Ctu(Dint)` etc. to the
old ones. It stays the one table of VM ids and fields; nothing else in codegen
names a counter id.

**Retire the bridge workaround.** `reads_standard_block_field`
(`codegen/src/lowered/place.rs`) and its use in the `Assign` arm of
`codegen/src/lowered/stmt.rs` go; every lowered `Assign` uses
`emit_assign_fitting`. Before removing it, the core PR confirms in the VM's
code that every field a VM block writes holds a value of its declared type:

- counters: `CV` at the counter's width (this change for `INT`; REQ-CW-vm-003
  and -004 for the rest); `Q`, `QU`, `QD` and the hidden edge fields are
  `i32::from(bool)`.
- timers: `ET` is written with `write_i32` (sign-extended 32 bits, the width
  of `TIME`), clamped to `PT`; `Q` is 0 or 1.
- `SR`/`RS`: `Q1` is `i32::from(bool)`; `R_TRIG`/`F_TRIG`: `Q` is 0 or 1.

The lowered route then omits the narrowing for `i := c.CV` and `CV => i` as it
does for `i := n`, because the lowered program already proves the read fits
(REQ-LOW-lowering-046). The route that reads the AST still narrows every
store, so for those statements the two routes no longer emit the same
instructions.

## Prefactoring

None needed. The counters are already generic over `CounterValue` with one
`counter_value!` line per width, `standard_fb_layout` is already the single
table of ids, and the VM dispatch already lists each counter id. The new width
is a line in each of those places. `intrinsic.rs` is 732 lines and stays well
under 1000 with the new tests; `vm.rs` grows by three match alternatives.
Changing `CounterWidth::of` to return `Option` is part of the core change, as
it exists to make the new ids safe.

## Design doc reference

- [`specs/design/counter-widths.md`](../design/counter-widths.md) — width
  table, REQ-CW-vm-001, new REQ-CW-vm-005, "Out of scope".
- [`specs/design/bytecode-instruction-set.md`](../design/bytecode-instruction-set.md)
  — type id table and the paragraph after it.
- [`specs/design/lowered-program.md`](../design/lowered-program.md) —
  "Standard function blocks", REQ-LOW-codegen-078, "Delivery Constraints".
- [`specs/design/bytecode-container-format.md`](../design/bytecode-container-format.md)
  — versioning policy (no change; see above).

## File map

| File | Change |
|---|---|
| `compiler/container/src/fb_type.rs` | Add `CTU_INT`, `CTD_INT`, `CTUD_INT` (`0x002C`–`0x002E`); reword `CTU`/`CTD`/`CTUD` docs to `_DINT` |
| `compiler/vm/src/intrinsic.rs` | `CounterWidth::I16`; `counter_value!(i16, 2, i64)`; `at_width!` arm; `CounterWidth::of` names every id and returns `Option`; unit tests |
| `compiler/vm/src/vm.rs` | Add the new ids to the `CTU`/`CTD`/`CTUD` dispatch arms; handle `of` returning `None` |
| `compiler/codegen/src/compile_call.rs` | `standard_fb_layout`: split `Int` from `Dint` for each counter |
| `compiler/codegen/src/lowered/place.rs` | Remove `reads_standard_block_field` |
| `compiler/codegen/src/lowered/stmt.rs` | `Assign` always uses `emit_assign_fitting` |
| `compiler/codegen/tests/it/end_to_end_fb_counter_widths.rs` | Include `INT` in the property tests (rename `WIDE`, update its comment); add #2332's cases |
| `compiler/codegen/tests/it/compile_route_lowered_fb_call.rs` | Rename and re-pin `run_when_int_counter_counts_past_int_then_stored_narrowed`; replace same-code cases that bind an `INT` counter output or read its field |
| `compiler/codegen/tests/it/compile_route_lowered_fb_field.rs` | Replace same-code cases that read a standard block's field, if they now differ |
| `specs/design/counter-widths.md` | Width table, REQ-CW-vm-001, REQ-CW-vm-005, drop out-of-scope bullet |
| `specs/design/bytecode-instruction-set.md` | Type id table and paragraph |
| `specs/design/lowered-program.md` | Drop the `_INT` exception (three places) |
| `docs/reference/standard-library/function-blocks/{ctu,ctd,ctud}.rst` | `CV` saturates at the bounds of its type |

## Tests

**VM unit tests** (`vm/src/intrinsic.rs`):

- `ctu_when_int_at_max_then_stays_and_q_from_preset` — `CTU` of `INT` at 32767
  with a rising edge stays at 32767; `Q` is `CV >= PV`.
- `ctd_when_int_at_min_then_stays` — `CTD` of `INT` at -32768 stays there.
- `ctud_when_int_at_bounds_then_saturates_both_ways`.
- `ctd_when_int_counts_below_zero_then_cv_sign_extended` — -1 fills all 64
  bits of the slot. This is the `#[spec_test]` of the new REQ-CW-vm-005
  together with the saturation cases.
- `counter_width_of` gains cases for `CTU_INT`, `CTD_INT`, `CTUD_INT` (→
  `I16`) and for `CTU`/`CTD`/`CTUD` (→ `I32`); a non-counter id gives `None`.

**End to end** (`codegen/tests/it/`), each run by both routes (the harness in
`common/run.rs` compiles every program both ways and compares variables):

- `end_to_end_fb_counter_widths.rs`: property tests over every width including
  `INT`.
- #2332's tables, checked by value: `CTD` from -32768 counted down → `CV =>
  out` (`INT`) -32768, `d.CV` into `DINT` -32768, `d.CV < -32768` FALSE; `CTU`
  after 40,000 rising edges → `CV => out` 32767, `c.CV` into `DINT` 32767,
  `c.CV > 32767` FALSE.
- `run_when_int_counter_counts_past_int_then_stored_narrowed` →
  `run_when_int_counter_counts_past_int_then_saturates`, pinning -32768.
- Same-code tests that relied on the bridge narrowing (`counter_int`,
  `field_read` in `compile_when_standard_block_called_then_same_code_as_ast`;
  the `("", "i")` width in
  `compile_when_every_counter_input_and_output_bound_then_same_code_as_ast`;
  `read_standard_block` in `compile_route_lowered_fb_field.rs`; and any timer
  or `BOOL` output case the run shows now differs) are replaced with value
  tests, or with cases where both routes emit the same instructions (a binding
  to a wider variable, or no output). The core PR lists each one changed.

**Other tests pinning 32-bit `INT` counting.** Grep found none beyond the
above: `end_to_end_fb_field_width.rs` narrows an input (unchanged: `PV` is
still stored narrowed), and `end_to_end_fb_output_width.rs` stores -1 (in
range, still sign-extended). The core PR re-runs the grep and names any it
changes.

## Tasks

### Core change PR — "Count the counters of INT in 16 bits and saturate" (Fixes #2332)

- [ ] Add `CTU_INT`, `CTD_INT`, `CTUD_INT` ids to `fb_type.rs`; reword the
      `CTU`/`CTD`/`CTUD` docs
- [ ] `CounterWidth::I16`, `counter_value!(i16, 2, i64)`, `at_width!` arm;
      update the `CounterWidth` doc comment
- [ ] `CounterWidth::of` names every counter id, no wildcard, returns `Option`
- [ ] VM dispatch in `vm.rs` handles the new ids
- [ ] VM unit tests above, with `#[spec_test]` for REQ-CW-vm-005
- [ ] `standard_fb_layout` maps the `INT` counters to the new ids
- [ ] Confirm every VM block field holds a value of its type (counters,
      timers' `ET`, `BOOL` outputs)
- [ ] Remove `reads_standard_block_field` and its use
- [ ] Update/replace the end-to-end and same-code tests; add #2332's cases
- [ ] `counter-widths.md`, `bytecode-instruction-set.md`,
      `lowered-program.md`
- [ ] `ctu.rst`, `ctd.rst`, `ctud.rst` (per `doc-standards.md`)
- [ ] `cd compiler && just`, `cd specs && just`, docs checks
- [ ] Comment on #2288 that the `INT` counter change has landed in the VM, so
      generating the blocks no longer changes behaviour

### Cleanup

- [ ] Close this plan PR unmerged

## Out of scope

- Generating the standard blocks in lowering (#2288, ADR-0060).
- Any other counter width, timer or block.
- Renaming the `0x0020`–`0x0022` constants to `_DINT` names.
