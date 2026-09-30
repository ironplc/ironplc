# Plan: Located variables in CONFIGURATION and RESOURCE VAR_GLOBAL

Issue: https://github.com/ironplc/ironplc/issues/1913

## Goal

`name AT %QX0.0 : BOOL;` inside a `VAR_GLOBAL` block of a `CONFIGURATION`, a
`RESOURCE` (or a top-level `VAR_GLOBAL` under `--allow-top-level-var-global`)
parses, keeps its name and address, analyzes, compiles, runs, and round-trips
through plc2plc, the same way a located variable declared in a `PROGRAM`
does.

## Architecture

IEC 61131-3 Ed. 2, B.1.4.3:

```
global_var_decl ::= global_var_spec ':' [ located_var_spec_init | function_block_type_name ]
global_var_spec ::= global_var_list | [global_var_name] location
```

`global_var_spec` is an ordered PEG choice. For `lamp AT %QX0.0 : BOOL`,
`global_var_list` matches `lamp`, the choice commits, and `global_var_decl`
fails on `AT`. The located alternative is never reached with a name (it also
lacks the `_` between name and `AT`), and when it is reached it returns an
empty name and drops the address (`TODO this is clearly wrong`).

Fix: make the located form its own alternative of `global_var_decl`, tried
first, reusing `located_var_decl` (the same `[name] AT location :
located_var_spec_init` a `VAR` block uses) and marking the declaration
`VariableType::Global`. The symbolic form keeps the current list rule. The
broken located branch of `global_var_spec` goes away.

Downstream, nothing new is needed for the nominal path:

- Analyzer: `xform_resolve_symbol_and_function_environment` already records
  a located declaration's name and address in the symbol table for any
  scope, so the global is visible to `VAR_EXTERNAL` and to the MCP
  `project_io` tool (classified by its `%I`/`%Q`/`%M` address).
- Codegen: a located variable is a named variable slot (`symbolic_id()`);
  the VM has no separate process image. Configuration globals go through the
  same `assign_variables`, so a located global compiles and runs exactly
  like a program-level `AT` variable. Verified by hand with `ironplcc
  compile` + `ironplcvm run --dump-vars`.

One analyzer rule does need a change:
`rule_var_decl_global_const_requires_external_const` returns P9999 ("Located
CONSTANT declaration") for any located `VAR_GLOBAL CONSTANT`. That path was
unreachable while the parser rejected located globals; now it is reachable.
A named located constant is a constant global like any other, so the rule
should collect it by `symbolic_id()`. An unnamed one (`AT %MW8 : INT`)
cannot be referenced by a `VAR_EXTERNAL`, so there is nothing to check.

## Prefactoring

None separate. The grammar change *is* moving the located alternative up one
level and reusing `located_var_decl`; there is no shape to simplify first
without changing behaviour (removing the broken branch alone would change
what `AT %QX0 : BOOL` parses to).

## Relation to other open work

- #1915 (fix #1912) moves the plc2plc rendering of configuration and
  resource `VAR_GLOBAL` blocks to where the grammar expects them. On `main`
  configuration globals are not rendered at all, so a round trip of a
  located *configuration* global needs #1915. This PR round-trips a located
  top-level `VAR_GLOBAL` (same `global_var_decl` rule, same renderer path for
  `VariableIdentifier::Direct`) in a new file, and does not touch
  `renderer.rs` or #1915's `configuration_globals.rs`.
- #1902 accepts several resources; it touches `configuration_declaration()`
  and `whitespace.rs` at other lines.

## File map

- `compiler/parser/src/parser.rs` — `global_var_decl` fix
- `compiler/parser/src/tests/located_globals.rs` (new) + `tests/mod.rs`
- `compiler/parser/src/tests/whitespace.rs` — one row
- `compiler/analyzer/src/rule_var_decl_global_const_requires_external_const.rs`
- `compiler/plc2plc/src/tests/located_globals.rs` (new) + `tests/mod.rs`
- `compiler/codegen/tests/it/end_to_end_located_globals.rs` (new) + `main.rs`
- `compiler/mcp/src/tools/project_io.rs` — test for a located global
- `docs/reference/language/variables/io-qualifiers.rst` — located globals

## Tasks

- [ ] Parser tests (AST shape: name, address, `Global`, qualifier) — red
- [ ] Parser fix — green; whitespace row
- [ ] Analyzer rule tests for located CONSTANT globals — red, then fix
- [ ] plc2plc round trip of a located top-level global
- [ ] Codegen end-to-end: program reads and writes located config globals
- [ ] MCP `project_io` lists a located configuration global by address
- [ ] Docs: located globals in io-qualifiers.rst (playground example)
- [ ] Open issues for out-of-scope bugs found on the way
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`; docs build
