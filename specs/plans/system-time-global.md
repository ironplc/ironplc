# Expose VM Uptime via Implicit `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` VAR_GLOBALs

## Context

IronPLC needs to let user code access the VM's monotonic uptime counter so that
dialect-specific functions like CODESYS's `TIME()` can be written as ordinary
ST functions. The IEC 61131-3 standard does **not** define any monotonic elapsed
timer function — `TIME()` returning ms-since-boot is a CODESYS vendor extension.

Rather than adding a dedicated VM opcode, we expose the uptime through the
existing global variable mechanism:

1. The compiler injects two implicit globals: `__SYSTEM_UP_TIME : TIME` and `__SYSTEM_UP_LTIME : LTIME`
2. The VM writes the uptime in milliseconds into both variables before each scan
3. Users access them via `VAR_EXTERNAL` in their own functions/FBs
4. A user-defined `FUNCTION TIME : TIME` wrapping `__SYSTEM_UP_TIME` provides the CODESYS-compatible API

This keeps the VM opcode-free for time access and lets users build arbitrary
time abstractions (TIME(), T_PLC_MS, etc.) in pure ST.

See **ADR-0030** for the tradeoff analysis behind the dual-variable design.

### Research Summary

| Source | Monotonic Timer | Standard? |
|--------|----------------|-----------|
| IEC 61131-3 Ed 2 | RTC FB (wall-clock only, must be initialized) | Yes |
| IEC 61131-3 Ed 3 | CONCAT/SPLIT/DAY_OF_WEEK (manipulation only) | Yes |
| CODESYS | `TIME()` operator (ms since boot, unsigned) | No — vendor extension |
| TwinCAT | `PlcTaskSystemInfo.DcTaskTime` (64-bit, 100ns) | No — vendor extension |
| Siemens | `SFC64 TIME_TCK` (signed i32 ms, wraps ~24.8d) | No — vendor extension |
| OSCAT | `T_PLC_MS()` wraps `TIME()` | Depends on vendor `TIME()` |

**Key finding:** No edition of IEC 61131-3 defines a monotonic elapsed timer
function. The `TIME()` operator is purely CODESYS-specific. The standard only
provides timer FBs (TON/TOF/TP with ET output) for measuring elapsed time
while a condition is true, and RTC for wall-clock tracking.

## Design

### System Variables

| Variable | Type | Storage | Wrap | Use case |
|----------|------|---------|------|----------|
| `__SYSTEM_UP_TIME` | `TIME` | i32 ms | ~24.8 days | Direct use with timer FBs (no conversion needed) |
| `__SYSTEM_UP_LTIME` | `LTIME` | i64 ms | ~292M years | Long-running applications, high-precision |

Both represent milliseconds since VM start. Updated once before each scan round.
All tasks in the same scan round observe the same values.

The `__SYSTEM_UP_TIME` variant wraps at ~24.8 days. Elapsed-duration subtraction
(`current - previous`) produces correct results as long as the interval is under
~24.8 days, which covers all practical timer FB use cases. For durations exceeding
days, use `__SYSTEM_UP_LTIME`.

### Variable Table Layout

```
Index:  0                1                  2 .. G+2    G+3 .. N      N+1 .. M
        ──────────────  ──────────────────  ─────────  ────────────  ─────────────
        __SYSTEM_       __SYSTEM_           user       program       function vars
        UP_TIME         UP_LTIME            globals    locals
        (i32, 1 slot)   (i64, 1 slot)
```

Both system globals are injected before user globals. `__SYSTEM_UP_TIME` at
`VarIndex(0)`, `__SYSTEM_UP_LTIME` at `VarIndex(1)`. User globals shift to
index 2+. Since `VAR_EXTERNAL` references are resolved by **name** (not index),
existing user code is unaffected.

### Container Flag

A single flag bit in `FileHeader.flags` tells the VM whether the container
expects the system uptime variables at indices 0 and 1:

```rust
pub const FLAG_HAS_SYSTEM_UPTIME: u8 = 0x01;
```

The VM checks this flag in `run_round()` and, if set, writes both uptime
values before executing tasks.

### Epoch and Monotonicity

- **Epoch**: Time since VM start, starting at 0
- **Monotonicity**: `current_time_us` passed to `run_round()` must be
  monotonically non-decreasing
- **Cold start**: Both variables start at 0
- **Warm start**: If the VM is stopped and restarted, the timer resets to 0

### Gating

New compiler option `allow_system_uptime_global`:
- Enabled by default for `--dialect rusty`
- Disabled for strict IEC dialects
- CLI flag: `--allow-system-uptime-global`

The double-underscore prefix convention (`__SYSTEM_UP_TIME`, `__SYSTEM_UP_LTIME`)
makes name conflicts extremely unlikely.

## Implementation Steps

### Step 1: Add compiler option

**File: `compiler/parser/src/options.rs`**

Add entry to `define_compiler_options!` macro:
```
"Expose __SYSTEM_UP_TIME and __SYSTEM_UP_LTIME as implicit VAR_GLOBALs (runtime monotonic uptime)",
"--allow-system-uptime-global",
[Rusty],
allow_system_uptime_global,
```

Update test assertions: `FEATURE_DESCRIPTORS.len()` and `rusty_features.len()`
each increment by 1.

**File: `compiler/plc2x/src/lsp.rs`** (~line 64)

Add `options.allow_system_uptime_global |= flag("allowSystemUptimeGlobal");`

### Step 2: Register implicit globals in analyzer

**File: `compiler/analyzer/src/stages.rs`** (~line 104)

Register both globals in the symbol environment when the option is enabled:

```rust
if options.allow_system_uptime_global {
    symbol_environment.insert(
        &Id::from("__SYSTEM_UP_TIME"),
        SymbolKind::Variable,
        &ScopeKind::Global,
    )?;
    symbol_environment.insert(
        &Id::from("__SYSTEM_UP_LTIME"),
        SymbolKind::Variable,
        &ScopeKind::Global,
    )?;
}
```

Add an analyzer rule: reject any user `VAR_GLOBAL` declaration of either
reserved name with a clear diagnostic when the option is enabled.

**Pre-implementation check:** Verify whether `VAR_EXTERNAL __SYSTEM_UP_TIME :
TIME` compiles without the global being in the symbol table. If it does,
the symbol environment registration may still be needed for completeness
but is not blocking.

### Step 3: Inject synthetic globals in codegen

**File: `compiler/codegen/src/compile.rs`** (~line 148)

Currently:
```rust
let global_vars: &[VarDecl] = config.map(|c| c.global_var.as_slice()).unwrap_or(&[]);
```

Change to prepend the system uptime globals:
```rust
let user_globals: &[VarDecl] = config.map(|c| c.global_var.as_slice()).unwrap_or(&[]);
let mut global_vars: Vec<VarDecl> = Vec::new();
if options.allow_system_uptime_global {
    global_vars.push(VarDecl::simple(
        "__SYSTEM_UP_TIME",
        VariableType::Global,
        "TIME",
    ));
    global_vars.push(VarDecl::simple(
        "__SYSTEM_UP_LTIME",
        VariableType::Global,
        "LTIME",
    ));
}
global_vars.extend_from_slice(user_globals);
```

The `compile()` function signature must be extended to accept
`options: &CompilerOptions`. Currently it is:
```rust
pub fn compile(library: &Library, context: &SemanticContext) -> Result<Container, Diagnostic>
```

Change to:
```rust
pub fn compile(library: &Library, context: &SemanticContext, options: &CompilerOptions) -> Result<Container, Diagnostic>
```

Update all callers (in `compiler/plc2x/src/stages.rs` and tests).

### Step 4: Set flag in container header

**File: `compiler/container/src/header.rs`**

Add constant:
```rust
pub const FLAG_HAS_SYSTEM_UPTIME: u8 = 0x01;
```

**File: `compiler/codegen/src/compile.rs`**

When building the container header, set the flag:
```rust
if options.allow_system_uptime_global {
    header.flags |= header::FLAG_HAS_SYSTEM_UPTIME;
}
```

### Step 5: VM writes uptime before each scan

**File: `compiler/vm/src/vm.rs`** (~line 276, the INPUT_FREEZE stub)

Replace the no-op stub:
```rust
// System variable injection: write monotonic uptime to VarIndex(0) and VarIndex(1).
if self.container.header.flags & header::FLAG_HAS_SYSTEM_UPTIME != 0 {
    let time_ms = (current_time_us / 1000) as i64;
    // __SYSTEM_UP_TIME: i32 milliseconds (wrapping)
    self.variables
        .store(VarIndex::new(0), Slot::from_i32(time_ms as i32))
        .expect("system uptime variable must exist at index 0");
    // __SYSTEM_UP_LTIME: i64 milliseconds (non-wrapping)
    self.variables
        .store(VarIndex::new(1), Slot::from_i64(time_ms))
        .expect("system uptime variable must exist at index 1");
}
```

Check whether `Slot::from_i32` and `Slot::from_i64` exist — may need equivalent
based on the current Slot API in `compiler/vm/src/value.rs`.

### Step 6: Tests

**Codegen test:**
- Verify `__SYSTEM_UP_TIME` gets `VarIndex(0)` and `__SYSTEM_UP_LTIME` gets `VarIndex(1)` when flag is on
- Verify container header has `FLAG_HAS_SYSTEM_UPTIME` bit set
- Verify user globals shift to index 2+

**VM end-to-end test:**
- Program reads both variables via `VAR_EXTERNAL`, stores in local variables
- Run two scan rounds with different `current_time_us` values (e.g., 1_000_000 and 5_000_000)
- Assert `__SYSTEM_UP_TIME` reflects `(current_time_us / 1000) as i32` (1000 and 5000)
- Assert `__SYSTEM_UP_LTIME` reflects `(current_time_us / 1000) as i64` (1000 and 5000)

**Integration test (full ST pattern):**
```iec
FUNCTION TIME : TIME
VAR_EXTERNAL
    __SYSTEM_UP_TIME : TIME;
END_VAR
    TIME := __SYSTEM_UP_TIME;
END_FUNCTION

PROGRAM main
VAR
    t : TIME;
END_VAR
    t := TIME();
END_PROGRAM
```

Compile with `--allow-time-as-function-name --allow-system-uptime-global`
(or `--dialect rusty`). Run with `current_time_us = 5_000_000`. Verify
`t = T#5000ms`.

### Step 7: Documentation

**New file: `docs/reference/extension-library/system-variables.rst`**
- Document both `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` as system-provided globals
- Explain: updated before each scan, contain ms since VM start
- Document wrap behavior: TIME wraps at ~24.8 days, LTIME effectively never wraps
- Document epoch and restart behavior
- Document multi-task semantics: all tasks in the same scan round see the same values
- Show enabling: `--allow-system-uptime-global` or `--dialect rusty`

**New file: `docs/reference/extension-library/examples/time-function.rst`**
- Show how to write `FUNCTION TIME : TIME` wrapping `__SYSTEM_UP_TIME`
- Show the OSCAT pattern: `x := TIME(); y := TIME_TO_DWORD(x);`

## Key Files

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | New `allow_system_uptime_global` option |
| `compiler/analyzer/src/stages.rs` | Register both globals in symbol env; reserved-name rule |
| `compiler/codegen/src/compile.rs` | Inject two synthetic VarDecls, set header flag, accept options |
| `compiler/container/src/header.rs` | `FLAG_HAS_SYSTEM_UPTIME` constant |
| `compiler/vm/src/vm.rs` | Write both uptime values in `run_round()` |
| `compiler/plc2x/src/lsp.rs` | LSP flag mapping |
| `specs/adrs/0030-dual-uptime-system-variables.md` | ADR documenting design decision |

## Verification

1. `cd compiler && just compile` — builds cleanly
2. `cd compiler && just test` — all tests pass
3. End-to-end: `VAR_EXTERNAL __SYSTEM_UP_TIME : TIME` reads correct ms value
4. End-to-end: `VAR_EXTERNAL __SYSTEM_UP_LTIME : LTIME` reads correct ms value
5. Full pattern: user-defined `FUNCTION TIME : TIME` works under `--dialect rusty`
6. `cd compiler && just` — full CI passes (clippy + tests + coverage)
