# Plan: Don't revert a whole library's type resolution because one unrelated declaration failed

## Problem

Several "recoverable" transform passes in `resolve_types()`
(`compiler/analyzer/src/stages.rs`) follow this pattern:

```rust
let fallback = library.clone();
match xform(library, &mut type_environment) {
    Ok(result) => library = result,
    Err(errs) => {
        diagnostics.extend(errs);
        library = fallback;
    }
}
```

`xform_resolve_late_bound_type_initializer::apply` is one of them. It
walks the *entire* merged library and resolves every `VAR`'s
placeholder type into a concrete kind (`FunctionBlock`, `Enumeration`,
etc.). If **any single declaration anywhere** fails to resolve (e.g. it
references a type that isn't declared in the compilation unit), the
function returns `Err(diagnostics)`:

```rust
let result = resolver.fold_library(lib).map_err(|e| vec![e]);
if !resolver.diagnostics.is_empty() {
    return Err(resolver.diagnostics);
}
result
```

This throws away `resolver.fold_library`'s `Ok` result even though it
already correctly resolved every *other* declaration. Back in
`resolve_types()`, `library = fallback` then reverts the **whole
library** to its pre-pass state -- undoing every successful resolution
from this pass, not just the one that failed.

## Example

```iecst
FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;   (* genuinely broken: Undeclared_Type doesn't exist anywhere *)
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR
    inst : FB_Callee;      (* perfectly valid, and completely unrelated to FB_A *)
END_VAR
    inst();
END_FUNCTION_BLOCK
```

Expected: one diagnostic, `P2008` on `FB_A.x`.

Actual: `FB_B.inst`'s initializer never gets resolved to
`InitialValueAssignmentKind::FunctionBlock` (the whole pass reverted
because of `FB_A.x`), so `rule_function_block_invocation.rs` never
recognizes `inst` as a declared FB instance, and `inst();` additionally
and incorrectly reports `P4012` ("not a variable in scope") -- despite
`FB_B` having nothing to do with `FB_A`.

Confirmed with a real, minimal 2-file reproduction from a private
TwinCAT corpus (not included here): two independent `FUNCTION_BLOCK`s,
one with a call to an undeclared type elsewhere in the same file (`x :
FB_SUNPOS;`, `FB_SUNPOS` not present in the compilation unit) and one
with a completely unrelated, valid FB-instance call (`nutate :
FB_IAU2000B; ... nutate(...);`) in the *same* file even -- the valid
call was misreported as `P4012` purely because of the other, unrelated
broken declaration. Removing the broken declaration (without touching
the valid one) made the false positive disappear; separately, supplying
the missing type declaration also fixed it. Neither file's own content
needed to change for the other to resolve correctly -- confirming this
is not really about merge *order*, but about whether the merged unit
contains *any* unrelated resolution failure at all.

## Why this matters beyond one test case

This is the root cause behind a "merge-order/pairing sensitivity" found
independently while corpus-testing multi-project compilation: pass
rates for a given file appeared to depend on which *other* unrelated
projects it happened to be checked alongside. It doesn't -- what
varies is whether that particular combination of files contains some
other, unrelated resolution failure, which then collaterally breaks
diagnostics for completely unconnected, valid code. Every real-world
multi-file/multi-project check is affected to some degree, since large
real corpora almost always have at least one genuine issue somewhere.

## Scope of the fix

`xform_resolve_late_bound_type_initializer` is the confirmed instance.
The same `let fallback = ...; ... Err(errs) => library = fallback`
pattern appears for other passes in `resolve_types()`
(`xform_resolve_type_decl_environment`, `xform_resolve_late_bound_expr_kind`,
`xform_fold_initializer_expressions`, `xform_int_to_bool_initializer`,
`xform_resolve_symbol_and_function_environment`,
`xform_named_to_positional_args`, and likely more found while
implementing) -- each needs checking for the same "diagnostics
non-empty implies discard the whole transform's output" shape, not
just this one pass.

## Fix approach

For `xform_resolve_late_bound_type_initializer::apply` specifically:
return `Ok(result)` from `fold_library` together with the accumulated
`resolver.diagnostics`, instead of turning any non-empty diagnostics
list into a hard `Err` that discards `result`. The caller
(`resolve_types`) should keep the transformed library and add the
diagnostics to its own list, exactly like it already does for the
`Ok` branch, rather than reverting to `fallback`.

Concretely, this likely means changing this class of pass's signature
from `Result<Library, Vec<Diagnostic>>` (all-or-nothing) to something
that can express "transformed, with some diagnostics" as a success
case -- e.g. returning `(Library, Vec<Diagnostic>)` unconditionally
and reserving `Err` for a genuine hard failure (one where continuing
would be unsound, if any such case exists for this pass at all). Needs
a pass-by-pass audit rather than a single shared helper, since each of
the "recoverable_xforms" may have different reasons for its current
`Result` shape.

## Tests

- New test reproducing the example above (unrelated broken declaration
  + valid FB-instance call in a different POU), asserting exactly one
  diagnostic (`P2008` on the broken one) and none on the valid one.
- Variant with both declarations in the *same* POU (matching the real
  corpus shape more closely).
- Regression tests confirming existing hard-failure diagnostics for
  each affected pass are unchanged (still reported, just without
  collateral damage to unrelated declarations).
- Full workspace suite must stay green -- this changes an early,
  foundational stage of the pipeline that many other passes build on.

## Out of scope

- Re-auditing every pass in the compiler for similar patterns outside
  `resolve_types()`'s `recoverable_xforms` list and the two calls
  immediately around it (`xform_resolve_constant_expressions`,
  `xform_fold_initializer_expressions`) -- start with the ones already
  confirmed or directly adjacent, expand only if the audit surfaces
  more.
