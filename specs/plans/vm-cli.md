# Spec: VM CLI and Variable Dump

## Overview

This spec defines the `ironplcvm` command-line executable and the variable dump output format. The executable loads a bytecode container file and runs it on the VM. The variable dump provides observable output to verify the VM executed correctly.

This spec builds on:

- **[Bytecode Container Format](bytecode-container-format.md)**: The container file that `ironplcvm` reads
- **[Runtime Execution Model](runtime-execution-model.md)**: The VM lifecycle and scan cycle that `ironplcvm` drives
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: The instructions the VM executes

## Design Goals

1. **Observable execution** — the variable dump provides proof that the VM ran and computed correct results, without polluting stdout
2. **Silent success** — a successful run produces no terminal output; the exit code signals success or failure
3. **Consistent CLI patterns** — follows the same argument structure and conventions as `ironplcc`

## CLI Interface

### Binary Name

`ironplcvm`

### Global Options

| Option | Short | Description |
|--------|-------|-------------|
| `--verbose` | `-v` | Increase logging verbosity. Repeatable up to 4 times (error → warn → info → debug → trace). |
| `--log-file <path>` | `-l` | Write log output to a file instead of stderr. |

### Subcommands

#### `run`

Loads a bytecode container file and executes one scan cycle.

```
ironplcvm run [OPTIONS] <FILE>
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to the bytecode container file (`.iplc`). |

**Options:**

| Option | Description |
|--------|-------------|
| `--dump-vars <PATH>` | After a successful scan, write all variable values to the specified file. |

**Behavior:**

1. Open and read the container file.
2. Load the container into the VM (`Vm::new().load(container)`).
3. Start the VM (`vm.start()`).
4. Execute one scan cycle (`vm.run_single_scan()`).
5. If `--dump-vars` is specified, write the variable dump file (see format below).
6. Exit with code 0 on success.

**Error behavior:**

- If the file cannot be opened or read: print error to stderr, exit with non-zero code.
- If the file is not a valid container (bad magic, unsupported version, etc.): print error to stderr, exit with non-zero code.
- If execution traps (divide by zero, stack overflow, invalid instruction, etc.): print the trap description to stderr, exit with non-zero code.

#### `version`

Prints the version string.

```
ironplcvm version
```

Output: `ironplcvm version <VERSION>` followed by a newline on stdout.

## Variable Dump Format

The `--dump-vars <path>` option writes all variable slot values to a file after a successful scan cycle.

### Format

One line per variable, zero-indexed, newline-terminated:

```
var[0]: 10
var[1]: 42
```

### Grammar

```
dump       = { variable } ;
variable   = "var[" , index , "]: " , value , "\n" ;
index      = decimal_integer ;    (* 0-based, no leading zeros except for 0 itself *)
value      = signed_decimal_i32 ; (* e.g., "42", "-1", "0" *)
```

### Rules

1. **Variable count** comes from `container.header.num_variables`.
2. **Variable order** is ascending by index, 0 through `num_variables - 1`.
3. **Value representation**: each variable is printed as a signed 32-bit integer (via `Slot::as_i32()`). When the container type section is implemented, the dump format can become type-aware.
4. **File creation**: the dump file is created or overwritten (not appended). If the file cannot be written, the command returns a non-zero exit code with an error on stderr.
5. **Empty programs**: if `num_variables` is 0, the dump file is created but empty.

### Example

For a program `x := 10; y := x + 32;` with two variables:

```
var[0]: 10
var[1]: 42
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — program loaded and scan completed without traps. |
| Non-zero | Failure — file error, container error, or runtime trap. |

## Future Extensions

- `--scans <N>` — run N scan cycles instead of one.
- Type-aware variable dump once the container type section is populated.
- `--continuous` — run scan cycles in a loop until interrupted.
