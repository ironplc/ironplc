# LEN of a STRING held in an array element or a structure field

Issue: [#1485](https://github.com/ironplc/ironplc/issues/1485)

## Goal

`LEN(x[1])` and `LEN(r.s)` return the true current length of the element or
field, for `STRING` and for `WSTRING` alike, whatever capacity the element or
field was declared with.

## Context: what the issue reported and what is left of it

Issue #1485 was filed against a `compile_len` that resolved its argument to a
*name* and looked that name up in `ctx.string_vars`, so an array element was
`P9999 Capability is not implemented`.

That resolution no longer exists. `compile_string::resolve_string_arg`
materializes any operand that is not a simple named variable into a temporary
data-region slot and hands `LEN` that slot's offset, so the program in the
issue compiles and runs today — as does the structure-field form the issue did
not ask about, in both encodings. What is missing is coverage: nothing in the
test suite pins the behaviour, and `end_to_end_aggregate_copy.rs` still stages
its elements through `STRING` variables and cites the limitation as live.

Measuring the four shapes against a declared capacity above the 254-code-unit
default turns up what does remain of the defect:

| Operand | `LEN` today | Expected |
|---|---|---|
| `s : STRING[400]` holding 300 code units | 300 | 300 |
| `a : ARRAY[1..2] OF STRING[400]`, `LEN(a[1])` | 254 | 300 |
| `r.s : STRING[400]`, `LEN(r.s)` | 254 | 300 |

`allocate_string_temp` initializes every temporary at
`DEFAULT_STRING_MAX_LENGTH`, so the `STR_STORE_VAR` that fills it truncates any
operand declared larger. A named variable is read in place and escapes this; an
element or a field is not, and silently loses everything past 254. Every string
builtin routes its operands through the same helper, so `FIND`, `CONCAT`,
`MID` and the rest truncate identically, as does a `STRING` argument copied
into a user function's parameter slot.

## Architecture

The temporary exists to give an operand a data-region slot; its capacity should
be the operand's own, not a constant. The encoding of a string operand is
already resolved by walking the access back to the declaration that states it
(`string_width::string_expr_char_width`), and that same declaration states the
capacity. So the walk widens to answer both questions at once rather than
gaining a second walk beside it:

- `StringShape { char_width, max_length }` replaces the bare `CharWidth` that
  `string_expr_char_width` returned; `max_length` is `None` where the
  expression has no declared capacity of its own (a literal, a function
  result), and those keep the 254 default they have today.
- `resolve_string_arg` passes the shape's `max_length` to
  `allocate_string_temp`, which initializes the slot at that capacity.

A temporary is only ever an operand — the VM's string opcodes read their
`data_offset` operands and write their results into the temp-buffer pool — so
sizing one to the operand's own capacity cannot truncate a result.

## Prefactoring

Widening the existing walk *is* the prefactoring: the alternative is a second
`string_expr_max_length` that repeats `variable_char_width`'s root walk, its
`array_vars` lookup and its `walk_struct_chain` call. It is not split into its
own commit because a `StringShape` whose `max_length` nobody reads yet is a
field clippy reports as dead.

## File map

- `compiler/codegen/src/string_width.rs` — `StringShape`; `string_expr_shape`
  in place of `string_expr_char_width`
- `compiler/codegen/src/compile_string.rs` — `allocate_string_temp` takes the
  capacity; `resolve_string_arg` supplies it
- `compiler/codegen/tests/it/end_to_end_len.rs` — regression coverage
- `compiler/codegen/tests/it/end_to_end_aggregate_copy.rs` — drop the staging
  and the stale citation

## Tasks

- [ ] Widen the string-expression walk to `StringShape`
- [ ] Size the operand temporary from the shape
- [ ] Cover `LEN` of a `STRING`/`WSTRING` array element and structure field,
      including a capacity above 254 in each shape
- [ ] De-stage the `ARRAY OF STRING` copy test and delete its citation
- [ ] `cd compiler && just`

## Out of scope

`LEN(a)` — a whole `ARRAY OF STRING` rather than an element — passes the
analyzer and reaches codegen, which reads the first element's header and
returns a number. The cause is general rather than string-specific: an
expression whose type is an anonymous array gets no `resolved_type`, so
`rule_function_call_type_check` skips it and `ABS(a)` and `n := a` are accepted
too. Filed separately.
