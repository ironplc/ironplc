# MCP `symbols` Tool Implementation Plan

## Context

The IronPLC MCP server needs a `symbols` tool that returns the full symbol table
for a set of sources. This plan extends the analyzer's `SymbolEnvironment` and
`TypeEnvironment` to carry the data the tool needs (variable direction and
hardware address), then builds the MCP tool on top of those environments.

Design reference: `specs/design/mcp-server.md` (REQ-TOL-050..055, REQ-ARC-050, REQ-ARC-060)

## Part 1: Extend SymbolInfo (Analyzer)

- Add `variable_type: Option<VariableType>` and `address: Option<String>` to `SymbolInfo`
- Add `insert_variable` method to `SymbolEnvironment`
- Update `visit_var_decl` in `xform_resolve_symbol_and_function_environment.rs` to
  populate the new fields, including handling Direct variables (existing TODO)

## Part 2: MCP symbols Tool

- Create `compiler/mcp/src/tools/symbols.rs`
- Extract programs/FBs from `SymbolEnvironment::get_global_symbols()` + scoped variables
- Extract functions from `FunctionEnvironment::iter()` (filter `!is_stdlib()`)
- Extract types from `TypeEnvironment::iter()` (filter non-elementary, classify by `IntermediateType`)
- Implement `pou` filter (REQ-TOL-054) and 256 KiB response cap (REQ-TOL-055)
- Register tool in `server.rs`, fill spec conformance tests, add CLI integration tests

## Verification

```bash
cd compiler && just
```
