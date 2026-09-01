# Plan: stop printing aggregates as their data-region offset

Fixes the cheap half of [#1599](https://github.com/ironplc/ironplc/issues/1599).

## Goal

`--dump-vars` prints a STRUCT, ARRAY or function-block variable as an integer
that is really its **byte offset into the data region**:

```
origin: 0          (* Point, at data offset 0 *)
inline_a: 8        (* ARRAY[1..3] OF DINT, at offset 8 *)
timer: 56          (* TON instance, at offset 56 *)
plain: 7           (* an actual value *)
```

Widening the struct moves the numbers, so an array's printed "value" changes
when an unrelated type declaration changes. This is the `msg: 0` defect of
#1558 in a new place: a plausible number where there is no value.

Render an honest placeholder instead. Structural rendering (`origin: (X := 11,
Y := 22)`) needs a layout sub-table and is tracked separately.

## Architecture

The renderer cannot tell an aggregate from a scalar today: both arrive as
`iec_type_tag::OTHER`. That tag is a grab-bag — it also carries **named
subrange types, which hold real scalar values**, so a blanket
"OTHER -> placeholder" would regress those from `75` to `<MY_RANGE>`.

So the fix is honest metadata, not string-sniffing: three new type tags that
say *this slot holds an offset, not a value*.

| Tag | Value | Emitted for |
|-----|-------|-------------|
| `STRUCT` | 25 | a structure variable |
| `ARRAY` | 26 | an array variable (including array-of-struct) |
| `FB_INSTANCE` | 27 | a function-block instance |

`VariableRenderer` renders those three as `<TYPE_NAME>` — the VAR_NAME entry
already records a good name (`POINT`, `ARRAY OF DINT`, `TON`) — marked
`valid: false`, so the `<unavailable>` / `<invalid>` precedent from #1576
carries over and a surface that styles values shows it as a placeholder.

Adding tag *values* to an existing byte field is additive: a reader that does
not know tag 25 takes the `_` fallback, which is exactly today's behaviour, so
no format version bump. LDATE/LTOD/LDT were added the same way.

`REF_TO` keeps `OTHER`: it holds a variable index, which is at least a
referent a reader can act on, and it is not an aggregate. Worth revisiting,
not here.

## Prefactoring

None needed. `VariableRenderer::render` already branches
`STRING | WSTRING` -> data region before the scalar path; aggregates are a
third arm of that same match, and the placeholder constructor
(`RenderedValue::placeholder`) already exists from #1576.

## Design doc reference

`specs/design/variable-value-rendering.md` — one new requirement in the
"Values that cannot be read" section. `specs/design/bytecode-container-format.md`
carries the tag table; note it is already stale (missing 18-24, which have
existed since the LDATE work), so fix that divergence opportunistically per
the steering rule.

## File map

Modified:
- `compiler/container/src/debug_section.rs` — three tag constants
- `compiler/container/src/debug_format/mod.rs` — the aggregate arm
- `compiler/codegen/src/compile_setup.rs` — struct, FB, and the local-decl path
- `compiler/codegen/src/compile_array.rs` — array registration
- `compiler/codegen/src/compile_array_struct.rs` — array-of-struct registration
- `compiler/vm-cli/tests/cli.rs` — end-to-end dump of each aggregate kind
- `specs/design/variable-value-rendering.md`
- `specs/design/bytecode-container-format.md`
- `docs/reference/runtime/ironplcvm.rst`

## Tasks

- [ ] Tag constants + spec table (including the stale 18-24 rows)
- [ ] Renderer arm + unit tests
- [ ] Codegen emits the new tags at every aggregate site
- [ ] End-to-end `--dump-vars` test over a compiled struct/array/FB program
- [ ] Docs
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
