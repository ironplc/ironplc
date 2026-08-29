# Table-Driven Disassembly of the Opcode Space

## Goal

Make the container viewer render every assigned opcode with its mnemonic and
decoded operands, and reshape the code so that a future opcode *cannot* be
added without the viewer rendering it — enforced by the compiler and by the
shape of the code, not by a test that someone must remember to keep green.

Fixes ironplc/ironplc#1451 (50 of 126 assigned opcodes render as
`UNKNOWN(0x..)`, including `CALL`, all 64-bit and float arithmetic, and the
`STORE_VAR_I64`/`F32`/`F64` stores).

## Why the gap keeps reappearing

The opcode space is enumerated three times, in three shapes:

1. `opcode.rs` — one `pub const` per opcode (the byte value).
2. `opcode.rs::instruction_size_opt` — a match giving each opcode its *size*.
3. `disassemble.rs::decode_instructions` — a match giving each opcode its
   *mnemonic and operand rendering*.

Nothing ties (3) to (1) and (2), so an opcode added to (1) and (2) silently
falls through (3)'s catch-all. Worse, (2) and (3) each encode the operand
layout independently — (2) as a total byte count, (3) as a set of read
offsets — so the two can disagree about where an instruction ends.

The issue suggests a completeness test. A test is the weaker guard: it fails
after the fact, and only if someone keeps running it. The stronger fix is to
remove the duplicate enumerations so the incomplete state cannot be written
down.

## Architecture

**One declaration per opcode, carrying its operand layout.** A
`declare_instruction_set!` macro in `container/src/opcode.rs` takes rows of

```rust
/// Load a 32-bit integer constant from the constant pool.
LOAD_CONST_I32 = (OP_CLASS_LOAD_CONST, T_I32) => [ConstIndex];
```

and generates from each row:

- the `pub const LOAD_CONST_I32: Opcode` byte (unchanged value, unchanged docs),
- `opcode::mnemonic(op) -> Option<&'static str>` (the row's own name, via
  `stringify!` — no second copy of the string to drift),
- `opcode::operands(op) -> Option<&'static [Operand]>` (the row's layout).

`instruction_size` then *derives* from the layout (`1 + sum of operand
widths`) instead of restating it, and `is_assigned` stays derived from that.
Size and rendering can no longer disagree, because both read the same list.

`Operand` is a small enum of operand *roles* — `ConstIndex`, `VarIndex`,
`RefIndex`, `ArrayDescIndex`, `DataOffset`, `FieldIndex`, `FbTypeId`,
`FunctionId`, `BuiltinId`, `JumpOffset`, `CmpOp`, `MaxLength`, `CharWidth`,
`NumFields`, `FieldVarOffset`, `ParamVarOffset` — each with a known byte
width and a known rendering.

`decode_instructions` then has **no per-opcode arms at all**. It reads the
mnemonic and the layout and formats each operand by role. Consequences:

- An assigned opcode cannot be missing from the viewer: there is no arm to
  forget. `UNKNOWN(0x..)` again means only "unassigned byte".
- The one remaining extension point — a *new operand role* — is an exhaustive
  `match` on `Operand` with no catch-all, so adding a role that the viewer
  does not render **fails to compile**.

The built-in function IDs have the identical defect one level down: the
viewer names 45 of the 107 `opcode::builtin` IDs by hand and prints the rest
as bare hex. The same macro treatment fixes it, but it rewrites the built-in
constants in place, which collides with the module split (below). It
therefore lands as its own follow-up once both this change and the split have
merged; here the hand-written list is confined to one helper function so that
follow-up is a body swap.

### Alternatives considered

- **A completeness test only** (as the issue proposes). Kept as a
  belt-and-braces test, but not the primary guard: it catches the mistake
  after it is written rather than preventing it.
- **An `Opcode` enum with one variant per opcode, matched exhaustively in the
  viewer.** This makes a missing arm a compile error, but keeps a per-opcode
  arm in the viewer (the thing that drifted) and adds a second name per
  opcode (variant *and* constant). The table-driven form deletes the arms
  entirely, which is strictly stronger: nothing to forget beats being
  reminded.

## Prefactoring

The prefactor is the `declare_instruction_set!` table itself, landing in its own
commit before the viewer changes: the table reshapes the opcode space so the
viewer rewrite is a deletion rather than an addition of 50 more arms. Only
then does `decode_instructions` change, and it loses ~700 lines of
near-identical match arms rather than gaining any.

A second, unrelated simplification — splitting `opcode::builtin` and
`opcode::fb_type` into their own module files, which brings `opcode.rs` back
under the 1000-line limit — ships as a **separate pull request**. It is not
required by this change: the two touch different regions of `opcode.rs` and
merge cleanly in either order (verified by test-merge). It is listed here
only so the relationship is on the record.

## Design doc reference

- `specs/design/bytecode-instruction-set.md` — encoding rules (op-class /
  type-tag) that the table rows restate in structured form.

## File map

- `compiler/container/src/opcode.rs` — modified: `declare_instruction_set!` macro,
  `Operand` enum, opcode rows; `instruction_size_opt` derived from the
  layouts.
- `compiler/project/src/disassemble.rs` — modified: table-driven
  `decode_instructions`.

## Tasks

- [ ] Add `Operand` and `declare_instruction_set!`; convert all 126 opcode constants
      to table rows; derive `instruction_size_opt` from the layouts
- [ ] Verify no opcode byte changed (extract name → `(class, tag)` pairs
      before and after and diff)
- [ ] Confine the hand-written built-in name list to one helper, for the
      follow-up that generates it
- [ ] Rewrite `decode_instructions` to render from the table; make operand
      reads bounds-checked so a truncated container renders `<truncated>`
      instead of panicking
- [ ] Test: every byte for which `opcode::is_assigned` holds decodes to its
      mnemonic, never `UNKNOWN`
- [ ] Test: every operand role renders (one instruction per role)
- [ ] Test: `builtin::name` covers every declared built-in ID
- [ ] `cd compiler && just`
