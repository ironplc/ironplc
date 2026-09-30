# Accept More Than One RESOURCE in a CONFIGURATION

Issue: #1856

## Goal

Parse a `CONFIGURATION` that declares more than one `RESOURCE`. Today the
second `RESOURCE` is P0002, which stops `check` and the editor on valid
IEC 61131-3 source.

## Architecture

IEC 61131-3 Edition 2, B.1.7:

```
configuration_declaration ::= 'CONFIGURATION' configuration_name
    [global_var_declarations]
    (single_resource_declaration | (resource_declaration {resource_declaration}))
    [access_declarations]
    [instance_specific_initializations]
    'END_CONFIGURATION'
```

`ConfigurationDeclaration::resource_decl` is already a `Vec`, and every
consumer (analyzer rules, `xform_resolve_constant_expressions`, the plc2plc
renderer, codegen, the MCP server, the PLCopen XML transform) already
iterates it. Only the grammar builds a one-element vector. The change is
confined to `configuration_declaration()` in `parser/src/parser.rs`: match
`resource_declaration() ++ _` instead of a single `resource_declaration()`.

Downstream behaviour once it parses:

- **Analyzer**: `rule_task_names_unique` and
  `rule_program_task_definition_exists` visit each `ResourceDeclaration`, so
  task names are scoped per resource, as the standard requires. No change.
- **Codegen**: every resource declares at least one program instance
  (`single_resource_declaration` requires one), so two resources always mean
  two instances, which `check_single_program_instance` already rejects with
  P9999 pointing at the tracking issue #1613. No silent misbehaviour; no
  change.

Out of scope (separate grammar gaps, not reported by the issue): the bare
`single_resource_declaration` form (no `RESOURCE` keyword) and
`access_declarations` in a configuration.

## Prefactoring

None needed. The change replaces one grammar term and deletes two stale
`TODO` comments; there is no repeated branching to consolidate, and the
edit does not grow `parser.rs`.

## Design doc reference

None; the configuration grammar has no design document.

## File map

- `compiler/parser/src/parser.rs` — grammar change
- `compiler/parser/src/tests/configuration.rs` (new) + `tests/mod.rs` — AST shape
- `compiler/parser/src/tests/whitespace.rs` — row for the new `_`
- `compiler/plc2plc/src/tests/configuration.rs` (new) + `tests/mod.rs` — round trip
- `compiler/analyzer/src/rule_task_names_unique.rs`,
  `compiler/analyzer/src/rule_program_task_definition_exists.rs` — per-resource scoping tests
- `compiler/codegen/tests/it/compile_program_count.rs` — P9999 across resources
- `docs/reference/language/pous/configuration.rst` — single-program note

## Tasks

- [ ] Parser test: two resources parse to two `ResourceDeclaration`s (fails first)
- [ ] Grammar: `resource_declaration() ++ _`
- [ ] Whitespace row between two resources
- [ ] plc2plc round trip of a two-resource configuration
- [ ] Analyzer: same task name in two resources is accepted; a program
      referencing another resource's task is P4006
- [ ] Codegen: two resources give P9999 on the second instance
- [ ] Docs: include the single-program limitation on the CONFIGURATION page
- [ ] `git rm` this plan
- [ ] `cd compiler && just`
