# Enumeration Code Generation — Implementation Plan

## Goal

Add code generation, execution, and debug support for user-defined enumerations so programs using `TYPE ... END_TYPE` enumeration declarations can compile and run end-to-end, with the playground displaying both the integer ordinal and the human-readable value name.

## Architecture

Enumerations compile to DINT integer operations with no new VM opcodes. An ordinal map built at codegen entry resolves enum value names to 0-based integers. A new Tag 9 (ENUM_DEF) debug section sub-table enables the playground to display value names.

## Design doc reference

[specs/design/enumeration-codegen.md](../design/enumeration-codegen.md)

## File map

| File | Action | Purpose |
|------|--------|---------|
| `compiler/codegen/src/compile_enum.rs` | New | Ordinal map builder, resolution helpers |
| `compiler/codegen/src/compile.rs` | Modify | Add `enum_map` to `CompileContext`, call builder |
| `compiler/codegen/src/lib.rs` | Modify | Add `mod compile_enum` |
| `compiler/codegen/src/compile_setup.rs` | Modify | Handle `EnumeratedType` in assign/init |
| `compiler/codegen/src/compile_expr.rs` | Modify | Compile `EnumeratedValue` expressions |
| `compiler/codegen/src/compile_stmt.rs` | Modify | Compile enum CASE selectors |
| `compiler/codegen/src/compile_struct.rs` | Modify | Compile enum struct field init |
| `compiler/container/src/debug_section.rs` | Modify | Add `EnumDefEntry`, Tag 9 read/write |
| `compiler/container/src/builder.rs` | Modify | Add `add_enum_def()` |
| `compiler/playground/src/lib.rs` | Modify | Enum-aware `format_variable_value` |
| `specs/design/bytecode-container-format.md` | Modify | Add Tag 9 to Tag Registry |
| `compiler/codegen/tests/end_to_end_enum.rs` | New | End-to-end tests |

## Tasks

- [ ] **PR 1: Ordinal mapping infrastructure** — Create `compile_enum.rs` with `build_enum_ordinal_map()`, `resolve_enum_ordinal()`, `enum_var_type_info()`, `resolve_enum_default_ordinal()`. Add `enum_map` field to `CompileContext`. (REQ-EN-001–004, REQ-EN-080–083)
- [ ] **PR 2: Variable allocation + initialization** — Handle `InitialValueAssignmentKind::EnumeratedType` in `assign_variables()`, `emit_initial_values()`, and `emit_function_local_prologue()`. (REQ-EN-010–012, REQ-EN-020–023)
- [ ] **PR 3: Expressions + CASE selectors** — Replace `todo!()` in `compile_expr.rs` for `ExprKind::EnumeratedValue` and in `compile_stmt.rs` for `CaseSelectionKind::EnumeratedValue`. Add enum fallback for CASE `op_type` resolution. (REQ-EN-030–034, REQ-EN-040–041)
- [ ] **PR 4: Struct field enum init** — Replace pass-through in `compile_struct.rs` for `StructInitialValueAssignmentKind::EnumeratedValue`. (REQ-EN-050–051)
- [ ] **PR 5: Debug enum table + playground** — Add Tag 9 (ENUM_DEF) to debug section. Update `ContainerBuilder`. Emit enum definitions from codegen. Update playground to display value names. Update container format spec. (REQ-EN-060–064, REQ-EN-070–072)
