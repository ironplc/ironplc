# Global Variables Code Generation Plan

Adds code generation support for `VAR_GLOBAL` (declared in `CONFIGURATION`) and `VAR_EXTERNAL` (referenced in `PROGRAM`). Parsing and semantic analysis already exist; this is purely a codegen change plus documentation updates.

## Design

### Variable Table Layout

```
Index:  0 .. G-1     G .. N-1           N .. M-1
        ─────────    ────────────────   ─────────────
        global vars  program vars       function vars
                     (excl. EXTERNAL)
```

- Global variables occupy the first `G` slots (indices `0..G`).
- `VAR_EXTERNAL` declarations in the program are **not** assigned new slots. Instead, the compiler maps their names to the corresponding global variable's index.
- Program-local variables (`VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`) are assigned after globals.
- User-defined function variables follow after program variables (unchanged).

### Initialization

Global variables are initialized in the init function (function 0) using their initial values from the `ConfigurationDeclaration`. `VAR_EXTERNAL` variables do not need separate initialization since they alias the globals.

### Backward Compatibility

When no `ConfigurationDeclaration` exists (standalone `PROGRAM`), `G = 0` and behavior is identical to today. Programs with `VAR_EXTERNAL` but no configuration would fail at codegen (this is already caught by the analyzer).

### Scope

- Configuration-level `VAR_GLOBAL` only (not resource-level).
- All currently supported types (integer, float, bool, bit string, STRING).
- `VAR_GLOBAL CONSTANT` globals work automatically (the variable is read-only by convention; the analyzer already enforces that `VAR_EXTERNAL CONSTANT` matches).

## Implementation Steps

### Step 1: Extract global variables in `compile_reachable`

**File**: `compiler/codegen/src/compile.rs`

Add a `find_configuration` helper (analogous to `find_program`) that returns `Option<&ConfigurationDeclaration>`. It's optional because standalone programs without a configuration are valid.

Update `compile_reachable` to:
1. Call `find_configuration(library)` to get optional config.
2. Extract `&config.global_var` (or empty slice if no config).
3. Pass global vars to `compile_program_with_functions`.

### Step 2: Modify `compile_program_with_functions` to accept global vars

**File**: `compiler/codegen/src/compile.rs`

Change the signature to accept `global_vars: &[VarDecl]`.

Before assigning program variables:
1. Call `assign_variables(ctx, builder, global_vars, types)` to assign global vars at indices `0..G`.
2. Record the count of global variables (`global_var_count`).

### Step 3: Skip `VAR_EXTERNAL` during program variable assignment

**File**: `compiler/codegen/src/compile.rs`

In `assign_variables`, when processing program variables, skip declarations with `VariableType::External`. Instead, look up the variable name in `ctx.variables` (already populated by global var assignment) and verify it exists. If the name isn't found (no matching global), emit a diagnostic error.

The simplest approach: filter `VAR_EXTERNAL` declarations out of the program's variable list before passing to `assign_variables`, and handle the aliasing separately.

Concretely, in `compile_program_with_functions`:
```rust
// Assign global variables first (indices 0..G)
assign_variables(ctx, builder, global_vars, types)?;

// Split program vars: externals alias globals, others get new slots
let (externals, locals): (Vec<_>, Vec<_>) = program.variables
    .iter()
    .partition(|v| v.var_type == VariableType::External);

// For each VAR_EXTERNAL, verify its name exists in ctx.variables (from globals)
for ext in &externals {
    if let Some(id) = ext.identifier.symbolic_id() {
        if !ctx.variables.contains_key(id) {
            return Err(/* diagnostic: external variable has no matching global */);
        }
    }
}

// Assign program-local variables (indices G..N)
assign_variables(ctx, builder, &locals, types)?;
```

### Step 4: Initialize global variables

**File**: `compiler/codegen/src/compile.rs`

In `compile_program_with_functions`, emit initial values for global variables before program variables:

```rust
emit_initial_values(&mut init_emitter, &mut ctx, global_vars, types)?;
emit_initial_values(&mut init_emitter, &mut ctx, &locals, types)?;
```

(Skip `VAR_EXTERNAL` declarations since they're aliases.)

### Step 5: Add end-to-end tests

**File**: `compiler/codegen/tests/end_to_end_global.rs` (new file)

Tests following BDD naming convention:

1. `end_to_end_when_global_var_with_initial_value_then_external_reads_value` — Declares `VAR_GLOBAL x : INT := 42` in configuration, reads it via `VAR_EXTERNAL` in program, assigns to local. Verify local = 42.

2. `end_to_end_when_global_var_written_via_external_then_value_persists` — Writes to a global via `VAR_EXTERNAL`, then reads it back. Verify the value was written.

3. `end_to_end_when_global_var_no_initial_value_then_default_zero` — Global without initializer defaults to 0.

4. `end_to_end_when_multiple_globals_then_all_accessible` — Multiple `VAR_GLOBAL` variables, each accessed via `VAR_EXTERNAL`.

5. `end_to_end_when_global_constant_then_readable` — `VAR_GLOBAL CONSTANT` with initial value, read via `VAR_EXTERNAL CONSTANT`.

6. `end_to_end_when_no_configuration_then_program_still_works` — Standalone program without configuration (regression test, should pass unchanged).

### Step 6: Update documentation

**File**: `docs/reference/language/variables/scope.rst`

1. Change `VAR_GLOBAL` status from "Not yet supported" to "Supported".
2. Change `VAR_EXTERNAL` status from "Not yet supported" to "Supported".
3. Add a `VAR_GLOBAL` / `VAR_EXTERNAL` example using `.. playground::` directive (since it's a complete program with configuration).

Example:

```iec
CONFIGURATION config
  VAR_GLOBAL
    MaxSpeed : INT := 100;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
  VAR
    currentSpeed : INT;
  END_VAR
  currentSpeed := MaxSpeed;
END_PROGRAM
```

### Step 7: Update playground compile function

**File**: `compiler/playground/src/lib.rs`

Verify the playground's `compile()` function works with configurations. The playground calls `compile_reachable`, which we're modifying. Since configurations are optional, no change should be needed — but verify with a playground test that includes a configuration.

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| No ConfigurationDeclaration | G=0, works as before |
| VAR_EXTERNAL without matching global | Error diagnostic at codegen (also caught by analyzer) |
| Multiple VAR_EXTERNAL referencing same global | All map to same index — works naturally |
| VAR_GLOBAL CONSTANT | Assigned normally; constness enforced by analyzer |
| Resource-level VAR_GLOBAL | Not supported in this change; ignored |
| VAR_EXTERNAL in function blocks | Out of scope for initial implementation (FBs don't have codegen yet beyond built-in types) |

## Verification

1. All new tests pass: `cd compiler && just test`
2. Existing tests unchanged (no regressions)
3. Full CI pipeline: `cd compiler && just`
4. Coverage ≥ 85%
