# Plan: TwinCAT Constant-Initializer Dialect Gap and Initializer-Fold Revert Bug

## Goal

Fix the two defects found while validating the real `PiExample_sln` TwinCAT
project from issue #1199 (see
[2026-08-06-twincat-solution-library-e2e-test.md](2026-08-06-twincat-solution-library-e2e-test.md)):

1. **Dialect gap:** `--dialect twincat` rejects `VAR` constant-expression
   initializers (`d2r : LREAL := PI/180.0;`) with P4037, but real TwinCAT
   accepts them — the same reasoning that already placed the flag in the
   `codesys` dialect (TwinCAT 3 runs on the CODESYS V3 runtime). The
   `twincat` dialect should enable
   `allow_constant_initializer_expressions`.
2. **Internal-error bug:** whenever `xform_fold_initializer_expressions`
   diagnoses anything (P4037/P4038/P4039), `stages.rs` treats the transform
   as failed and reverts to the *pre-transform* library. That breaks the
   transform's own invariant ("no other pass ever observes `SimpleExpr`"):
   the surviving `SimpleExpr` nodes reach
   `rule_var_decl_const_initialized`, which raises a P9998 internal error
   after every legitimate P4037. Reproduced with the real project under
   `--dialect twincat`.

## Non-goals

- Changing which other dialects enable the flag, or any other flag's dialect
  membership.
- Revisiting the revert-on-error convention for other transforms; only this
  transform's contract ("always normalizes, diagnostics are per-declaration")
  makes revert wrong.

## Approach

### 1. Return the normalized library alongside diagnostics

Change `xform_fold_initializer_expressions::apply` to the recoverable-xform
signature already used in `stages.rs`
(`Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>`): the normalized
library and the collected diagnostics travel together, and `stages.rs`
extends diagnostics without reverting. The transform normalizes every
`SimpleExpr` even when it diagnoses, so the Ok value is always well-formed.

Additionally, when the flag is disabled the transform currently discards the
initializer value entirely (`initial_value: None`), which would cascade into
a misleading "constant must have initializer" error on `VAR CONSTANT`
declarations that *do* have one. Attempt the fold anyway in that arm — P4037
is still emitted and still fails the build, but a foldable value is kept so
downstream rules see the declaration as initialized.

### 2. Make the rule's `SimpleExpr` arm factual instead of an internal error

A `SimpleExpr` *is* an initializer, so `rule_var_decl_const_initialized`
treating it as "not initialized — internal error" is wrong on any path where
it is still observable (today: the revert path; after this fix: only a hard
transform failure). Treat it as initialized.

### 3. Enable the flag in the `twincat` dialect

Tag `allow_constant_initializer_expressions` with `TwinCat` in the
`define_compiler_options!` macro, and update the exact-flag dialect test and
the dialect documentation
(`docs/explanation/enabling-dialects-and-features.rst`).

### 4. Tests

- Stages-level regression: analyzing `VAR CONSTANT x : LREAL := 3.0/2.0;`
  with the flag disabled yields P4037 and *no* P9998 (and no cascaded
  constant-must-have-initializer error).
- Update the existing transform unit tests for the new `apply` signature.
- Switch the issue-#1199 end-to-end solution tests from `--dialect codesys`
  to `--dialect twincat`, which now checks the real-world initializer form
  clean — verified against the actual `PiExample_sln` project.

## Risks

- Downstream passes now run over libraries that carry initializer
  diagnostics (they previously ran over the reverted originals in that
  case). The normalized shape is the same one produced on the happy path, so
  this strictly reduces the states other passes can observe.
