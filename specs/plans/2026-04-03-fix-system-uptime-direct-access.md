# Fix P4007: Allow direct access to `__SYSTEM_UP_TIME` without `VAR_EXTERNAL`

## Goal

Allow programs to reference `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` directly
(without `VAR_EXTERNAL` declarations) when `--allow-system-uptime-global` is
enabled or `--dialect rusty` is used.

## Architecture

The implicit globals are already registered in the `SymbolEnvironment` and
injected by codegen/VM. Two analyzer passes need to be made aware of them:

1. **`rule_use_declared_symbolic_var`** — seed the `ScopedTable` root scope so
   references don't trigger P4007.
2. **`xform_resolve_expr_types`** — seed `var_types` so expression type
   resolution knows `__SYSTEM_UP_TIME` is `TIME` and `__SYSTEM_UP_LTIME` is
   `LTIME`.

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` | Seed ScopedTable with implicit globals |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Accept `CompilerOptions`, seed var_types |
| `compiler/analyzer/src/stages.rs` | Pass options to `xform_resolve_expr_types` |
| `compiler/codegen/tests/end_to_end_system_uptime.rs` | Add direct-access end-to-end test |

## Tasks

- [ ] Seed implicit globals in `rule_use_declared_symbolic_var`
- [ ] Add `CompilerOptions` to `xform_resolve_expr_types` and seed var_types
- [ ] Update `stages.rs` to pass options
- [ ] Add unit test in `rule_use_declared_symbolic_var`
- [ ] Add end-to-end test for direct access without `VAR_EXTERNAL`
- [ ] Run full CI pipeline
