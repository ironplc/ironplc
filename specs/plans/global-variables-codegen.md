# Global Variables Code Generation

## Context

IronPLC previously parsed and analyzed `VAR_GLOBAL` and `VAR_EXTERNAL`
declarations but did not generate code for them. The codegen ignored
`ConfigurationDeclaration` entirely.

## Key Findings

- **Parser gap**: `located_var_spec_init()` only accepted `simple_spec_init()`, not
  arrays. Similarly, `external_declaration_spec()` only accepted `simple_specification()`.
  Both rules were extended to support `array_spec_init()` / `array_specification()`.
- **No analyzer changes needed**: `VAR_EXTERNAL` creates `Simple(type_name, None)`,
  which `ExprTypeResolver.insert()` already handles. Expression type resolution works
  for both simple and array externals without modification.

## Design

### Variable Table Layout

```
Index:  0 .. G-1     G .. N-1           N .. M-1
        ─────────    ────────────────   ─────────────
        global vars  program locals     function vars
                     (VAR_EXTERNAL skipped — aliases globals)
```

Global variables are assigned first. `VAR_EXTERNAL` declarations in the program are
not assigned new slots; the program body references the global's index by name.
For arrays, the global's `ArrayVarInfo` (data offset, descriptor, dimensions) is
found when the program body compiles array accesses — no special aliasing needed.

### Initialization

Global variables are initialized in the init function (function 0) using their
initial values from the `ConfigurationDeclaration`. Locals are initialized after.

### Scope

- Configuration-level `VAR_GLOBAL` only (not resource-level).
- All currently supported types including arrays.
- `VAR_GLOBAL CONSTANT` works (constness enforced by the analyzer).

## Changes

### Parser (`compiler/parser/src/parser.rs`)

- `located_var_spec_init`: Added `array_spec_init()` alternative before `simple_spec_init()`.
- `external_declaration_spec`: Added `array_specification()` alternative before `simple_specification()`.

### Codegen (`compiler/codegen/src/compile.rs`)

- Added `find_configuration()` helper to extract optional `ConfigurationDeclaration`.
- `compile_reachable`: Extracts `global_var` from configuration (or empty slice).
- `compile_program_with_functions`: New `global_vars` parameter.
  1. Assigns global variables first (indices 0..G).
  2. Filters out `VAR_EXTERNAL` from program variables (they alias globals).
  3. Assigns program-local variables (indices G..N).
  4. Initializes globals then locals in the init function.

### Tests (`compiler/codegen/tests/end_to_end_global.rs`)

8 end-to-end tests covering: initial values, writes via external, default zero,
multiple globals, constants, array read/write, and no-configuration regression.

### Documentation (`docs/reference/language/variables/scope.rst`)

- `VAR_GLOBAL` and `VAR_EXTERNAL` status changed to "Supported".
- Added Global Variables section with interactive playground example
  showing both primitive and array globals.
