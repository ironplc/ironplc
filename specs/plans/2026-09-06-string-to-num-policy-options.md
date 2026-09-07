# Plan: behavior policies on `CompilerOptions`, and their CLI, LSP and MCP surfaces

Slice 3 of 5 of the `STRING_TO_UDINT` steel thread for
[ADR-0049](../adrs/0049-behavior-policies-selected-at-compile-time.md)
(fixes [#1592](https://github.com/ironplc/ironplc/issues/1592) when the
series lands). The slices, in order: the `CodegenOptions` / VM prefactor;
the policy enums and func_id block; **this options slice**; the VM scanner
and trap; codegen selection and user documentation.

## Goal

Make the two `STRING_TO_<numeric>` policies selectable: fields on
`CompilerOptions` with the dialect presets, a `POLICY_DESCRIPTORS` table,
and the `--policy-*` flags, LSP keys and MCP option entries derived from it.
Codegen does not read the selection yet, so this slice changes no compiled
program; the website documents the flags when the behavior lands.

## Architecture

- `define_compiler_options!` gains a `policies { ... }` section next to the
  flag rows. One row per policy names its enum, the alternative each vendor
  preset selects, and its option key; the macro generates the field, the
  preset mapping and the descriptor. A policy selection *replaces* the
  preset's, unlike a flag, which only enables.
- The CLI adds one `clap::ValueEnum` newtype per policy (the orphan rule
  keeps clap out of the parser crate), the LSP reads a string under the
  lowerCamelCase key, and the MCP server lists policies as `enum` options
  with `allowed_values`. Each surface is guarded by a drift test over
  `POLICY_DESCRIPTORS`.
- The glossary gains *behavior policy*; the syntax guide points at it so a
  future vendor difference in runtime behavior is not made into a flag.

## Prefactoring

None needed: the macro is the one place that maps dialects to options, and
the second section slots in beside the first without touching the flag rows.

## Design doc reference

`specs/design/behavior-policies.md` (the parser-owned sections).

## File map

| File | Change |
|---|---|
| `compiler/parser/Cargo.toml`, `src/options.rs` | container dependency, policy fields, presets, descriptors, accessors, `describe_dialects` |
| `compiler/parser/src/lib.rs`, `build.rs`, `src/spec_conformance_behavior_policies.rs` (new) | REQ-BP-parser tests |
| `compiler/ironplc-cli/bin/main.rs`, `src/lsp.rs` | flags, LSP keys, drift tests |
| `compiler/mcp/src/tools/common.rs`, `tools/list_options.rs`, `spec_conformance.rs` | string-valued policy options |
| `specs/steering/glossary.md`, `syntax-support-guide.md` | term and pointer |

## Tasks

- [ ] Options, presets, descriptors, accessors with tests
- [ ] CLI, LSP, MCP wiring with drift tests
- [ ] Glossary, design-doc sections
- [ ] `cd compiler && just`; delete this plan
