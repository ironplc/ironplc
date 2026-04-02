# Expose Monotonic Timer via Implicit `__SYSTEM_TIME_MS` VAR_GLOBAL

## Context

IronPLC needs to let user code access the VM's monotonic timer so that
dialect-specific functions like CODESYS's `TIME()` can be written as ordinary
ST functions. The IEC 61131-3 standard does **not** define any monotonic elapsed
timer function — `TIME()` returning ms-since-boot is a CODESYS vendor extension.

Rather than adding a dedicated VM opcode, we expose the time through the
existing global variable mechanism:

1. The compiler injects an implicit `VAR_GLOBAL __SYSTEM_TIME_MS : TIME`
2. The VM writes `(current_time_us / 1000) as i32` into that variable before each scan
3. Users access it via `VAR_EXTERNAL __SYSTEM_TIME_MS : TIME` in their own functions/FBs
4. A user-defined `FUNCTION TIME : TIME` wrapping this global provides the CODESYS-compatible API

This keeps the VM opcode-free for time access and lets users build arbitrary
time abstractions (TIME(), T_PLC_MS, etc.) in pure ST.

### Research Summary

| Source | Monotonic Timer | Standard? |
|--------|----------------|-----------|
| IEC 61131-3 Ed 2 | RTC FB (wall-clock only, must be initialized) | Yes |
| IEC 61131-3 Ed 3 | CONCAT/SPLIT/DAY_OF_WEEK (manipulation only) | Yes |
| CODESYS | `TIME()` operator (ms since boot) | No — vendor extension |
| TwinCAT | No dedicated function | N/A |
| Siemens | No dedicated function | N/A |
| OSCAT | `T_PLC_MS()` wraps `TIME()` | Depends on vendor `TIME()` |

**Key finding:** No edition of IEC 61131-3 defines a monotonic elapsed timer
function. The `TIME()` operator is purely CODESYS-specific. The standard only
provides timer FBs (TON/TOF/TP with ET output) for measuring elapsed time
while a condition is true, and RTC for wall-clock tracking.

## Design

### Variable Table Layout (Extended)

```
Index:  0               1 .. G      G+1 .. N        N+1 .. M
        ───────────    ─────────    ────────────    ─────────────
        __SYSTEM_       user        program         function vars
        TIME_MS         globals     locals
        (injected)                  (VAR_EXTERNAL skipped — aliases globals)
```

The system time global is always injected first (before user globals), so it
occupies `VarIndex(0)`. User globals shift to index 1+. Since `VAR_EXTERNAL`
references are resolved by **name** (not index), existing user code is
unaffected.

### Container Flag

A flag bit in `FileHeader.flags` (currently always 0) tells the VM whether
the container expects the system time variable at index 0:

```rust
pub const FLAG_HAS_SYSTEM_TIME_MS: u8 = 0x01;
```

The VM checks this flag in `run_round()` and, if set, writes the converted
time before executing tasks.

### TIME Data Type

`(current_time_us / 1000) as i32` — milliseconds, wrapping after ~24.8 days.
This matches IronPLC's existing i32-based TIME representation and is consistent
with CODESYS behavior (where TIME wraps, though CODESYS uses u32).

### Gating

New compiler option `allow_system_time_global`:
- Enabled by default for `--dialect rusty`
- Disabled for strict IEC dialects
- CLI flag: `--allow-system-time-global`

The double-underscore prefix convention (`__SYSTEM_TIME_MS`) makes name
conflicts extremely unlikely. A future enhancement could add a
`--system-time-global-name` option for renaming if needed.

## Implementation Steps

### Step 1: Add compiler option

**File: `compiler/parser/src/options.rs`**

Add entry to `define_compiler_options!` macro:
```
"Expose __SYSTEM_TIME_MS as an implicit VAR_GLOBAL (runtime monotonic clock)",
"--allow-system-time-global",
[Rusty],
allow_system_time_global,
```

Update test assertions: `FEATURE_DESCRIPTORS.len()` and `rusty_features.len()`
each increment by 1.

**File: `compiler/plc2x/src/lsp.rs`** (~line 64)

Add `options.allow_system_time_global |= flag("allowSystemTimeGlobal");`

### Step 2: Register implicit global in analyzer (if needed)

**File: `compiler/analyzer/src/stages.rs`** (~line 104)

The analyzer's `xform_resolve_symbol_and_function_environment.rs` currently
has a TODO for VAR_EXTERNAL cross-validation — external references aren't
fully checked against globals. Verify whether `VAR_EXTERNAL __SYSTEM_TIME_MS :
TIME` compiles without the global being in the symbol table:

- If it compiles: no analyzer change needed (just codegen injection)
- If it fails: add the global to the symbol environment:

```rust
if options.allow_system_time_global {
    symbol_environment.insert(
        &Id::from("__SYSTEM_TIME_MS"),
        SymbolKind::Variable,
        &ScopeKind::Global,
    )?;
}
```

### Step 3: Inject synthetic global in codegen

**File: `compiler/codegen/src/compile.rs`** (~line 148)

Currently:
```rust
let global_vars: &[VarDecl] = config.map(|c| c.global_var.as_slice()).unwrap_or(&[]);
```

Change to prepend the system time global:
```rust
let user_globals: &[VarDecl] = config.map(|c| c.global_var.as_slice()).unwrap_or(&[]);
let mut global_vars: Vec<VarDecl> = Vec::new();
if options.allow_system_time_global {
    global_vars.push(VarDecl::simple(
        "__SYSTEM_TIME_MS",
        VariableType::Global,
        "TIME",
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
pub const FLAG_HAS_SYSTEM_TIME_MS: u8 = 0x01;
```

**File: `compiler/codegen/src/compile.rs`**

When building the container header, set the flag:
```rust
if options.allow_system_time_global {
    header.flags |= header::FLAG_HAS_SYSTEM_TIME_MS;
}
```

### Step 5: VM writes system time before each scan

**File: `compiler/vm/src/vm.rs`** (~line 276, the INPUT_FREEZE stub)

Replace the no-op stub:
```rust
// System variable injection: write monotonic timer to VarIndex(0).
if self.container.header.flags & header::FLAG_HAS_SYSTEM_TIME_MS != 0 {
    let time_ms = (current_time_us / 1000) as i32;
    self.variables
        .store(VarIndex::new(0), Slot::from_i32(time_ms))
        .expect("system time variable must exist at index 0");
}
```

Check whether `Slot::from_i32` exists — may need `Slot(time_ms as u64)` or
equivalent based on the current Slot API in `compiler/vm/src/value.rs`.

### Step 6: Tests

**Codegen test:**
- Verify `__SYSTEM_TIME_MS` gets `VarIndex(0)` when flag is on
- Verify container header has `FLAG_HAS_SYSTEM_TIME_MS` bit set
- Verify user globals shift to index 1+

**VM end-to-end test:**
- Program reads `__SYSTEM_TIME_MS` via `VAR_EXTERNAL`, stores in local variable
- Run two scan rounds with different `current_time_us` values (e.g., 1_000_000 and 5_000_000)
- Assert the variable reflects `(current_time_us / 1000) as i32` (1000 and 5000)

**Integration test (full ST pattern):**
```iec
FUNCTION TIME : TIME
VAR_EXTERNAL
    __SYSTEM_TIME_MS : TIME;
END_VAR
    TIME := __SYSTEM_TIME_MS;
END_FUNCTION

PROGRAM main
VAR
    t : TIME;
END_VAR
    t := TIME();
END_PROGRAM
```

Compile with `--allow-time-as-function-name --allow-system-time-global`
(or `--dialect rusty`). Run with `current_time_us = 5_000_000`. Verify
`t = T#5000ms`.

### Step 7: Documentation

**New file: `docs/reference/extension-library/system-variables.rst`**
- Document `__SYSTEM_TIME_MS` as a system-provided global variable
- Explain: type `TIME`, updated before each scan, contains ms since VM start
- Show enabling: `--allow-system-time-global` or `--dialect rusty`

**New file: `docs/reference/extension-library/examples/time-function.rst`**
- Show how to write `FUNCTION TIME : TIME` wrapping `__SYSTEM_TIME_MS`
- Show the OSCAT pattern: `x := TIME(); y := TIME_TO_DWORD(x);`

## Key Files

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | New `allow_system_time_global` option |
| `compiler/analyzer/src/stages.rs` | Register global in symbol env (if needed) |
| `compiler/codegen/src/compile.rs` | Inject synthetic VarDecl, set header flag, accept options |
| `compiler/container/src/header.rs` | `FLAG_HAS_SYSTEM_TIME_MS` constant |
| `compiler/vm/src/vm.rs` | Write time to `VarIndex(0)` in `run_round()` |
| `compiler/plc2x/src/lsp.rs` | LSP flag mapping |

## Verification

1. `cd compiler && just compile` — builds cleanly
2. `cd compiler && just test` — all tests pass
3. End-to-end: `VAR_EXTERNAL __SYSTEM_TIME_MS : TIME` reads correct ms value
4. Full pattern: user-defined `FUNCTION TIME : TIME` works under `--dialect rusty`
5. `cd compiler && just` — full CI passes (clippy + tests + coverage)
