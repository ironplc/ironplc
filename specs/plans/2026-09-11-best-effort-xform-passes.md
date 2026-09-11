# Best-effort transform passes for constant type params and named arguments

Closes part 1 of [#1571](https://github.com/ironplc/ironplc/issues/1571).

## Goal

Stop `xform_resolve_constant_expressions` and `xform_named_to_positional_args`
from discarding a successfully transformed library when any one declaration or
call produces a diagnostic. Both currently return `Err(diagnostics)` *after* a
successful fold, and `stages::resolve_types` reverts to a pre-pass clone — so
one bad `STRING[MISSING]` or one mistyped named argument un-resolves every
unrelated declaration in the merged library.

Not user-visible in the CLI today (compilation aborts on the first error), but
it is exactly wrong for LSP live editing, where partial results are the point.

## Architecture

`xform_fold_initializer_expressions` already models the target shape:

```rust
pub fn apply(...) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>
```

`Ok((library, diagnostics))` means "here is the transformed library, and here
is what was wrong with parts of it". `Err` is reserved for a fold that could
not produce a library at all.

Keeping the partial result is strictly safer than reverting for both passes:

- **`xform_resolve_constant_expressions`** — a diagnosed reference is left as
  `IntegerRef::Constant`/`SignedIntegerRef::Constant`. Reverting leaves *every*
  reference in that state, including the ones that resolved, so it cannot be
  the safer option. Downstream handling of an unresolved `Constant` is already
  graceful: `rule_decl_subrange_limits` skips the node,
  `intermediates/array.rs` and `intermediates/subrange.rs` report a diagnostic,
  and codegen is never reached because `project::compile` gates it on an empty
  diagnostic list.
- **`xform_named_to_positional_args`** — a diagnosed call keeps its
  `NamedInput` entries. Reverting leaves every call in the library named,
  including valid ones. `xform_resolve_expr_types`,
  `xform_mark_unwritten_constants`, `call_assignment_check` and
  `codegen::compile_stmt` all have `NamedInput` arms already.

`xform_named_to_positional_args` also gates rewriting on the *library-wide*
error list (`if !self.errors.is_empty()`), so one duplicated named argument
stops every later call in the library from being rewritten. That gate becomes
user-visible once the result is kept, so it must become per-call.

## Prefactoring

`stages::resolve_types` repeats the same nine-line revert block nine times,
and the best-effort block twice. The two policies are distinguishable only by
reading the `match` arms, which is how `stages.rs:270` came to say
"Recoverable: convert named function call arguments to positional." above code
that reverts the whole merged library.

Prefactor first: extract two helpers whose *names* state the policy —
`run_reverting_on_error` and `run_best_effort` — and rewrite every call site
through them, with no behaviour change. The two conversions then become a
one-word change at the call site, and a comment can no longer contradict the
code without contradicting the function name next to it.

**That prefactor landed separately** in `#1690`, so what is left here is the
behaviour change.

## Design doc reference

No design document covers transform-pass failure policy. The convention is
stated in `specs/steering/compiler-architecture.md`; #1690 replaces the
sentence that said only "reverts to a pre-pass clone" with both policies, and
this branch adds what best effort means at declaration granularity. Per-pass
rationale stays in the pass doc comments, where the next editor of the pass
will read it.

## File map

| File | Change |
|---|---|
| `compiler/analyzer/src/stages.rs` | Keep the result for the two converted passes (the two helpers they are called through land in #1690) |
| `compiler/analyzer/src/xform_resolve_constant_expressions.rs` | `apply` returns `(Library, Vec<Diagnostic>)`; document why the partial result is kept; tests updated |
| `compiler/analyzer/src/xform_named_to_positional_args.rs` | `apply` returns `(Library, Vec<Diagnostic>)`; per-call duplicate gating; document why; tests updated |
| `specs/steering/compiler-architecture.md` | Record what best effort means at declaration granularity |

## Tasks

- [x] Prefactor: extract `run_reverting_on_error` and `run_best_effort` in `stages.rs`, route all recoverable passes through them, no behaviour change — landed separately in #1690
- [ ] Convert `xform_resolve_constant_expressions::apply` to the best-effort signature
- [ ] Convert `xform_named_to_positional_args::apply` to the best-effort signature
- [ ] Make the duplicate-named-argument gate per-call instead of library-wide
- [ ] Update the two call sites in `stages.rs` and fix the misleading comment at the named-args site
- [ ] Add tests: a diagnosed declaration/call alongside a valid one keeps the valid one transformed and reports exactly one diagnostic
- [ ] Update `specs/steering/compiler-architecture.md`
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
