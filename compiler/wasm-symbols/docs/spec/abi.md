# Runtime ABI and execution model

ABI version **1.1**.

This document specifies the contract between a logic module and the runtime
that executes it. Version 1.1 adds optional features to version 1.0. Every
addition is backward compatible: a 1.0 runtime ignores what it does not know,
and a 1.1 runtime loads 1.0 modules.

## 1. Module format

- **ABI-001 (M)** A logic module shall be a WebAssembly core module (WebAssembly 2.0) using only these features: mutable globals, sign-extension operators, non-trapping float-to-int conversions, multi-value, bulk memory operations, and reference types limited to one `funcref` table. SIMD, threads, exceptions, tail calls, garbage collection and 64-bit memories shall not be used.
- **ABI-002 (M)** The module shall define exactly one linear memory and export it as `memory`; the memory shall not be shared; its minimum size shall hold all static data; the module shall not rely on `memory.grow`.
- **ABI-003 (M)** The module shall import only functions of the `plc_rt` module listed in section 4; a runtime shall refuse a module importing anything else.
- **ABI-004 (M)** The module shall contain the custom sections `plc.meta` (symbol map, [symbol-map.md](symbol-map.md)) and `plc.build` (section 8).
- **ABI-005 (M)** The module shall be deterministic: its behaviour shall depend only on its memory, the values returned by the imports, and the calls of the runtime.

## 2. Exports

| Export | Type | Role | Since |
|---|---|---|---|
| `memory` | memory | Linear memory | 1.0 |
| `plc_abi_version` | `() -> i32` | ABI major version (1) | 1.0 |
| `plc_init` | `() -> i32` | Sets every variable to its initial value; 0 on success | 1.0 |
| `plc_task_run` | `(task: i32) -> i32` | Runs one cycle of the programs of a task; 0 on success, -1 for an unknown task | 1.0 |
| `plc_retain_base`, `plc_retain_size` | immutable `i32` globals | Retain region (section 5) | 1.1 |
| `plc_input_base`, `plc_input_size`, `plc_output_base`, `plc_output_size`, `plc_marker_base`, `plc_marker_size` | immutable `i32` globals | Process image regions (section 6) | 1.1 |
| `plc_fuel` | mutable `i64` global | Fuel counter, only when instrumentation is enabled (section 9) | 1.1 |
| `plc_trap_code`, `plc_trap_site` | mutable `i32` globals | Cause and source site of the last trap raised by an inserted check (section 10) | 1.1 |

- **ABI-010 (M)** The module shall export `memory`, `plc_abi_version`, `plc_init` and `plc_task_run` with exactly these types.
- **ABI-011 (M)** `plc_init` shall set every variable of the module, retentive or not, to its initial value (LANG-050), including the internal state of function block instances, and shall not call `plc_rt` imports other than `log`.
- **ABI-012 (M)** `plc_task_run(t)` shall run, in order, the program instances assigned to task `t` in the symbol map, each once; task numbers are the indexes of the `tasks` list of the symbol map.
- **ABI-013 (M)** A module produced by `ironplcc` shall export the 1.1 globals of sections 5, 6 and 10; a runtime shall not require them from a 1.0 module.
- **ABI-014 (C)** A module could export a per-program entry point `plc_program_<NAME>` (no parameter, no result) for tools that run one program alone; the runtime shall not require it.

## 3. Execution model

One cycle of a task, driven by the runtime:

1. freeze the cycle time returned by `now_ns`;
2. write the input image into the input region (section 6);
3. apply forced values;
4. call `plc_task_run(task)`;
5. apply forced values again;
6. read the output region into the output image;
7. sample the variables observed by clients.

- **ABI-020 (M)** The module shall read located inputs only from the input region and write located outputs only to the output region; it shall never exchange I/O through imports.
- **ABI-021 (M)** The module shall not modify memory outside a call of one of its exports.
- **ABI-022 (M)** Between two calls, the runtime may read and write any variable listed in the symbol map (debugging, forcing, supervision); the module shall not cache variable values in WebAssembly globals or locals across calls.
- **ABI-023 (M)** If a runtime error occurs (section 10), then the call shall end with a WebAssembly trap; the memory stays readable for diagnosis, and the runtime applies its error handling.

## 4. Imports (`plc_rt`)

| Import | Type | Role | Required |
|---|---|---|---|
| `now_ns` | `() -> i64` | Cycle time in nanoseconds, monotonic, constant during a cycle | When the module uses timers or reads the time |
| `log` | `(level: i32, ptr: i32, len: i32) -> ()` | UTF-8 message to the runtime log | Optional |
| `debug_hook` | `(site: i32) -> ()` | Called before each statement when debug instrumentation is enabled | Only with the debug option |

- **ABI-030 (M)** The module shall import `now_ns` only if it needs the time, and `debug_hook` only if debug instrumentation was requested (API-022); a runtime shall provide `now_ns` and `log` always, and `debug_hook` when it supports debugging.
- **ABI-031 (M)** Where timers are used, they shall compute elapsed times from `now_ns` values only, so that the runtime controls real or virtual time (LANG-090).
- **ABI-032 (M)** `debug_hook` sites shall be indexes into the `sites` table of the symbol map, giving the source span of the statement.

## 5. Memory layout

All variables have a fixed address chosen at compile time and published in the
symbol map. The layout rules below are part of the ABI because tools read
memory through the symbol map; how the compiler orders its regions is not.

- **ABI-040 (M)** Addresses 0 to 1023 shall not hold any variable, so that address 0 never designates data.
- **ABI-041 (M)** Scalar sizes and representations shall be those of [language-subset.md](language-subset.md), section 3.1, little-endian; `BOOL` is one byte holding 0 or 1; every scalar is aligned on its size (natural alignment).
- **ABI-042 (M)** A structure shall store its members in declaration order, each at the next offset aligned on its alignment; the alignment of the structure is the largest alignment of its members, and its size is rounded up to it.
- **ABI-043 (M)** An array shall store its elements in row-major order (last index varying fastest), with a stride equal to the element size rounded up to the element alignment.
- **ABI-044 (M)** `STRING[n]` shall occupy `n + 1` bytes: the characters, then a terminating zero, the current length being the position of the first zero; `WSTRING[n]` shall occupy `2(n + 1)` bytes on the same principle with 16-bit code units. Alignment 1 and 2.
- **ABI-045 (M)** An enumeration shall be stored as its base type (default `DINT`); a subrange as its base type.
- **ABI-046 (M)** A function block or program instance shall be laid out as a structure whose members are its `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` (as 4-byte addresses), `VAR` and `VAR_STAT` variables in declaration order, followed by the internal state of standard function blocks where applicable; `VAR_TEMP` variables are not part of the instance.
- **ABI-047 (M)** Retentive variables shall be stored in the retain region, a contiguous range published by `plc_retain_base` and `plc_retain_size` and by the symbol map; a function block type with retentive members has its instance split into a volatile part and a retentive part.
- **ABI-048 (M)** Each static datum (global, program instance, frame) shall have an address independent of the compilation order of unrelated POUs only within a compilation; the ABI does not guarantee stable addresses across compilations. Tools shall always use the symbol map.

## 6. Process image

- **ABI-050 (M)** Located variables shall be allocated in three contiguous regions: inputs (`%I`), outputs (`%Q`) and markers (`%M`), published by the `plc_*_base` and `plc_*_size` globals and by the `location` field of the symbol map.
- **ABI-051 (M)** A located variable is stored like any variable of its type (a located `BOOL` takes one byte); the runtime packs and unpacks the I/O of its drivers into these variables using the symbol map, including bit addresses (`%IX0.3`).
- **ABI-052 (M)** The number in `%IW`, `%ID`, `%IL` (and `%Q`, `%M`) addresses shall be read as a byte offset in the image; `%IXb.x` designates bit `x` of byte `b`; overlaps between located variables of different sizes are allowed and reported as a warning.

## 7. Calling convention (internal, informative)

This section is not part of the contract: the runtime never calls POUs
directly. It documents what debuggers may see in stack traces.

- Each POU is one WebAssembly function.
- Programs use absolute addresses of their static instance.
- A function block receives `(plain: i32, retain: i32, non_retain: i32)`: the addresses of the three parts of its instance.
- A function uses one static frame holding its inputs, outputs, locals and result, reset at each call; the caller writes the arguments into the frame, calls, then reads the result and outputs.
- `VAR_TEMP` variables of programs and function blocks live in a static frame reset at the start of each call.
- Control flow lowering:
  - `IF`: nested `if ... else ... end`;
  - `CASE`: a `br_table` when the labels are dense, a chain of comparisons otherwise;
  - `WHILE`, `REPEAT`, `FOR`: `block $exit (loop $top (block $continue body) step (br $top))`, with the exit test at the top (`WHILE`, `FOR`) or at the bottom (`REPEAT`);
  - `EXIT`: `br $exit` of the innermost loop;
  - `CONTINUE`: `br $continue` of the innermost loop, which falls through to the step and the next test;
  - `RETURN`: `return` from the WebAssembly function of the POU, after copying nothing (results and outputs already live in memory).

## 8. Build section (`plc.build`)

CBOR map (RFC 8949) with the keys:

| Key | Type | Content |
|---|---|---|
| `abi_major` | uint | 1 |
| `abi_minor` | uint | 1 |
| `source_sha256` | text | SHA-256 of the sources (names and normalised texts) |
| `compiler` | text | Compiler name and version |
| `retain_layout` | text | SHA-256 of the list of retentive leaves (path, type, offset) |
| `options` | map | Instrumentation options used (`fuel`, `debug`, `checks`) |

- **ABI-060 (M)** A runtime shall restore a saved retain region byte for byte only if its `retain_layout` equals that of the new module; otherwise it shall restore per variable, by path and type, through the symbol map.

## 9. Fuel instrumentation (optional)

- **ABI-070 (M)** Where fuel instrumentation is enabled (API-022), the module shall export the mutable global `plc_fuel`, decrement it at the entry of each POU and at each loop back-edge by the number of WebAssembly instructions of the code executed since the previous decrement (static count per basic block), and, if it becomes negative, set `plc_trap_code` to 1 and trap.
- **ABI-071 (M)** The runtime shall set `plc_fuel` before each `plc_task_run`; the count shall be identical on every engine, so that an overrun is detected at the same point in the browser, on a PC and on a board.

## 10. Runtime errors

| Code | Cause | Detected by |
|---|---|---|
| 0 | Unknown (trap raised by the engine) | Engine |
| 1 | Fuel exhausted | Inserted check |
| 2 | Integer division or `MOD` by zero | Inserted check |
| 3 | Array index out of bounds | Inserted check |
| 4 | Subrange value out of range on assignment | Inserted check |
| 5 | Integer division overflow (minimum value divided by -1) | Inserted check |

- **ABI-080 (M)** Before trapping on an inserted check, the module shall store the code in `plc_trap_code` and the index of the source site in `plc_trap_site` (a `sites` entry of the symbol map), then execute `unreachable`.
- **ABI-081 (M)** `plc_init` and `plc_task_run` shall set `plc_trap_code` to 0 on entry.
- **ABI-082 (S)** Checks 3 and 4 should be removable by an option for measurements, the default being checks enabled.
