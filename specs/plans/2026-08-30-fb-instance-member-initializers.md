# Plan: Function-Block Instance Member Initializers, and the P4043 Gate

Fixes [#1543](https://github.com/ironplc/ironplc/issues/1543).

## Goal

Four defects, all reachable from the `:= (name := value)` initializer form:

1. **P4043's documented remedy does not work.** `tonDelta : TON := (PT :=
   T#100MS);` — the exact code `docs/reference/compiler/problems/P4043.rst`
   tells the reader to write — makes `tonDelta` a *struct* variable rather
   than a function-block instance, so invoking it fails with `P4012
   "Function block invocation is not a variable in scope"`, and compiling it
   fails with `P9999 "Unknown structure type"`. Following the page gets the
   reader a different error and no working program.

2. **The gate misses a bare variable reference.** Under the default strict
   Edition 2 dialect, `s : MyStruct := (x := g);` (where `g` is a runtime
   `INT` variable) is accepted with no diagnostic, and only fails much later
   in codegen as `P9999 "Unknown enum value"` — naming the wrong construct
   entirely. `g` is a runtime value by any reading, so P4043 must fire.

3. **Two documentation gaps.** P4043's message talks about *non-constant*
   expressions while the rule actually keys on syntactic shape (it accepts
   `(x := 1)` and `(x := -1)` but refuses `(x := 1 + 1)`), and the
   `pDevice^.Delta` example is associated with `--dialect=twincat`, which
   deliberately does not enable `REF_TO`.

4. **Span defect.** P4043 (and codegen's `P9999`) render their label at file
   offset 0 whenever the value is a compound expression, because
   `ConstantKind::span()` returns `SourceSpan::default()` for every literal
   kind except a couple, so `ExprKind::BinaryOp`'s `SourceSpan::join` of its
   two operands produces `0..0` with an empty `FileId`.

## Design references

- `specs/design/beckhoff-twincat-dialect.md` — the dialect that motivates
  the initializer extension.
- `specs/adrs/0040-*` — a dialect violation gets a specific `P####` that
  names the construct **and points at its real span**. Item 4 is that
  second half.

The durable outcome of item 1 belongs in
`specs/design/function-block-infrastructure-design.md`: an FB-typed
declaration written with a parenthesized member initializer is a
function-block *instance*, not a structure, and the resolution happens in
the analyzer rather than the parser.

## Architecture

### 1. FB-typed `:= (member := value)` resolves to an FB instance

`x : T := (a := 1)` parses through `structured_var_init_decl__without_ambiguous()`
into `InitialValueAssignmentKind::Structure`, unconditionally — the parser
cannot know whether `T` names a `STRUCT` or a function block, because type
declarations are not in scope yet. Every *other* ambiguous type reference
(`x : T;`) parses to `InitialValueAssignmentKind::LateResolvedType` and is
resolved by `xform_resolve_late_bound_type_initializer`, which already maps
a function-block type name to `InitialValueAssignmentKind::FunctionBlock`.
The `Structure` case simply has no such arm, so an FB-typed declaration with
an initializer never becomes an FB instance.

The fix is one more arm in the same fold: when a `Structure` initializer's
type name resolves to a function block (a stdlib FB in the type environment,
an unsupported standard type, or a user `FUNCTION_BLOCK` declaration), rewrite
it to `FunctionBlock`, moving `elements_init` across to `init` — the two
fields have the same type, `Vec<StructureElementInit>`, precisely because
they are the same construct.

`FunctionBlockInitialValueAssignment::init` has been carried through the AST
since the initializer was introduced but has never been non-empty, so
codegen ignores it. With this change it becomes reachable, and
`emit_initial_values` must emit a store per member: the same
`FB_LOAD_INSTANCE` / value / `FB_STORE_PARAM` / `POP` sequence that
`compile_stmt` already emits for `timer.PT := T#100MS`.

### 2. A bare identifier in the value position

`structure_element_initialization()` offers `constant()` then
`enumerated_value()` then `expression()`. `enumerated_value()` matches a
bare identifier, and the trailing lookahead added for `pDevice^.Delta` (which
requires the alternative to consume the whole value) *succeeds* for a bare
identifier, so PEG locks in `EnumeratedValue` and never reaches
`expression()`. No lookahead can fix this: at parse time `g` and `RED` are
the same token sequence in the same position. The residual hole is
structural, and the parser comment claims a completeness the construction
cannot deliver.

Resolution has to happen where declarations are known.
`xform_resolve_late_bound_expr_kind` already owns exactly this decision for
every *other* bare identifier: `resolve_late_bound` returns an
`EnumeratedValue` when the name is not a variable in scope but is a known
enumeration value, and a `Variable` otherwise. Reusing it for the struct
element value means one call in a new
`fold_struct_initial_value_assignment_kind`, rewriting an unqualified
`EnumeratedValue` to `Expression(Variable)` when the name resolves to a
variable. P4043 then fires on it, and with the flag on the value is a real
variable reference rather than a phantom enum member.

A `Type#VALUE` qualified enumerated value is left alone — it is unambiguous.

### 3. Documentation

- `P4043.rst`: replace the broken "use a constant value" remedy's claim with
  one that actually works and is verified by a test, and add the sentence
  distinguishing this gate (syntactic shape) from
  `--allow-constant-initializer-expressions`/P4037 (which folds).
- `enabling-dialects-and-features.rst` and `ironplcc.rst`: note that the
  `REF_TO` spelling in the example needs a dialect that enables `REF_TO`,
  and that TwinCAT spells it `POINTER TO`.

### 4. Constant spans

Give every `ConstantKind` variant a real span, so `ExprKind::span()` is
correct for any expression built from literals:

| Literal | Today | Change |
|---|---|---|
| `Integer` | has `span`, parser passes `SourceSpan::default()` | parser passes the token span |
| `RealLiteral` | none | add `span` |
| `BooleanLiteral` | none | add `span` |
| `CharacterStringLiteral` | none | add `span` |
| `DurationLiteral` | has `span`, built from **token indices** not byte offsets | build from token spans; `ConstantKind::span()` stops discarding it |
| `TimeOfDayLiteral`, `DateLiteral`, `DateAndTimeLiteral` | none | add `span` |
| `BitStringLiteral` | delegates to `Integer` | covered |

`SourceSpan`'s `PartialEq` is unconditionally `true`, so no equality
assertion anywhere changes behaviour.

## Prefactoring

**Extract the FB-instance field store** — *landed separately, on branch
`claude/extract-fb-field-store`; this plan builds on it.*

`compile_stmt::compile_assignment` contained the four-instruction sequence
that writes one field of an FB instance, inline and entangled with the
assignment-target dispatch around it (signal: the new behaviour would
otherwise duplicate an existing emit sequence). It moved to
`codegen/src/compile_fb_init.rs` as `compile_fb_field_store`, taking the
instance and the field name, with the assignment path calling it.
Behaviour-preserving: the emitted bytecode is unchanged, so the existing
end-to-end FB tests are the proof. It also keeps `compile_setup.rs`
(991 lines) under the 1000-line module guideline, which adding the
initializer loop inline would break.

Splitting it out is deliberate: a pure extraction is reviewable against
"is this the same bytecode?" alone, and mixing it with the behaviour
changes below buries that question. Item 1 needs it, so this plan's branch
is stacked on it rather than reproducing it.

## Non-goals

- Constant folding in the P4043 gate. `(x := 1 + 1)` stays a P4043; the
  distinction from P4037 is documented rather than removed.
- `Array` / `Structure` values as FB instance member initializers. These
  are already a no-op for plain struct fields (pre-existing gap); the FB
  path returns `not_implemented` rather than silently dropping them.
- The `FunctionBlockCall` (`comm : FB_Comm(retries := 3)`) constructor form,
  which is a different construct with its own codegen gap.

## File map

| File | Change |
|---|---|
| `analyzer/src/xform_resolve_late_bound_type_initializer.rs` | `Structure` → `FunctionBlock` when the type name is an FB |
| `analyzer/src/xform_resolve_late_bound_expr_kind.rs` | unqualified `EnumeratedValue` struct-element value → `Expression(Variable)` when the name is a variable |
| `parser/src/parser.rs` | record the residual ordered-choice hole; token spans for literals |
| `codegen/src/compile_fb_init.rs` | initializer emission, beside the extracted field store |
| `codegen/src/compile_setup.rs` | emit FB instance member initializers |
| `codegen/src/compile_struct.rs` | comment recording that FB-typed initializers no longer reach here |
| `dsl/src/common.rs`, `dsl/src/time.rs` | literal spans; `ConstantKind::span()` |
| `docs/reference/compiler/problems/P4043.rst` | working remedy; P4037 distinction |
| `docs/explanation/enabling-dialects-and-features.rst`, `docs/reference/compiler/ironplcc.rst` | `REF_TO` dialect caveat |
| `specs/design/function-block-infrastructure-design.md` | record the resolution rule |

## Testing strategy

- Analyzer: `tonDelta : TON := (PT := T#100MS);` resolves to
  `InitialValueAssignmentKind::FunctionBlock` with the member init carried
  across; a user FB does the same; a real `STRUCT` still resolves to
  `Structure`.
- Analyzer: `s : MyStruct := (x := g)` produces P4043 under the default
  dialect, and is accepted with `allow_struct_initializer_expressions`;
  `(c := RED)` with `RED` an enum value is never flagged.
- End to end: the exact `P4043.rst` remedy program compiles *and runs*, with
  the timer's `PT` observably set to the initializer's value.
- CLI: `ironplcc check` on the doc example under the dialect the docs name.
- Span: the P4043 diagnostic for `(x := 1 + 1)` points at `1 + 1`.

## Tasks

- [ ] Plan (this document)
- [x] Prefactor: extract the FB-instance field store (separate branch)
- [ ] Item 1: FB-typed initializer resolution + codegen + tests
- [ ] Item 2: bare-identifier gate + tests
- [ ] Item 4: constant spans + tests
- [ ] Item 3: documentation
- [ ] Record the durable design note; delete this plan
- [ ] `cd compiler && just`
