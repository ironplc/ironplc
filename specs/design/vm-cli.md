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

Loads a bytecode container file and executes it.

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
| `--dump-vars [PATH]` | After the VM stops, write all variable values to `PATH`. If `PATH` is omitted or `-`, write to stdout. |
| `--scans <N>` | Run exactly `N` scheduling rounds then stop. When omitted, runs continuously until SIGINT (Ctrl+C). |
| `--group-by-scope` | Group the `--dump-vars` output by owning POU (frame) and annotate each variable with its IEC section. Falls back to the flat format when the container has no debug section. |

**Behavior:**

- **REQ-VC-001** `run` opens the container file at `<FILE>`. If the file cannot be opened, the command exits with code 2 and emits V6001 to stderr.
- **REQ-VC-002** `run` decodes the container. If the bytes are not a valid container (bad magic, truncated, unsupported version), the command exits with code 2 and emits V6002 to stderr.
- **REQ-VC-003** `run --scans N` executes exactly `N` scheduling rounds then exits 0.
- **REQ-VC-004** When execution traps (divide by zero, stack overflow, invalid instruction, etc.), `run` exits with code 1 and emits the trap's V-code to stderr.
- **REQ-VC-011** When no `--scans` value is given, `run` loops until SIGINT (Ctrl+C). On SIGINT it requests a clean stop and exits 0 after the current round.
- **REQ-VC-012** Between rounds, `run` sleeps until the next cyclic task is due (based on `next_due_us`) to avoid busy-looping.

#### `benchmark`

Measures execution timing by running a container for a fixed number of rounds.

```
ironplcvm benchmark [OPTIONS] <FILE>
```

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--cycles <N>` | 10000 | Number of measured scan rounds. |
| `--warmup <M>` | 1000 | Number of unmeasured warmup rounds before measurement. |

**Behavior:**

- **REQ-VC-013** `benchmark` prints a single JSON object to stdout containing `program`, `opt_level`, `cycles`, `warmup`, a `scan_us` object with `mean`, `stddev`, `p99`, and `max` in microseconds, and a `tasks` array with per-task metadata.
- **REQ-VC-014** `benchmark --cycles N --warmup M` executes `M` unmeasured warmup rounds followed by `N` measured rounds.
- **REQ-VC-015** For each cyclic task with `interval_us > 0`, the JSON `tasks[*]` entry includes a `budget_pct` object with `mean`, `p99`, and `max` expressed as a percentage of the task's interval.
- **REQ-VC-016** File-open (V6001) and container-read (V6002) errors behave identically to `run`: exit code 2 with the V-code on stderr.
- **REQ-VC-017** If a trap occurs during either the warmup or measured phase, `benchmark` exits with code 1 and emits the trap's V-code to stderr.

#### `version`

Prints the version string.

```
ironplcvm version
```

Output: `ironplcvm version <VERSION>` followed by a newline on stdout.

## Variable Dump Format

The `--dump-vars [PATH]` option writes all variable slot values after the VM stops (successfully or from a fault).

### Behavior

- **REQ-VC-005** After a successful run, `--dump-vars <PATH>` writes one variable per line, newline-terminated.
- **REQ-VC-006** If `--dump-vars` is specified without a `PATH`, or with `PATH` equal to `-`, the dump is written to stdout.
- **REQ-VC-007** If a runtime trap occurs and `--dump-vars` is set, the dump of the current variable state is written before the command exits non-zero.
- **REQ-VC-008** When the container's debug section names a variable, the line uses `<name>: <value>`. Otherwise the line uses `var[<index>]: <raw_i32>`.
- **REQ-VC-009** When debug info provides an IEC type tag, `<value>` is formatted per the type:

| Tag | Format | Example |
|-----|--------|---------|
| `BOOL` | `TRUE`/`FALSE` | `TRUE` |
| `SINT`, `INT`, `DINT`, `LINT` | signed decimal | `-42` |
| `USINT`, `UINT`, `UDINT`, `ULINT` | unsigned decimal | `42` |
| `REAL` | `{}` (32-bit float) | `3.14` |
| `LREAL` | `{}` (64-bit float) | `3.14159265` |
| `BYTE` | `16#XX` | `16#FF` |
| `WORD` | `16#XXXX` | `16#ABCD` |
| `DWORD` | `16#XXXXXXXX` | `16#DEADBEEF` |
| `LWORD` | `16#XXXXXXXXXXXXXXXX` | `16#00000000DEADBEEF` |
| `TIME` | `T#<ms>ms` | `T#250ms` |
| `LTIME` | `LTIME#<ms>ms` | `LTIME#250ms` |
| other | signed decimal fallback | `0` |

- **REQ-VC-010** If the dump file cannot be created (e.g., parent directory missing), the command exits with code 2 and emits V6004 to stderr.

### Format

One line per variable, zero-indexed, newline-terminated (no debug info):

```
var[0]: 10
var[1]: 42
```

With debug info:

```
Buzzer: TRUE
Counter: 42
```

### Rules

1. **Variable count** comes from `container.header.num_variables`.
2. **Variable order** is ascending by index, 0 through `num_variables - 1`.
3. **File creation**: the dump file is created or overwritten (not appended).
4. **Empty programs**: if `num_variables` is 0, the dump file is created but empty.

### Example

For a program `x := 10; y := x + 32;` with two variables:

```
var[0]: 10
var[1]: 42
```

## Scoped Variable Dump Format

The `--group-by-scope` option is a debug-info-aware alternative layout for
`--dump-vars`. It groups variables by the POU that declares them — using the
`function_id` and `var_section` metadata in the debug section's VAR_NAME
table — so the output mirrors the call structure a debugger would show.

### Behavior

- **REQ-VC-018** With `--group-by-scope`, variables are emitted in groups, each introduced by a `[<label>]` header line followed by one indented line per variable; program/global variables form a `[Globals]` group that is emitted first, before any per-POU group.
- **REQ-VC-019** Within `--group-by-scope`, each non-global variable line is annotated with its IEC section name (`VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`, `VAR_TEMP`, `VAR_EXTERNAL`, `VAR_GLOBAL`); variables in the `[Globals]` group omit the annotation.
- **REQ-VC-020** If the container has no debug section (or it names no variables), `--group-by-scope` falls back to the flat dump format (REQ-VC-005/008/009).

### Format

For a program with a global `counter : DINT`, a function `add_offset`
(input `n`, local `bump`, return value), and a function block `accumulator`
(input `step`, output `total`):

```
[Globals]
  counter : DINT = 3
[add_offset]
  n : DINT = 3  (VAR_INPUT)
  bump : DINT = 11  (VAR)
  add_offset : DINT = 14  (VAR_OUTPUT)
[accumulator]
  step : DINT = 3  (VAR_INPUT)
  total : DINT = 6  (VAR_OUTPUT)
```

The function's return value appears as a `VAR_OUTPUT`-annotated line named
after the function (matching the debug section's encoding).

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — program loaded and execution completed without traps. |
| 1 | Runtime trap — a VM trap occurred during execution. |
| 2 | IO or container error — file could not be opened, read, created, or written. |

## Future Extensions

- Type-aware variable dump for richer IEC types (arrays, structs) once the container type section covers them.
