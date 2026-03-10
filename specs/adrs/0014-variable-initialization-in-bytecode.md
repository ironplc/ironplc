# Variable Initialization via Separate Init Function

status: accepted
date: 2026-03-05

## Context and Problem Statement

IEC 61131-3 variable declarations can include initial values:

```
VAR
  x : INT := 10;
  y : INT := 32;
END_VAR
```

The VM starts all variable slots at zero. The compiler must generate initialization logic so that variables have their declared values when the program first executes.

How should the compiler and VM handle variable initial values, considering both correctness and future hot-reload requirements?

## Decision Drivers

* **Correctness** -- variables must hold their declared initial values before the first statement executes
* **Hot-reload compatibility** -- loading a new `.iplc` file into a running VM should not reset variables that retain their runtime state
* **Simplicity** -- the solution should minimize changes to the container format and VM
* **IEC 61131-3 semantics** -- initial values are applied once at program startup, not on every scan cycle

## Considered Options

* Inline bytecode in the program function
* Separate init function in the container
* Initial values as container metadata applied by the VM

## Decision Outcome

Chosen option: "Separate init function in the container", because it cleanly separates initialization from scan execution, matches IEC 61131-3 cold-start semantics, and supports future hot-reload without losing runtime variable state.

The compiler always emits two functions:
- **Function 0**: init function (load constants + store variables; just `RET_VOID` when no initial values)
- **Function 1**: scan function (program body)

The `ProgramInstanceEntry.init_function_id` field (previously `reserved`) identifies the init function. The VM unconditionally calls init functions once during `start()`, then `run_round()` only calls scan functions.

### Consequences

* Good, because initialization runs once at startup, matching IEC 61131-3 cold-start semantics
* Good, because hot-reload can skip init functions, preserving runtime variable state
* Good, because the container format change is minimal (repurposing an existing reserved field)
* Good, because the scan function contains only program body logic, improving clarity
* Good, because the VM can distinguish "first scan" from "subsequent scans"
* Neutral, because two functions instead of one adds modest codegen complexity

## Pros and Cons of the Options

### Inline Bytecode in the Program Function

Emit `LOAD_CONST` + `TRUNC` (if narrow) + `STORE_VAR` instructions at the top of the program function, before the body.

* Good, because zero changes to container format and VM
* Good, because reuses existing codegen (`compile_constant`, `emit_truncation`, `emit_store_var`)
* Good, because correct for cold-start (the only current execution mode)
* Neutral, because re-initialization on every scan is idempotent for constant initial values but incorrect for accumulating variables during hot-reload
* Bad, because does not support hot-reload without losing variable state
* Bad, because there is no clean separation between "initialize once" and "execute every scan"

### Separate Init Function in the Container

Add a second function to the container (alongside the scan function) that contains only initialization bytecode. The VM calls it once on startup.

* Good, because cleanly separates "init once" from "execute every scan"
* Good, because the VM can skip the init function during hot-reload, preserving variable state
* Good, because the init function can also handle future features (FB instance initialization, array initialization)
* Good, because uses the existing reserved field in ProgramInstanceEntry (no binary layout change)
* Neutral, because requires VM changes (call init function on startup)
* Neutral, because more codegen complexity (two functions instead of one)

### Initial Values as Container Metadata

Store initial values in a data table in the container. The VM reads the table and sets variable slots before calling the scan function.

* Good, because initialization is entirely in the VM's control -- it decides when to apply initial values
* Good, because the data table is compact and efficient (no bytecode dispatch overhead)
* Good, because the VM can implement sophisticated hot-reload policies (e.g., apply initial values only for newly added variables)
* Bad, because requires a new container section and format changes
* Bad, because requires new VM logic to interpret the data table
* Bad, because the data table format must handle all types (integers, floats, strings, structs, arrays), duplicating type knowledge between codegen and VM
* Bad, because loses the ability to use computed initial values (expressions as initializers) which IEC 61131-3 allows in some contexts

## More Information

### IEC 61131-3 Initialization Semantics

The standard distinguishes between warm restart (retain variable values) and cold restart (apply initial values). The current implementation always performs cold-restart semantics. The separate init function approach naturally maps to this: cold restart calls the init function, warm restart skips it.

### Container Format

The `init_function_id` field occupies bytes 14-15 of `ProgramInstanceEntry`, which were previously reserved.

### Function ID Convention

Every compiled program uses:
- `init_function_id = 0` (function 0 = init)
- `entry_function_id = 1` (function 1 = scan)
