# Replace the coarse `allow_iec_61131_3_2013` gate with per-feature flags

Tracking issue: [#1290](https://github.com/ironplc/ironplc/issues/1290)

## Motivation

`CompilerOptions.allow_iec_61131_3_2013` gates *all* Edition-3 features as a
single bundle. It is the one option field not produced by the
`define_compiler_options!` macro, and `from_dialect` sets it purely from
`dialect == Iec61131_3Ed3`, so it carries no information the `Dialect` doesn't
already imply. The coarse flag both under-selects (vendor dialects that want
one Ed3 feature can't get it without the whole edition) and over-selects
(enabling the edition to get `REF_TO` also drags in the long-time types).

The codebase already routes around this in `xform_demote_keywords.rs`, where
`demote_ref = !allow_iec_61131_3_2013 && !allow_ref_to` bolts a granular escape
hatch onto the coarse flag.

## Scope (this change)

Fold the two Edition-3 features the coarse flag actually gates today into the
descriptor system and delete the boolean:

- **Long-time-type keywords** (`LTIME`/`LDATE`/`LTOD`/`LDT`) → a new
  `allow_long_time_types` descriptor tagged `[Iec61131_3Ed3]`.
- **Reference keywords** (`REF_TO`/`REF`/`NULL`) → the existing `allow_ref_to`
  descriptor, now also tagged `Iec61131_3Ed3`.

`Dialect::Iec61131_3Ed3` becomes an ordinary preset assembled from the
descriptors that list it (explicit dialect tagging — the same mechanism
`allow_partial_access_syntax` already uses). No `since`/`Edition` metadata and
no ADR in this change; those remain follow-ups. OOP (`allow_fb_inheritance`,
issue #1287) is out of scope.

## Changes

### `compiler/parser/src/options.rs`
- Delete the hand-written `allow_iec_61131_3_2013` field and the
  `if dialect == Iec61131_3Ed3` special case in `from_dialect`.
- Add an `allow_long_time_types` descriptor (`--allow-long-time-types`,
  `[Iec61131_3Ed3]`).
- Add `Iec61131_3Ed3` to the `allow_ref_to` descriptor's dialect list.
- Drop the Ed3 special-casing in `describe_dialects` (Ed3 now has descriptors).
- Update the dialect flag-set tests (Ed2/Ed3/Rusty/Codesys/TwinCat).

### `compiler/parser/src/xform_demote_keywords.rs`
- `demote_time_types = !options.allow_long_time_types;`
- `demote_ref = !options.allow_ref_to;`
- Update doc comments and the `opts_edition3` test helper.

### CLI (`compiler/ironplc-cli/bin/main.rs`)
- Add the `--allow-long-time-types` flag and thread it into `compiler_options()`.

### Callers/tests updated to the new flags
- `compiler/parser/src/tests/common.rs`, `compiler/plc2plc/src/tests/common.rs`,
  `compiler/analyzer/src/rule_ref_to.rs`: the `edition3` helpers use
  `CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)`.
- `compiler/mcp/src/tools/common.rs`, `compiler/ironplc-cli/src/lsp.rs`:
  assertions switch from `allow_iec_61131_3_2013` to `allow_long_time_types`.

## Behavior preservation

`from_dialect(Iec61131_3Ed3)` still yields long-time types + `REF_TO`/`REF`/
`NULL` + partial-access, identical to the old edition boolean.
