# Add CODESYS Dialect

## Goal

Add a `codesys` dialect preset that selects the vendor-extension flags needed to
parse code written for the CODESYS IDE. CODESYS is one of the most popular
IEC 61131-3 environments; offering a named preset documents intent and avoids
users having to remember the longer `rusty` alias.

## Decision: which flags to enable

CODESYS supports the same syntactic vendor extensions that the existing `rusty`
preset enables, with one exception:

- `allow_system_uptime_global` is **not** enabled. The implicit
  `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME` globals are an IronPLC runtime
  convention (matching OSCAT/RuSTy), not a CODESYS feature. CODESYS exposes
  monotonic time via its own SysLib functions, so we do not pre-bind those
  identifiers for the CODESYS dialect.

The remaining 14 vendor-extension flags map to documented CODESYS behaviour:

| Flag | CODESYS rationale |
|---|---|
| `allow_c_style_comments` | CODESYS accepts `//` and `/* */` |
| `allow_missing_semicolon` | CODESYS is lenient after `END_IF`, `END_STRUCT` |
| `allow_top_level_var_global` | CODESYS GVLs declare `VAR_GLOBAL` outside CONFIGURATION |
| `allow_constant_type_params` | `STRING[CONST]` is idiomatic CODESYS |
| `allow_empty_var_blocks` | CODESYS allows empty `VAR…END_VAR` |
| `allow_time_as_function_name` | OSCAT (CODESYS-hosted) declares `TIME()` |
| `allow_ref_to` | `REF_TO`, `REF()`, `NULL` are CODESYS extensions |
| `allow_ref_arithmetic` | CODESYS allows pointer arithmetic |
| `allow_ref_stack_variables` | CODESYS allows `REF()` on local variables |
| `allow_ref_type_punning` | CODESYS allows reinterpreting via `REF_TO` |
| `allow_int_to_bool_initializer` | Documented in source as "CoDeSys, TwinCAT, RuSTy" |
| `allow_sizeof` | Documented in source as "CODESYS, TwinCAT, RuSTy" |
| `allow_cross_family_widening` | CODESYS allows implicit BYTE↔INT widening |
| `allow_partial_access_syntax` | CODESYS supports `.%Xn` (also IEC Ed. 3) |

Edition 2 stays as the base so identifiers like `LDT` remain usable (matching
the RuSTy approach for OSCAT-style code).

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | Add `Dialect::Codesys` variant, update `ALL`, `display_name`, `description`, `fmt::Display`; add `Codesys` to 14 flag dialect lists; add unit tests |
| `compiler/ironplc-cli/bin/main.rs` | Add `CliDialect::Codesys` variant and `to_dialect` arm |
| `compiler/ironplc-cli/src/lsp.rs` | Match `"codesys"` in `extract_compiler_options`; add LSP test |
| `compiler/codegen/tests/it/end_to_end_dialect.rs` | Add end-to-end tests for CODESYS dialect |
| `specs/steering/syntax-support-guide.md` | Add CODESYS row to dialect table |

## Tasks

- [x] Create plan
- [ ] Add `Dialect::Codesys` and wire it into the macro flag lists (omit `allow_system_uptime_global`)
- [ ] Add unit tests covering the CODESYS flag set
- [ ] Add `CliDialect::Codesys` to the CLI
- [ ] Add `"codesys"` to LSP `extract_compiler_options`
- [ ] Add end-to-end dialect tests
- [ ] Update steering guide dialect table
- [ ] Run `cd compiler && just` and verify all checks pass
