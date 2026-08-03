# Plan: `LTRUNC`/`LMOD` Stdlib Functions

## Goal

`LTRUNC(x)` and `LMOD(x, y)` are undeclared function calls today (`P4017`),
blocking files that use them. They were assumed (per the originating
survey) to be plain missing entries in the existing
`FunctionSignature::stdlib(...)` table, "same shape as the already-registered
`TRUNC`" — i.e. generic `ANY_REAL`/`ANY_NUM` functions like the rest of the
core arithmetic table.

## Verification against real documentation

**That assumption doesn't hold.** Checked Beckhoff's own `Tc2_Math` library
documentation directly before implementing (per the standing "verify before
assuming" habit — the same kind of correction already made this session for
the EXTENDS/IMPLEMENTS bucket and for `PI`):

- `LTRUNC` — `FUNCTION LTRUNC : LREAL`, parameter `lr_in : LREAL`. Unlike
  `TRUNC` (which returns `ANY_INT`, clamped to an integer type's range),
  `LTRUNC` returns `LREAL` — it truncates the fractional part but keeps
  the result as a float, specifically so values outside an integer type's
  range don't overflow.
- `LMOD` — `FUNCTION LMOD : LREAL`, parameters `lr_Value : LREAL`,
  `lr_Arg : LREAL`. Unlike `MOD` (integer-oriented), `LMOD` performs a
  floating-point modulo and can return a non-integer remainder (Beckhoff's
  own example: `LMOD(400.56, 360) = 40.56`).

Both are **`Tc2_Math` library functions** — a specific, named Beckhoff PLC
library a TwinCAT project must reference — not core IEC 61131-3 functions,
and not generic-CODESYS-core the way pragmas or `SIZEOF` are. They also
aren't generic over `ANY_REAL`/`ANY_INT` the way `TRUNC`/`MOD` are: both
operate on `LREAL` only, matching their real signatures.

This changes the shape of the fix: not "two more rows in the always-on
core arithmetic table," but two new vendor-extension function signatures,
gated behind their own flag — the same treatment `SIZEOF` and `PI` already
get (`allow_sizeof`, `allow_math_constants`), not folded into the
unconditional `get_trunc_function()`/`get_arithmetic_functions()` tables.

## Design

### New flag: `--allow-extended-math-functions`

A new `CompilerOptions` field via the existing `define_compiler_options!`
macro, enabled by default under `[Rusty, Codesys]` (matching `allow_sizeof`).
Kept separate from `allow_math_constants` (PI) since that flag's own
description is specifically about constants, not functions — conflating
the two would make either flag's name misleading.

### New function signatures (`LREAL`-only, not generic)

```rust
pub fn get_extended_math_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "LTRUNC",
            TypeName::from("LREAL"),
            vec![input_param("IN", "LREAL")],
        ),
        FunctionSignature::stdlib(
            "LMOD",
            TypeName::from("LREAL"),
            vec![input_param("IN1", "LREAL"), input_param("IN2", "LREAL")],
        ),
    ]
}
```

(Parameter names normalized to IronPLC's existing `IN`/`IN1`/`IN2`
convention, matching `TRUNC`/`MOD`, rather than Beckhoff's own
`lr_in`/`lr_Value`/`lr_Arg` — IronPLC's stdlib table doesn't preserve
vendor parameter names for any other function either, and call sites in
real code use positional args, not named ones, for these two.)

### Registration: conditional, in `stages.rs`, alongside `SIZEOF`

```rust
if options.allow_extended_math_functions {
    use crate::intermediates::stdlib_function::get_extended_math_functions;
    for sig in get_extended_math_functions() {
        function_environment
            .insert(sig)
            .expect("LTRUNC/LMOD should not conflict with stdlib");
    }
}
```

## Non-goals

- Any other `Tc2_Math` library function beyond `LTRUNC`/`LMOD` — no other
  such function appears in the originating survey's failure set.
- `NCError_TO_STRING` (same 5-file bucket in the survey) — a project-local
  function, not a stdlib gap; explicitly out of scope here.
- Generic widening/family-conversion support for `LTRUNC`/`LMOD` beyond
  their real, `LREAL`-only signatures — Beckhoff's own docs don't define
  them generically, so IronPLC shouldn't either.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | New `allow_extended_math_functions` flag |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | New `get_extended_math_functions()` |
| `compiler/analyzer/src/stages.rs` | Conditional registration alongside `SIZEOF` |
| `docs/explanation/enabling-dialects-and-features.rst` | Document the new flag |

## Testing Strategy

- Unit test on `get_extended_math_functions()`: correct names, param
  counts/types, `LREAL` return type (mirrors existing `stdlib_function.rs`
  tests for other functions).
- Semantic test: `LTRUNC(x)`/`LMOD(x, y)` resolve without error when the
  flag is on; still `P4017` (undeclared) when the flag is off.
- Regression: flag off by default under `iec61131-3-ed2`/`iec61131-3-ed3`
  (matches every other vendor-extension flag's default-off test pattern).

## Tasks

- [x] Write plan (this document)
- [x] New `allow_extended_math_functions` flag
- [x] `get_extended_math_functions()` in `stdlib_function.rs`
- [x] Conditional registration in `stages.rs`
- [x] Tests from Testing Strategy
- [x] Docs
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- The originating survey's "same shape as `TRUNC`" assumption didn't
  survive contact with Beckhoff's own `Tc2_Math` documentation — checked
  before writing any code, per the standing "verify before assuming"
  habit. `LTRUNC`/`LMOD` are `LREAL`-only, Beckhoff-library-specific
  functions, not generic IEC 61131-3 stdlib entries, so they got their
  own gated flag (`allow_extended_math_functions`) rather than joining
  the always-on `get_arithmetic_functions()`/`get_trunc_function()`
  tables.
- `CompilerOptions` uses a macro (`define_compiler_options!`) that
  auto-generates the CLI/LSP/MCP wiring from one table entry — no
  exhaustive-match ripple from adding the flag itself. Three hardcoded
  flag-count assertions still needed manual bumps, found by running the
  full test suite, not by grep: `options.rs`'s own
  `feature_descriptors_when_called_then_contains_all_vendor_flags`
  (20→21), `feature_descriptors_when_rusty_then_all_features_listed`
  (20→21), `feature_descriptors_when_codesys_then_omits_only_system_uptime_global`
  (19→20), plus `mcp/src/tools/list_options.rs`'s
  `build_response_when_called_then_contains_all_flags` (20→21) and its
  `Vec::with_capacity(20)` — this is the same silent-drift hazard the
  `twincat-status.md` "Rebase conflict resolution reference" section
  already documents from earlier branches in this stack.
- Verified end-to-end via the CLI: `LTRUNC`/`LMOD` resolve cleanly under
  `--dialect=codesys`, and produce `P4017 Function is not declared` under
  the default dialect (flag off) — confirming the gating works both ways.
