# Plan: `BOOL_TO_STRING`, `LREAL_TO_FMTSTR`, `ADR` Stdlib Functions

## Goal

Three "undeclared function" gaps flagged by a separate corpus-check pass
against the private test corpus: `BOOL_TO_STRING` (5 files, 72 call
sites -- the single largest undeclared-function cluster seen in this
series), `LREAL_TO_FMTSTR` (4 files), `ADR` (1 file). All three are real,
documented Beckhoff/CODESYS built-ins, not typos or project-local
helpers.

## Verification against real documentation and the corpus

Checked Beckhoff's own docs for each before designing anything (not
assumed from memory):

- **`BOOL_TO_STRING`**: part of the *standard* `TO_STRING` conversion
  family (same category as `INT_TO_STRING`/`REAL_TO_STRING`, already
  implemented) -- not a separate library function. `TRUE` converts to
  `'TRUE'`, `FALSE` to `'FALSE'`.
- **`LREAL_TO_FMTSTR`**: `TcUtilities.Lib` (Beckhoff `Tc2_Utilities`
  library), a genuine vendor extension. Signature confirmed directly:
  `LREAL_TO_FMTSTR(in : LREAL, iPrecision : INT, bRound : BOOL) : STRING(510)`.
  `iPrecision` controls decimal places; `bRound` enables rounding;
  documented special cases (`'#INF'`, `'#QNAN'`, `'#OVF'`) are a runtime
  concern, not a parsing/registration one.
- **`ADR`**: documented as the "Address Operator" (Beckhoff's own term),
  a core TcPlcCtrl operator, not a library function. Returns the
  argument's address as `PVOID`, or `DWORD`/`LWORD` depending on runtime
  architecture; Beckhoff recommends `PVOID` for portability, but IronPLC
  has no `PVOID` type today -- using `DWORD` (the well-documented 32-bit
  fallback) instead, noting the `PVOID` gap as a known simplification.
- Corpus usage confirmed directly: `ADR(sContent)`, `ADR(stIn_EL6001)`
  (single positional argument, any type); `LREAL_TO_FMTSTR(tempM1, 2,
  TRUE)` (matches the documented 3-argument signature exactly);
  `BOOL_TO_STRING(ready)`, `BOOL_TO_STRING(error)` (single BOOL argument).
- Confirmed `ADR`/`LREAL_TO_FMTSTR` are true syntactic function calls
  (`NAME(args)`), so registering them in `FunctionEnvironment` is
  sufficient for `ironplcc check` -- no new grammar needed, matching the
  precedent already established for `SIZEOF` (also styled an "operator"
  in Beckhoff's prose, but modeled as an ordinary stdlib function with an
  `ANY`-typed parameter).

## Design

### `BOOL_TO_STRING`: unconditional core stdlib addition, but codegen needs a real fix, not a no-op

Add `functions.push(build_conversion_function("BOOL", "STRING"));` to the
*existing*, unconditional `get_string_conversion_functions()` -- same
list `REAL_TO_STRING` already lives in, no new flag (matches how the
standard `TO_STRING` family has never been flag-gated).

**Codegen hazard found while designing (not just assumed a stub would
be needed)**: `compile_call.rs`'s generic `*_TO_STRING` dispatcher
(`parse_string_conversion`/`compile_string_conversion`) resolves `BOOL`
to the *same* `VarTypeInfo` shape as any other signed-32-bit source
(`resolve_type_name` maps `BOOL` to `(OpWidth::W32, Signedness::Signed)`,
identical to `DINT`). Left as-is, `BOOL_TO_STRING` would silently compile
via `CONV_I32_TO_STR`, producing `"1"`/`"0"` (or similar) instead of the
documented `"TRUE"`/`"FALSE"` -- **wrong output that compiles cleanly**,
strictly worse than a clean rejection. Fix: `parse_string_conversion`
special-cases a `BOOL` source into a distinct `StringConversion::BoolToString`
variant (checked by name, before generic numeric resolution), and
`compile_string_conversion` returns `Diagnostic::not_implemented` for it
-- real `"TRUE"`/`"FALSE"` codegen (constant-pool string literals +
conditional branch) is a genuine implementation task, out of scope for
"stop `ironplcc check` reporting undeclared function."

### `LREAL_TO_FMTSTR` and `ADR`: new stdlib registrations behind a new flag each

```rust
// LREAL_TO_FMTSTR — Beckhoff Tc2_Utilities (TcUtilities.Lib)
FunctionSignature::stdlib(
    "LREAL_TO_FMTSTR",
    TypeName::from("STRING"),
    vec![
        input_param("IN", "LREAL"),
        input_param("IPRECISION", "INT"),
        input_param("BROUND", "BOOL"),
    ],
)
```

```rust
// ADR — Beckhoff Address Operator (TcPlcCtrl core)
FunctionSignature::stdlib(
    "ADR",
    TypeName::from("DWORD"),
    vec![input_param("IN", "ANY")],
)
```

Two new flags (not reusing `allow_extended_math_functions`, which is
specifically the Beckhoff `Tc2_Math` library -- these are a different
library and a different core operator, and the project's own "dialect
placement" convention keeps unrelated vendor extensions on separate
flags):

- `allow_extended_string_functions` -- gates `LREAL_TO_FMTSTR` (room to
  add other `TcUtilities.Lib` string functions later without renaming).
- `allow_address_operator` -- gates `ADR`, matching `allow_sizeof`'s
  precedent (one flag per core vendor operator, not a library).

Both registered in `stages.rs` conditionally, exactly like
`allow_sizeof`/`allow_extended_math_functions` already are.

### Codegen: `not_implemented` for both, not silently wrong or missing

Neither has a sensible generic fallback the way `BOOL_TO_STRING` almost
did:

- `ADR` needs the argument's real runtime memory/data-region address --
  a genuine VM feature (this VM's variable storage model isn't yet
  investigated deeply enough to implement this correctly; a wrong
  address is far worse than a clean rejection).
- `LREAL_TO_FMTSTR` needs real floating-point formatting with rounding
  and the documented special cases (`#INF`/`#QNAN`/`#OVF`) -- a
  non-trivial runtime feature, not a simple opcode dispatch.

Both `compile_call.rs` dispatch arms (matched by lowercased function
name, same pattern as `"sizeof" => compile_sizeof(...)`) return
`Diagnostic::not_implemented` -- `ironplcc check` fully supports all
three functions (the actual motivating gap); `ironplcc compile` fails
clearly instead of producing wrong or silently-missing bytecode.

## Non-goals

- `PVOID` type modeling for `ADR`'s Beckhoff-recommended portable return
  type -- `DWORD` used instead, a known simplification.
- Real codegen for any of the three (address computation, formatted
  float-to-string with rounding, or `TRUE`/`FALSE` string codegen) --
  explicitly deferred with `not_implemented`, matching the `AND_THEN`/
  struct-init-expression precedent.
- `STRING_TO_BOOL` (the reverse direction) -- not reported as missing in
  the corpus survey; not added per "don't add capability beyond what's
  verified needed."
- Other `TcUtilities.Lib` functions beyond `LREAL_TO_FMTSTR` -- not
  reported as missing; the new flag leaves room to add them later if
  found.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | New `allow_extended_string_functions`, `allow_address_operator` flags |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | `BOOL_TO_STRING` added to `get_string_conversion_functions()`; new `get_extended_string_functions()` (`LREAL_TO_FMTSTR`); new `get_address_operator_function()` (`ADR`) |
| `compiler/analyzer/src/stages.rs` | Conditionally register the two new flag-gated functions |
| `compiler/codegen/src/compile_call.rs` | `StringConversion::BoolToString` variant + `not_implemented`; `not_implemented` dispatch arms for `adr`/`lreal_to_fmtstr` |

## Testing Strategy

- Stdlib registration tests: `BOOL_TO_STRING` present unconditionally;
  `LREAL_TO_FMTSTR`/`ADR` present only when their flag is set, absent
  otherwise.
- Parser/semantic test: the real motivating call shapes (`BOOL_TO_STRING(ready)`,
  `LREAL_TO_FMTSTR(tempM1, 2, TRUE)`, `ADR(sContent)`) no longer report
  `P4017` (undeclared function).
- Codegen tests: compiling a call to each of the three produces
  `Diagnostic::not_implemented` (`P9999`), not a panic or silently wrong
  bytecode -- especially `BOOL_TO_STRING`, to lock in the
  found-not-assumed codegen hazard above.
- End-to-end: verify via the CLI that `ironplcc check` accepts all three
  real motivating call shapes under `--dialect=codesys`.

## Tasks

- [x] Write plan (this document)
- [x] Verify real signatures against Beckhoff docs and the private corpus
- [x] `BOOL_TO_STRING`: add to `get_string_conversion_functions()`
- [x] `BOOL_TO_STRING`: fix the codegen hazard (`StringConversion::BoolToString` -> `not_implemented`)
- [x] New flags: `allow_extended_string_functions`, `allow_address_operator`
- [x] `LREAL_TO_FMTSTR`, `ADR` stdlib registrations + `stages.rs` wiring
- [x] Codegen `not_implemented` arms for `adr`/`lreal_to_fmtstr`
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork

## Implementation Notes

- **This branch (`twincat-dev`) predates upstream PR #1227's invariant-
  style options tests** -- it still uses the older hardcoded-count style
  (`from_dialect_when_rusty_then_all_vendor_flags_enabled_and_edition3_disabled`
  etc. asserting each flag individually, plus
  `feature_descriptors_when_called_then_contains_all_vendor_flags` and two
  siblings asserting raw `FEATURE_DESCRIPTORS.len()`/per-dialect counts).
  Adding two new flags required bumping four magic numbers across
  `options.rs` (22->24 total/rusty, 21->23 codesys) plus
  `compiler/mcp/src/tools/list_options.rs`'s own hardcoded count (22->24)
  -- caught entirely by `cargo test --workspace` failures, not
  anticipated in the original plan text.
- **The `BoolToString` hazard fix works by intercepting the name inside
  `parse_string_conversion` before generic `resolve_type_name`**: `BOOL`
  and `DINT` resolve to the identical `VarTypeInfo` shape
  `(OpWidth::W32, Signedness::Signed)`, so there's no way to distinguish
  them *after* that resolution -- the check has to happen on the raw
  `"BOOL_TO_STRING"` name string, before it's thrown away.
- Confirmed via direct CLI runs (not just unit tests) that all three
  functions: (a) resolve cleanly under `ironplcc check` when their flag
  is set (`BOOL_TO_STRING` needs no flag at all), (b) `LREAL_TO_FMTSTR`/
  `ADR` correctly still report `P4017` under the default dialect, and
  (c) all three fail cleanly with `P9999` under `ironplcc compile`,
  never a panic or silently wrong bytecode.
