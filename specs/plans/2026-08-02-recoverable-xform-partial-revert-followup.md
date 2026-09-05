# Plan: Extend the partial-revert fix to the remaining recoverable xforms

## Background

specs/plans/2026-08-02-partial-resolution-revert-on-unrelated-error.md
fixed `xform_resolve_late_bound_type_initializer` (and, as a zero-risk
side effect, `xform_resolve_late_bound_expr_kind`): both passes used to
accumulate per-declaration diagnostics on `self` while folding, then
convert any non-empty diagnostics list into a hard `Err` that discarded
an already-successful fold result. `resolve_types()` would then revert
the *entire* merged library on that `Err`, so one genuinely broken,
unrelated declaration silently undid type resolution for every other
declaration in the compilation unit, producing collateral false
positives elsewhere (e.g. a valid FB-instance variable misreported as
"not a variable in scope").

Auditing the rest of `resolve_types()`'s `recoverable_xforms` list plus
the two adjacent calls (as the original plan's scope allowed) found
that three more passes have the *exact same* accumulate-then-discard
shape:

- `xform_resolve_constant_expressions` (`compiler/analyzer/src/xform_resolve_constant_expressions.rs`)
- `xform_fold_initializer_expressions` (`compiler/analyzer/src/xform_fold_initializer_expressions.rs`)
- `xform_named_to_positional_args` (`compiler/analyzer/src/xform_named_to_positional_args.rs`)

All three were **not** fixed in the first change, because — unlike
`xform_resolve_late_bound_type_initializer`'s `LateResolvedType`
fallback, which is a value later passes are explicitly designed to
recognize and re-diagnose — the "leave it unresolved" fallback value in
each of these three has not been verified safe for every downstream
consumer:

- `xform_resolve_constant_expressions`: on a soft error (undefined
  constant, non-integer constant type, out-of-scope constant, or the
  vendor-extension flag disabled) the node is left as
  `IntegerRef::Constant`/`SignedIntegerRef::Constant` rather than a
  resolved literal. It isn't yet confirmed what every downstream pass
  that pattern-matches on `IntegerRef`/`SignedIntegerRef` does when it
  sees a `Constant` variant it wasn't expecting at that stage (crash,
  a confusing internal-error diagnostic, or silent misbehavior, vs. a
  clean re-diagnosis).
- `xform_fold_initializer_expressions`: on a soft error the initializer
  is normalized to an uninitialized `Simple` (per the module's own
  doc comment), which looks like a deliberately-designed-safe
  fallback similar to `LateResolvedType` — but this needs to be
  confirmed by reading `fold_var_decl`/wherever that normalization
  happens, not assumed.
- `xform_named_to_positional_args`: on a soft error (duplicate named
  arg, undeclared named parameter) the call's `param_assignment` is
  left with `NamedInput` entries instead of being rewritten to
  positional. Downstream passes (codegen, `xform_resolve_expr_types`)
  may assume all arguments are positional by this point in the
  pipeline.

## Goal

For each of the three passes above: determine whether "keep the
partially-transformed library, surface the diagnostics, and let a
later pass or codegen deal with the un-normalized fallback value" is
safe, and if so, apply the same fix as the confirmed instance (return
`Ok((Library, diagnostics))` unconditionally, reserving `Err` for a
genuine fold failure). If it is *not* safe as-is, decide and implement
whatever additional normalization is needed (e.g. always force a safe
fallback shape on the soft-error path, independent of whether the pass
signature changes) so that the fix can still be applied without
downstream breakage.

## Approach

For each of the three passes, in isolation:

1. Read every downstream consumer of the AST node shape left behind by
   the soft-error path (`IntegerRef`/`SignedIntegerRef::Constant` for
   the constant-expressions pass; the initializer shape for the
   fold-initializer pass; `ParamAssignmentKind::NamedInput` for the
   named-to-positional pass) to confirm none of them panics, produces
   a misleading diagnostic, or silently miscompiles when it encounters
   that value un-normalized.
2. If downstream handling is already safe (a clean, if imprecise,
   diagnostic or otherwise inert), apply the same signature change as
   the confirmed fix: `Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>`,
   propagating a hard `Err` only for the pass's own genuine fold
   failure (already-existing `?`-propagated errors), and update the
   corresponding call site in `resolve_types()` (`compiler/analyzer/src/stages.rs`)
   to keep the transformed library and extend `diagnostics` instead of
   reverting.
3. If downstream handling is unsafe, decide per-pass whether to (a)
   normalize the soft-error fallback to something downstream already
   tolerates before applying the signature change, or (b) leave that
   specific pass on the current revert-on-error behavior with a
   comment recording why, if the collateral-damage risk is judged
   lower than the risk of an unhandled fallback.
4. Update each pass's existing unit tests that currently assert
   `unwrap_err()` / `is_err()` for soft-error scenarios — these will
   need to move to asserting `Ok` with diagnostics present, mirroring
   the updates already made to `xform_resolve_late_bound_type_initializer`'s
   tests. Preserve any test that legitimately expects a hard `Err`
   (e.g. a genuine fold failure, if one exists for that pass).

## Tests

- Add a same-shape regression test per fixed pass: an unrelated,
  independently-valid declaration elsewhere in the same merged library
  is unaffected by the pass's soft diagnostic on a different
  declaration (mirroring `xform_resolve_late_bound_type_initializer`'s
  two new tests and `stages.rs`'s new integration test).
- Full workspace suite (`cd compiler && just`) must stay green.

## Out of scope

- `xform_resolve_type_decl_environment` and
  `xform_resolve_symbol_and_function_environment` — already confirmed
  in the prior audit to not have this shape (the former stops at the
  first error via `?` rather than accumulating; the latter never
  mutates the library on the discarded path).
- Any pass outside `resolve_types()`'s `recoverable_xforms` list and
  the two calls immediately adjacent to it.
