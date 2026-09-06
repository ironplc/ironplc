# Plan: `STRING_TO_UDINT` selects its conversion by policy; document the policies

Slice 5 of 5 of the `STRING_TO_UDINT` steel thread for
[ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md);
fixes [#1592](https://github.com/ironplc/ironplc/issues/1592). The slices,
in order: the `CodegenOptions` / VM prefactor; the policy enums and func_id
block; the compiler options and front ends; the VM scanner and trap;
**this codegen and documentation slice**.

## Goal

Close the thread: `STRING_TO_UDINT` compiles to the `CONV_STR_TO_U32_*`
builtin its two policies select, so the upper half of the UDINT range
converts, a non-convertible string traps or yields zero as selected, and a
`.iplc` carries its own behavior. Document the policies for users and flip
ADR-0049 to accepted.

## Architecture

- `CodegenOptions` carries the two policies (derived from `CompilerOptions`
  by the `From` impl the prefactor slice introduced) and threads them to
  `CompileContext`; the `StringToNum` arm routes the 32-bit unsigned target
  to `builtin::str_to_num::func_id`. Signed and sub-32-bit targets keep the
  single-encoding builtin until their own PRs.
- End-to-end tests run one source under every alternative, including the
  #1592 inputs; a codegen conformance test shows the same source under two
  selections differs only in the func_id operand (ADR-0049 confirmation 1).
- The website documents the policies on the type-conversion page (results
  per alternative, dialect selections, the CODESYS undefined-range
  divergence), the flags on the CLI page, and the selections per dialect.

## Prefactoring

Done in slice 1 (`CodegenOptions: From<&CompilerOptions>`), which is why
adding the policies to codegen touches one constructor.

## Design doc reference

`specs/design/behavior-policies.md` (the codegen-owned sections and the
"Adding a policy" checklist).

## File map

| File | Change |
|---|---|
| `compiler/codegen/src/compile.rs`, `compile_call.rs`, `lib.rs` | policies on `CodegenOptions` and `CompileContext`; U32 selection |
| `compiler/codegen/build.rs`, `src/spec_conformance_behavior_policies.rs` (new) | REQ-BP-codegen tests |
| `compiler/codegen/tests/it/end_to_end_string_to_udint.rs` (new), `main.rs`, `end_to_end_tc2_utilities.rs` | end-to-end matrix; options construction |
| `compiler/vm-cli/tests/cli.rs` | V4006 exit code and message from a compiled program |
| `docs/reference/standard-library/functions/type-conversions.rst`, `docs/reference/compiler/ironplcc.rst`, `docs/explanation/enabling-dialects-and-features.rst` | user documentation |
| `specs/adrs/0049-*.md` | `accepted`, postscript on what landed |

## Tasks

- [ ] Thread policies; select the builtin; conformance and e2e tests
- [ ] vm-cli trap test; user docs; ADR status
- [ ] `cd compiler && just`; docs build; delete this plan
