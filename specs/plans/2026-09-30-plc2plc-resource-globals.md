# Plan: render configuration and resource VAR_GLOBAL where the grammar expects it

Issue: #1912

## Problem

`visit_resource_declaration` writes the resource's globals after its tasks and
programs, one `VAR_GLOBAL` block per variable. The grammar accepts at most one
block, before the tasks. `visit_configuration_declaration` never writes
`ConfigurationDeclaration::global_var` at all.

## Steps

1. Tests first (`plc2plc/src/tests/configuration_globals.rs`): resource with one
   global, several globals, a `RETAIN` block, and configuration-level globals
   together with a resource block and `VAR_CONFIG`. All fail today.
2. Prefactor: split the declaration line out of `visit_var_decl` so a block
   writer can reuse it.
3. Add a helper that writes one `VAR_GLOBAL [qualifier] ... END_VAR` block for
   a list of declarations (nothing when empty).
4. Use it in `visit_configuration_declaration` (before the resources) and in
   `visit_resource_declaration` (before the tasks).

## Out of scope

- Located globals (`x AT %QX0.0 : BOOL`) inside a configuration: the parser's
  `global_var_spec` does not keep the name or address, so they cannot round
  trip until the parser is fixed.
- `VAR_ACCESS` in a configuration is not parsed yet.
- Program configuration elements (`PROGRAM p : T (IN := x)`) are not rendered.
