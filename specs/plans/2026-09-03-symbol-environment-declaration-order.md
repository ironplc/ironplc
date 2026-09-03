# Plan: Return Symbols in Declaration Order

## Context

The MCP `symbols` and `project_io` tools return program and variable
lists in a different order on every run
([#1577](https://github.com/ironplc/ironplc/issues/1577)).

### Root cause

`SymbolEnvironment` in `compiler/analyzer/src/symbol_environment.rs`
stores its symbols in `std::collections::HashMap`s:

- `global_symbols: HashMap<Id, SymbolInfo>`
- `scoped_symbols: HashMap<ScopeKind, HashMap<Id, SymbolInfo>>`

`HashMap` uses a randomly seeded hasher per process, so iteration order
changes between runs. Every read-only accessor that returns a list
(`get_programs`, `get_function_blocks`, `get_variables_in_scope`,
`get_enumeration_values_for_type`, `get_structure_fields_for_type`)
iterates these maps directly and inherits that order. `all_symbols()`
also iterates the outer `scoped_symbols` map, so the cross-scope lookups
are nondeterministic even within a single scope's contents.

## Approach

Replace the three `HashMap`s with `indexmap::IndexMap`, which keeps O(1)
lookup by key and iterates in insertion order. Symbols are inserted as
the analyzer walks the library, so insertion order is declaration order,
which is the order users expect to see.

`IndexMap::insert` on an existing key updates the value in place without
moving it. `SymbolEnvironment` currently allows redefinition, so a
redefined symbol keeps its first-declared position.

`indexmap` is already in `Cargo.lock` as a transitive dependency of
`petgraph` (a direct dependency of `analyzer`) and `rmcp`, so adding it as
a direct dependency compiles no new code.

### Prefactoring

None needed. The three insert paths already share a single
`entry(...).or_default()` shape and the accessors are thin `iter()`
filters, so the type swap drops in without restructuring.

## Steps

1. Add `indexmap = "2"` to `compiler/analyzer/Cargo.toml`.
2. Swap `HashMap` for `IndexMap` in `symbol_environment.rs`.
3. Add tests asserting that `get_programs`, `get_variables_in_scope` and
   `get_enumeration_values_for_type` return symbols in declaration order,
   with enough symbols that a hashed order is very unlikely to match by
   chance.
4. Run `cd compiler && just`.

## Out of scope

- Duplicate-symbol detection (existing `TODO`s in the insert paths).
- Ordering of anything outside `SymbolEnvironment`.
