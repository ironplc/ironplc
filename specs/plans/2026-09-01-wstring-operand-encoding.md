# Fix WSTRING comparison trap and prevent the class

Issue: [#1550](https://github.com/ironplc/ironplc/issues/1550)

## Goal

`eq := w = "abc"` on a `WSTRING` variable compiles clean and then traps at
runtime with `V9014 - string encoding mismatch: expected char_width 2, got 1`
— a trap whose own documentation says it "should not occur during normal
operation". Fix the trap, and remove the shape of codegen that let it exist.

## Cause

Codegen has no single place that answers *"what encoding does this string
value have?"*. Instead each site that produces a string value decides for
itself:

- `compile_string.rs::resolve_string_arg` hard-codes `NARROW_CHAR_WIDTH` for
  every scratch slot and for the string-literal constant it interns
  (`:122,132,142,156,171,181`), so every literal operand of a comparison or a
  string function is Latin-1 regardless of what it is compared against.
- `compile_stmt.rs:243`, `compile_stmt.rs:289` and `compile_setup.rs:469,746`
  each hand-roll the opposite rule — "if the RHS is a literal, encode it at the
  destination's width" — as a local `if let ... CharacterString(lit)`. The
  struct-field string assignment at `compile_stmt.rs:207` forgets the case
  entirely.
- `compile_call.rs:285` initializes a `WSTRING` parameter's slot wide and then
  fills it from whatever `resolve_string_arg` produced (narrow).

The width is therefore stated correctly in four places, incorrectly in three,
and the disagreement is only detected by the VM, one scan into the run.

## Prefactor: one place decides a string value's encoding

New module `compiler/codegen/src/string_width.rs`, holding the whole rule:

```rust
enum OperandWidth {
    /// Fixed by a declaration: a variable, a field, an array element, a
    /// function's declared return type.
    Declared(CharWidth),
    /// A literal, which has no encoding until it is used; carries the width
    /// its spelling suggests, for when nothing else decides.
    Adaptable(CharWidth),
    /// Codegen cannot tell.
    Unknown,
}

fn operand_width(ctx, expr) -> OperandWidth;
fn resolve_operand_char_width(ctx, operands, span) -> Result<CharWidth, Diagnostic>;
fn compile_string_value(emitter, ctx, expr, char_width) -> Result<(), Diagnostic>;
```

- `operand_width` reads `expr.resolved_type` (the analyzer already distinguishes
  `WSTRING` from `STRING` there) for anything but a literal or a call; literals
  come from `lit.width`, which #1575 put on the node; calls take the width of
  their own string arguments, falling back to the signature's return type.
- `resolve_operand_char_width` is what an operation with several string
  operands calls. Two operands with *different* `Declared` widths are a genuine
  program error and become **P4034** (`StringEncodingMismatch`) with a span —
  today `CONCAT(s, w)` compiles and traps. Otherwise: the declared width, else
  the enclosing operation's hint, else the literal spelling, else narrow.
- `compile_string_value` is the single "produce a string value at this
  encoding" entry point that replaces the four hand-rolled literal cases. It
  also parks the target width in `ctx.string_width_hint` for the duration of
  the nested compile, so an all-literal nested call (`w = CONCAT('a','b')`)
  adapts the same way a direct literal does.

`CompileContext::note_char_width` folds the `has_wide_string` bookkeeping (two
sites today, both easy to forget) into the same path.

Behaviour-preserving step first: `resolve_string_arg` gains an explicit
`char_width` parameter and its three duplicated scratch-slot allocations
collapse into one `alloc_string_scratch`; every caller passes
`NARROW_CHAR_WIDTH`, so nothing changes yet.

## Fix

`compile_string_compare`, `compile_find`, `compile_concat`, `compile_replace`,
`compile_insert`, `compile_string_2arg`, `compile_string_3arg` and the string
parameter copy-in in `compile_call.rs` resolve their operand group's width once
and pass it to `resolve_string_arg`. `compile_string_compare` also stops
reporting its diagnostics at `SourceSpan::default()`.

## Prevention: static string-encoding verification

The stack-balance verifier (`codegen/src/stack_balance.rs` →
`container::verify_stack_balance`) already exists for exactly this situation: a
runtime failure the VM cannot attribute, stopped at the point where codegen
hands over a container. String encoding gets the same treatment.

New `container/src/verify_string_encoding.rs`, rule **R0304**, run from
`codegen/src/string_encoding.rs` beside the stack-balance check and reported as
**P9998** (internal error) — reaching it means codegen emitted the bytecode
wrong, which is what the new width resolution exists to prevent:

1. Collect every data-region slot's declared width from `STR_INIT`. An offset
   initialized twice at different widths is recorded as unknown, not as a
   violation — it is a reused scratch slot, not a mistake.
2. `FIND_STR` / `CONCAT_STR` / `REPLACE_STR` / `INSERT_STR` carry both data
   offsets as immediates: two known, differing widths is a violation.
3. `BUILTIN CMP_STR` takes its two offsets from the two `LOAD_CONST_I32`
   instructions before it; when that peephole matches, the two widths must
   agree.
4. `LOAD_CONST_STR` followed by `STR_STORE_VAR` must intern a constant whose
   pool tag (`Str` / `WStr`) matches the destination slot's declared width —
   the exact instruction pair the reported bug emitted.

Anything the pass cannot resolve statically is left alone, so the pass never
turns a program it does not understand into a compiler error.

`specs/design/bytecode-verifier-rules.md` gains R0304 and its index row.

## Documentation

`docs/reference/runtime/problems/V9014.rst` claims the analyzer rejects every
cross-encoding operation statically. It does not — it reasons only about named
variables. Rewrite the section to describe the three layers that actually
exist: the analyzer rule for declared variables, codegen's operand-width
resolution (P4034), and R0304 verification before the container ships.

## Tests

- `codegen/tests/it/end_to_end_wstring.rs`: the issue's program (compare
  against a wide literal), plus a narrow-spelled literal against a `WSTRING`,
  `FIND`/`CONCAT` with literal operands, a `WSTRING` literal passed to a
  `WSTRING` function parameter, and the narrow equivalents to prove `STRING` is
  unchanged.
- `string_width.rs` unit tests for the precedence rule and the P4034 conflict.
- `verify_string_encoding.rs` unit tests: hand-built bytecode for each of the
  four checks, including the pre-fix instruction sequence from the issue.
- `end_to_end_string.rs` regression: narrow programs still emit narrow slots.

## Out of scope

- Extending `rule_string_encoding_compat` to string-function arguments. Codegen
  now reports the same P4034 for those, from the place that knows the widths;
  duplicating the check in the analyzer would need its own copy of the width
  resolution.
- Cross-encoding *conversion* (`WSTRING_TO_STRING`). Still unimplemented, and
  unchanged by this work.
