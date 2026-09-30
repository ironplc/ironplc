# Plan: Multi-digit fields in direct addresses

Issue: #1928

## Goal

A direct address follows the IEC 61131-3 Edition 2 grammar (B.1.4.1):

```
direct_variable ::= '%' location_prefix size_prefix integer {'.' integer}
integer         ::= digit {['_'] digit}
```

so `%MW10`, `%IX12.7`, `%QD100` and `%IX1.2.3.4` parse, and the example on
the I/O qualifiers reference page (`speed_setpoint AT %MW10 : INT;`) checks
and compiles.

## Findings

- The `DirectAddress` token regex is `%[IQM]([XBWDL])?(\d(\.\d)*)`: one digit
  per field, so `%MW10` lexes as `%MW1` followed by the integer `0`.
- `AddressAssignment::try_from(&str)` repeats that regex, and also:
  - indexes the optional size-prefix capture group unconditionally, so an
    address without a size prefix (`%I0`, valid IEC) panics the compiler;
  - is case-sensitive while the token is case-insensitive, so `%ix0.0` lexes
    and then fails to parse;
  - `unwrap`s the `u32` parse, which becomes reachable (overflow) once fields
    may have several digits.
- Consumers already handle multi-digit fields (they hold `Vec<u32>`), but the
  address is formatted twice: `format_address` in the analyzer (symbol table,
  surfaced by the MCP `project_io`/`symbols` tools) and
  `visit_address_assignment` in the plc2plc renderer.
- LSP semantic tokens classify by token type; nothing to change once the
  token is right. The VS Code TextMate grammar already accepts `\d+`.
- Codegen allocates a located variable like any other variable; a direct
  variable used in an expression is still `todo` (unchanged, out of scope).
- The analyzer has no direct-address range check (`%IX0.8`): the bit/byte
  layout of a direct address is implementation-dependent in IEC 61131-3, so
  none is added here.
- The partial-access selectors (`.%X15`, `.%B3`) already accept `\d+`;
  the incomplete forms `%I*`/`%Q*`/`%M*` are a separate token and unaffected.

## Prefactoring

1. Move `LocationPrefix`, `SizePrefix` and `AddressAssignment` out of
   `dsl/src/common.rs` (over 4000 lines) into `dsl/src/direct_address.rs`,
   re-exported from `common` so no caller changes.
2. Give `AddressAssignment` a `Display` that spells the address in IEC form
   and use it from both the analyzer and the plc2plc renderer, replacing the
   two hand-written formatters.

## File map

- `compiler/dsl/src/direct_address.rs` (new), `compiler/dsl/src/common.rs`,
  `compiler/dsl/src/lib.rs`
- `compiler/parser/src/token.rs`
- `compiler/parser/src/tests/direct_address.rs` (new), `tests/mod.rs`,
  `tests/whitespace.rs` if an adjacency row is warranted
- `compiler/plc2plc/src/tests/direct_address.rs` (new), `tests/mod.rs`
- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`
- `compiler/plc2plc/src/renderer.rs`
- `compiler/codegen/tests/it/end_to_end_located_multi_digit.rs` (new)
- `compiler/mcp/src/tools/project_io.rs` (test)
- `docs/reference/language/variables/io-qualifiers.rst` (static example
  becomes a playground)

## Tasks

- [ ] Prefactor 1: extract direct-address types into their own module
- [ ] Prefactor 2: `Display` for `AddressAssignment`, used by analyzer and renderer
- [ ] Tests first: `AddressAssignment::try_from` unit tests (multi-digit,
      no size prefix, lower case, underscores, overflow rejected)
- [ ] Tests first: lexer tokenizes `%MW10`, `%IX12.7`, `%QD100`,
      `%IX1.2.3.4` as one token and does not swallow the following tokens
- [ ] Fix `AddressAssignment::try_from` without regex or `unwrap`
- [ ] Fix the `DirectAddress` token regex
- [ ] plc2plc round trip with multi-digit addresses
- [ ] End-to-end: the docs example compiles and runs
- [ ] MCP `project_io` reports `%QW12` for a multi-digit address
- [ ] Docs: make the io-qualifiers example a playground
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`; docs build
