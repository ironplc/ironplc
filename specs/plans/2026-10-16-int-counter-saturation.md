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

Names in `container/src/fb_type.rs`: every counter constant names its width.
The new ids are `CTU_INT`, `CTD_INT`, `CTUD_INT`, and the constants for
`0x0020`–`0x0022` are renamed from `CTU`/`CTD`/`CTUD` to `CTU_DINT`/`CTD_DINT`/
`CTUD_DINT`, so no constant reads as the plain IEC block while counting at
another width. Backward compatibility is not a concern: the rename touches
seven uses (`codegen/src/compile_call.rs`, `vm/src/vm.rs`,
`vm/src/intrinsic.rs`).

**One table of the VM's standard blocks.** Today the `FB_CALL` dispatch in
`vm/src/vm.rs` lists the counter ids, and `CounterWidth::of` lists them again
with a wildcard arm that sends every unlisted id to `I32`. A new id has to be
added in both places, and forgetting `of` silently counts in 32 bits. A new
enum in its own VM module replaces both lists:

```rust
pub enum Intrinsic {
    Ton, Tof, Tp,
    Ctu(CounterWidth), Ctd(CounterWidth), Ctud(CounterWidth),
    Sr, Rs, RTrig, FTrig,
}

impl Intrinsic {
    /// The VM's block for `FB_CALL` type id `type_id`, or `None` for an id
    /// that names no standard block (a user-defined block's, or none).
    pub fn of(type_id: u16) -> Option<Intrinsic>;
}
```

`Intrinsic::of` is the one match over the standard ids, naming each one, and
returns `None` for every other `u16` (the only fallback a match over `u16` can
have). `CounterWidth::of` goes. The `FB_CALL` dispatch matches
`Intrinsic::of(type_id)`: `Some(intrinsic)` is an exhaustive match over the
enum with no wildcard, and `None` keeps today's lookup of a user-defined
block, which traps with `Trap::InvalidFbTypeId` if there is none.

**Check `FB_CALL` type ids at load.** `Vm::load` already validates a container
before any buffer is written (`validate_call_depth`,
`validate_system_uptime`). It gains a third check: every `FB_CALL` in the code
section names either a standard block (`Intrinsic::of` is `Some`) or a
user-defined block in the type section. A container that names any other id is
rejected at load with `Trap::InvalidFbTypeId(id)`, instead of trapping when the
call first runs, which may be rounds later or never. So a counter id that
`Intrinsic::of` does not list fails every program that uses it, at load.

The check walks each function body by instruction boundary. That walk lives
in the container crate (`cfg::instruction_boundaries`, crate-private). The
container crate exposes a function that yields each `FB_CALL`'s function id,
offset and type id, and errors on a body that does not decode. The VM maps that
error to the trap it raises today for an undecodable instruction
(`Trap::InvalidInstruction`). It is one pass over the code at load. The check
goes in a new VM module, not in `vm.rs`, which is already over the size
limit. `specs/design/bytecode-verifier-rules.md` R0002 says today that an
`FB_CALL` type id is "< FB type count", which has not been true since standard
ids were fixed constants. Its row changes to "a standard block's id, or a
user-defined block's id in the type section".

**Container format version.** `FORMAT_VERSION` stays 4. This fixes a defect.
Adding `FB_CALL` type ids changes neither the header layout nor a section's
encoding. The `_UDINT`, `_LINT` and `_ULINT` ids were added the same way.

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

One prefactor PR. Adding a counter id today needs a new `match` arm in two
places, the `FB_CALL` dispatch and `CounterWidth::of`. Forgetting the second
compiles and counts in 32 bits, which is the shape of this defect. The
prefactor:

- adds `Intrinsic` and `Intrinsic::of` in a new VM module, mapping today's
  ids exactly as today (`0x0020`–`0x0022` → `I32`);
- has the `FB_CALL` dispatch match `Intrinsic::of(type_id)`;
- removes `CounterWidth::of`;
- renames `fb_type::CTU`/`CTD`/`CTUD` to `CTU_DINT`/`CTD_DINT`/`CTUD_DINT`.

It changes no behaviour. Every id that dispatched to a block still does, at
the same width. Every other id still reaches the user-defined block lookup.
The existing tests pass unchanged apart from the rename. `CounterWidth::of`'s
test becomes `Intrinsic::of`'s.

Everything else drops into the existing shapes:

- the counters are already generic over `CounterValue`, with one
  `counter_value!` line per width;
- `standard_fb_layout` is already the one table of ids in codegen.

`intrinsic.rs` (732 lines) stays under 1000. The new code goes in new modules,
not in `vm.rs`.

## Design doc reference

- [`specs/design/counter-widths.md`](../design/counter-widths.md): the width
  table, REQ-CW-vm-001, the new REQ-CW-vm-005 and "Out of scope".
- [`specs/design/bytecode-instruction-set.md`](../design/bytecode-instruction-set.md):
  the type id table and the paragraph after it.
- [`specs/design/bytecode-verifier-rules.md`](../design/bytecode-verifier-rules.md):
  R0002's `FB_CALL` row.
- [`specs/design/lowered-program.md`](../design/lowered-program.md):
  "Standard function blocks", REQ-LOW-codegen-078 and "Delivery Constraints".
- [`specs/design/bytecode-container-format.md`](../design/bytecode-container-format.md):
  the versioning policy (no change).

## File map

| File | PR | Change |
|---|---|---|
| `compiler/container/src/fb_type.rs` | prefactor | Rename `CTU`/`CTD`/`CTUD` to `CTU_DINT`/`CTD_DINT`/`CTUD_DINT` |
| `compiler/vm/src/intrinsic_id.rs` (new) | prefactor | `Intrinsic`, `Intrinsic::of` and its tests |
| `compiler/vm/src/intrinsic.rs` | prefactor | Remove `CounterWidth::of`; update the doc comment |
| `compiler/vm/src/vm.rs` | prefactor | Dispatch `FB_CALL` on `Intrinsic::of` |
| `compiler/codegen/src/compile_call.rs` | prefactor | Rename uses |
| `compiler/container/src/fb_type.rs` | core | Add `CTU_INT`, `CTD_INT`, `CTUD_INT` (`0x002C`–`0x002E`) |
| `compiler/vm/src/intrinsic.rs` | core | `CounterWidth::I16`, `counter_value!(i16, 2, i64)`, the `at_width!` arm, unit tests |
| `compiler/vm/src/intrinsic_id.rs` | core | Map the new ids to `I16` |
| `compiler/container/src/` (`cfg.rs` or a sibling) | core | Public walk yielding each `FB_CALL`'s function id, offset and type id |
| `compiler/vm/src/load_check.rs` (new) | core | Reject an unknown `FB_CALL` type id at load; called from `Vm::load` |
| `compiler/codegen/src/compile_call.rs` | core | `standard_fb_layout`: split `Int` from `Dint` for each counter |
| `compiler/codegen/src/lowered/place.rs` | core | Remove `reads_standard_block_field` |
| `compiler/codegen/src/lowered/stmt.rs` | core | `Assign` always uses `emit_assign_fitting` |
| `compiler/codegen/tests/it/end_to_end_fb_counter_widths.rs` | core | Include `INT` in the property tests (rename `WIDE` and update its comment); add #2332's cases |
| `compiler/codegen/tests/it/compile_route_lowered_fb_call.rs` | core | Rename and re-pin `run_when_int_counter_counts_past_int_then_stored_narrowed`; replace same-code cases that bind an `INT` counter output or read its field |
| `compiler/codegen/tests/it/compile_route_lowered_fb_field.rs` | core | Replace same-code cases that read a standard block's field, if they now differ |
| `specs/design/counter-widths.md` | core | The width table, REQ-CW-vm-001 and REQ-CW-vm-005; drop the out-of-scope bullet |
| `specs/design/bytecode-instruction-set.md` | core | The type id table and paragraph; a container naming an unknown `FB_CALL` id is rejected at load |
| `specs/design/bytecode-verifier-rules.md` | core | R0002's `FB_CALL` row |
| `specs/design/lowered-program.md` | core | Drop the `_INT` exception (three places) |
| `docs/reference/standard-library/function-blocks/{ctu,ctd,ctud}.rst` | core | `CV` saturates at the bounds of its type |

## Tests

**Prefactor** (`vm/src/intrinsic_id.rs`):

- `of_when_standard_id_then_block_at_width`: one case per standard id.
- `of_when_user_or_unknown_id_then_none`: `0x1000` and `0x002F`.

**VM unit tests** (core, `vm/src/intrinsic.rs`):

- `ctu_when_int_at_max_then_stays_and_q_from_preset`: a `CTU` of `INT` at
  32767 stays there on a rising edge, and `Q` is `CV >= PV`.
- `ctd_when_int_at_min_then_stays`: a `CTD` of `INT` at -32768 stays there.
- `ctud_when_int_at_bounds_then_saturates_both_ways`.
- `ctd_when_int_counts_below_zero_then_cv_sign_extended`: -1 fills all 64
  bits of the slot.
- These, with the saturation cases, are the `#[spec_test]`s of the new
  REQ-CW-vm-005.
- `Intrinsic::of` gains cases for `CTU_INT`, `CTD_INT` and `CTUD_INT` (→
  `I16`).

**Load check** (core, `vm/src/load_check.rs`, plus a VM test through
`Vm::load`):

- `load_when_fb_call_names_unknown_type_then_invalid_fb_type_id`: rejected at
  load, before `start`.
- `load_when_fb_call_names_standard_or_user_type_then_ok`.
- `load_when_no_fb_call_then_ok`.

**End to end** (core, `codegen/tests/it/`). Each runs by both routes: the
harness in `common/run.rs` compiles every program both ways and compares the
variables.

- `end_to_end_fb_counter_widths.rs`: property tests over every width,
  including `INT`.
- #2332's tables, checked by value:
  - a `CTD` loaded with -32768 and counted down: `CV => out` (`INT`) is
    -32768, `d.CV` into a `DINT` is -32768, and `d.CV < -32768` is FALSE;
  - a `CTU` after 40,000 rising edges: `CV => out` is 32767, `c.CV` into a
    `DINT` is 32767, and `c.CV > 32767` is FALSE.
- `run_when_int_counter_counts_past_int_then_stored_narrowed` is renamed
  `run_when_int_counter_counts_past_int_then_saturates` and pins -32768.
- Same-code tests that relied on the bridge narrowing are replaced with value
  tests, or with cases where both routes emit the same instructions (a binding
  to a wider variable, or no output). The core PR lists each one it changes.
  The candidates are:
  - `counter_int` and `field_read` in
    `compile_when_standard_block_called_then_same_code_as_ast`;
  - the `("", "i")` width in
    `compile_when_every_counter_input_and_output_bound_then_same_code_as_ast`;
  - `read_standard_block` in `compile_route_lowered_fb_field.rs`;
  - any timer or `BOOL` output case that the run shows now differs.

**Other tests pinning 32-bit `INT` counting.** Grep found none beyond those
above:

- `end_to_end_fb_field_width.rs` narrows an input. It is unchanged, because
  `PV` is still stored narrowed.
- `end_to_end_fb_output_width.rs` stores -1, which is in range and still
  sign-extended.

The core PR re-runs the grep and names any test it changes.

## Tasks

### Prefactor PR: "Dispatch the VM's standard blocks from one table of ids"

- [ ] Rename `fb_type::CTU`/`CTD`/`CTUD` to `CTU_DINT`/`CTD_DINT`/`CTUD_DINT`
- [ ] `Intrinsic` and `Intrinsic::of` in a new module, naming every
      standard id
- [ ] `FB_CALL` dispatch matches `Intrinsic::of`; remove `CounterWidth::of`
- [ ] Tests of `Intrinsic::of`
- [ ] `cd compiler && just`

### Core change PR: "Count the counters of INT in 16 bits and saturate" (Fixes #2332)

Stacked on the prefactor PR until it merges.

- [ ] Add `CTU_INT`, `CTD_INT` and `CTUD_INT` to `fb_type.rs`; map them in
      `Intrinsic::of`
- [ ] `CounterWidth::I16`, `counter_value!(i16, 2, i64)` and the `at_width!`
      arm; update the `CounterWidth` doc comment
- [ ] VM unit tests, with `#[spec_test]` for REQ-CW-vm-005
- [ ] The public walk over `FB_CALL` sites in the container crate
- [ ] The load-time check of `FB_CALL` type ids in `Vm::load`, with tests
- [ ] `standard_fb_layout` maps the `INT` counters to the new ids
- [ ] Confirm that every VM block field holds a value of its type (counters,
      the timers' `ET`, `BOOL` outputs)
- [ ] Remove `reads_standard_block_field` and its use
- [ ] Update or replace the end-to-end and same-code tests; add #2332's cases
- [ ] `counter-widths.md`, `bytecode-instruction-set.md`,
      `bytecode-verifier-rules.md` and `lowered-program.md`
- [ ] `ctu.rst`, `ctd.rst` and `ctud.rst` (per `doc-standards.md`)
- [ ] `cd compiler && just`, `cd specs && just` and the docs checks
- [ ] Comment on #2288 that the `INT` counter change has landed in the VM, so
      generating the blocks no longer changes behaviour

### Cleanup

- [ ] Close this plan PR unmerged

## Out of scope

- Generating the standard blocks in lowering (#2288, ADR-0060).
- Any other counter width, timer or block.
- The rest of the bytecode verifier at load: only the `FB_CALL` type id check
  is added.
