# Design: VM Runtime Error Codes (V-prefix)

## Summary

Add unique, documented error codes to the VM runtime. Each error gets a V-code
(e.g., V4001) printed to stderr for documentation lookup, and a category-level
exit code (1, 2, or 3) for scripting. This mirrors the compiler's P-prefix and
editor's E-prefix error code systems.

## Motivation

The compiler and editor already give precise, documented error codes for every
user-facing error. The VM runtime does not — all errors exit with code 1 and
have no structured codes. Users cannot look up specific errors in documentation,
and scripts cannot distinguish error categories.

## Design

See [ADR-0014](../adrs/0014-vm-error-code-categories.md) for the decision
rationale on category ranges and exit code strategy.

### Two identifiers per error

Each error has two identifiers:

1. **V-code** (e.g., V4001) — the specific error, printed to stderr, used for
   documentation lookup. Uses the compiler's thousands-digit category scheme.
   Unlimited growth.
2. **Exit code** (1, 2, or 3) — the error *category*, used by scripts to branch.
   Fits in u8.

### Category mapping

The VM uses the same thousands-digit category scheme as the compiler, so the
same first digit carries the same meaning across all IronPLC components.

| V-Code Range | Exit Code | Category | Compiler parallel |
|-------------|-----------|----------|------------------|
| V4001–V4999 | 1 | Runtime execution errors (user's program) | P4xxx (semantic) |
| V6001–V6999 | 2 | File system / IO errors | P6xxx (file system) |
| V9001–V9999 | 3 | Internal VM errors (compiler/VM bugs) | P9xxx (internal) |

### Initial V-code assignments

#### V4xxx — Runtime execution errors (exit code 1)

Errors caused by the user's program logic.

| V-Code | Trap Variant | Message |
|--------|-------------|---------|
| V4001 | DivideByZero | Divide by zero |
| V4002 | NegativeExponent | Negative exponent |
| V4003 | WatchdogTimeout | Watchdog timeout |

#### V6xxx — File system / IO errors (exit code 2)

Errors reading or writing files before or after execution.

| V-Code | Condition | Message |
|--------|-----------|---------|
| V6001 | File open failed | Unable to open file |
| V6002 | Container read failed | Unable to read container |
| V6003 | Signal handler failed | Failed to set signal handler |
| V6004 | Dump file creation failed | Unable to create dump file |
| V6005 | Variable read failed | Unable to read variable |
| V6006 | Dump file write failed | Unable to write dump file |
| V6007 | Log config failed | Unable to configure logger |

#### V9xxx — Internal VM errors (exit code 3)

Errors that indicate a VM invariant was violated.

| V-Code | Trap Variant | Message |
|--------|-------------|---------|
| V9001 | StackOverflow | Stack overflow |
| V9002 | StackUnderflow | Stack underflow |
| V9003 | InvalidInstruction | Invalid instruction |
| V9004 | InvalidConstantIndex | Invalid constant index |
| V9005 | InvalidVariableIndex | Invalid variable index |
| V9006 | InvalidFunctionId | Invalid function ID |
| V9007 | InvalidBuiltinFunction | Invalid built-in function |

**Total: 17 codes** (3 execution + 7 IO + 7 internal).

### CLI output format

```
Error: V4001 - VM trap: divide by zero (task 0, instance 0)
Error: V6001 - Unable to open /path/to/file.iplc: No such file or directory
Error: V9001 - VM trap: stack overflow (task 0, instance 0)
```

Format: `{v_code} - {message}`. Mirrors the editor's `formatProblem` pattern.

## Implementation Plan

### Step 1: Add `exit_code()` and `v_code()` to Trap enum

**File:** `compiler/vm/src/error.rs`

Add two methods to `Trap`:

```rust
impl Trap {
    /// Returns the V-code string for this trap (e.g., "V4001").
    pub fn v_code(&self) -> &'static str { ... }

    /// Returns the category exit code for this trap (1 = execution, 3 = internal).
    pub fn exit_code(&self) -> u8 { ... }
}
```

Classify traps as execution errors (V4xxx, exit 1) or internal errors (V9xxx,
exit 3) based on whether the user's program or the VM/compiler is at fault.

Add unit tests for each variant (BDD naming).

### Step 2: Create `VmError` type in CLI crate

**File:** `compiler/vm-cli/src/error.rs` (new module)

Define a `VmError` struct:

```rust
pub struct VmError {
    pub v_code: &'static str,
    pub exit_code: u8,
    message: String,
}
```

With constructors:
- `VmError::from_trap(trap, task_id, instance_id)` — uses `trap.v_code()` and
  `trap.exit_code()`
- `VmError::io(v_code, message)` — for CLI/IO errors (exit code 2)

And constants for CLI error codes:

```rust
pub const FILE_OPEN: &str = "V6001";
pub const CONTAINER_READ: &str = "V6002";
pub const SIGNAL_HANDLER: &str = "V6003";
pub const DUMP_CREATE: &str = "V6004";
pub const VAR_READ: &str = "V6005";
pub const DUMP_WRITE: &str = "V6006";
pub const LOG_CONFIG: &str = "V6007";
pub const IO_EXIT_CODE: u8 = 2;
```

`Display` impl formats as `{v_code} - {message}`.

### Step 3: Update CLI to use VmError

**File:** `compiler/vm-cli/src/cli.rs`

Change `run()` and `benchmark()` return types from `Result<(), String>` to
`Result<(), VmError>`. Update every `.map_err(...)` call.

**File:** `compiler/vm-cli/src/logger.rs`

Change `configure()` return type from `Result<(), String>` to
`Result<(), VmError>`.

**File:** `compiler/vm-cli/src/main.rs`

Change `main()` from `Result<(), String>` to `ExitCode`:

```rust
pub fn main() -> ExitCode {
    let args = Args::parse();
    let result = logger::configure(args.verbose, args.log_file)
        .and_then(|()| match args.action { ... });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(e.exit_code)
        }
    }
}
```

### Step 4: Update CLI integration tests

**File:** `compiler/vm-cli/tests/cli.rs`

Update existing tests to assert specific exit codes and V-codes in stderr:
- `run_when_file_not_found_then_exit_2_and_v6001`
- `run_when_invalid_container_then_exit_2_and_v6002`

Add a new trap test:
- `run_when_divide_by_zero_trap_then_exit_1_and_v4001`

### Step 5: Create problem codes CSV

**File:** `compiler/vm-cli/resources/problem-codes.csv` (new)

```csv
Code,Name,Message
V4001,DivideByZero,Divide by zero during program execution
V4002,NegativeExponent,Negative exponent in arithmetic operation
V4003,WatchdogTimeout,Task exceeded its watchdog time limit
V6001,FileOpenFailed,Unable to open the specified file
V6002,ContainerReadFailed,Unable to read bytecode container
V6003,SignalHandlerFailed,Failed to install signal handler
V6004,DumpCreateFailed,Unable to create variable dump file
V6005,VariableReadFailed,Unable to read variable value
V6006,DumpWriteFailed,Unable to write to variable dump file
V6007,LogConfigFailed,Unable to configure the logger
V9001,StackOverflow,VM operand stack overflow
V9002,StackUnderflow,VM operand stack underflow
V9003,InvalidInstruction,Unknown bytecode instruction
V9004,InvalidConstantIndex,Constant pool index out of bounds
V9005,InvalidVariableIndex,Variable table index out of bounds
V9006,InvalidFunctionId,Function ID not found in container
V9007,InvalidBuiltinFunction,Built-in function ID not recognized
```

### Step 6: Create documentation pages

**Directory:** `docs/reference/runtime/problems/` (new)

Create 17 `.rst` files following the existing pattern:

```rst
=====
V4001
=====

.. problem-summary:: V4001

The program attempted to divide by zero at runtime. ...

Example
-------
...
```

- **V4xxx docs**: Explain the runtime condition and how to guard against it in
  Structured Text
- **V6xxx docs**: Explain the OS-level cause and how to fix it (permissions,
  paths, etc.)
- **V9xxx docs**: Explain these indicate a compiler or VM bug and should be
  reported

### Step 7: Update runtime docs index

**File:** `docs/reference/runtime/index.rst`

Add `Error Codes <problems/index>` to the toctree.

### Step 8: Update Sphinx extension

**File:** `docs/extensions/ironplc_problemcode.py`

1. Add VM help topics:
   `runtime_help_topics = set([...listdir('reference/runtime/problems')])`
2. Merge into `help_topics`
3. Add VM CSV to `definitions` list:
   `join('..', 'compiler', 'vm-cli', 'resources', 'problem-codes.csv')`
4. Add runtime index generation in `generate_problem_index` — glob `V*.rst`
   from `reference/runtime/problems/`
5. Add redirect entry: `('runtime/problems', 'reference/runtime/problems')`

### Step 9: Update steering docs

**File:** `specs/steering/problem-code-management.md`

Add a "VM Runtime Error Codes (V-prefix)" section:
- Registry location: `compiler/vm-cli/resources/problem-codes.csv`
- Documentation location: `docs/reference/runtime/problems/V####.rst`
- Category ranges: V4xxx (execution), V6xxx (file system), V9xxx (internal)

## Verification

1. `cd compiler && just` — must pass (compile, coverage >= 85%, clippy, fmt)
2. `ironplcvm run nonexistent.iplc` exits with code 2 and stderr contains
   "V6001"
3. A divide-by-zero container exits with code 1 and stderr contains "V4001"
4. Docs build validates all V-codes have `.rst` files
