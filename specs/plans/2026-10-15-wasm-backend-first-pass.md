# Plan: A First-Pass WebAssembly Backend

Tracking issue: #1968. The core change PRs below are listed on that issue,
which stays open until they have all landed.

## Goal

Add `ironplc-wasm`, a backend that translates a lowered program into a
WebAssembly module. It follows [Lowered Program](../design/lowered-program.md)
and ADR-0057 to ADR-0064. The point of the first pass is the one the design's
Delivery Constraints give for a minimal WebAssembly path: show that nothing in
the lowered program assumes the bytecode VM, before the rest of codegen moves
onto it.

The first pass covers what lowering covers today:

- `PROGRAM` bodies;
- scalar expressions of every elementary type, with their conversions,
  truncations and the scalar `Intrinsic`s;
- control flow: `If`, `Case`, `Loop`, `For`, `Exit`, `Continue`, `Return`;
- places through `Field`, `Index` and the non-null `Deref` of a body's
  instance parameter, so arrays and structures;
- `STRING` and `WSTRING` values and the standard string functions.

Out of scope, each reported as P9999 at the construct's span:

- calls of user functions (`Callee::User`) and every function block, including
  the standard ones: call binding and function blocks are still being
  refactored;
- nullable references (`RefTo`, `Null`, `Deref` of a `REF_TO`);
- converting a `REAL` to or from a `STRING`;
- the watchdog;
- selecting the target from `ironplcc`. Wiring the backend into
  `ironplc_project` and the CLI, with user documentation, is a follow-up
  issue (Backend Guide, Step 6).

The closed draft #1846 (`s-celles-bot:feature/wasm-target`) is prior art
only. The module ABI and the symbol map below may reuse its ideas. Its code
generator is not reused, because it compiles the AST, which ADR-0059 forbids.

## Architecture

```
CleanAnalysis ──▶ ironplc-lowering::lower_program ──▶ lowered::Program ──▶ ironplc-wasm::compile ──▶ WasmModule (bytes)
                  (Core 1)                             (Prefactor 4)        (Core 2 to Core 6)
```

**Crate.** The crate is `compiler/wasm` (`ironplc-wasm`, slug `wasm`). Its
`[dependencies]` are `ironplc-ir`, `ironplc-problems` and `wasm-encoder`, and
nothing else (ADR-0059). The crate root carries
`#![deny(clippy::wildcard_enum_match_arm)]`. The entry point is:

```rust
pub fn compile(program: &lowered::Program, options: &WasmOptions)
    -> Result<WasmModule, Vec<Diagnostic>>;
```

It reports every limit it finds, not only the first. `WasmOptions` is empty in
the first pass. It exists so that target choices, such as a feature set, have
a place that cannot hold `CompilerOptions`. There is no `SourceLookup`
parameter, because the first pass emits no line map. `SourceLookup` moves to
`ironplc-ir` when a line map is added, as the Backend Guide says.

**Phases**, as in the Backend Guide:

1. **Capability checks** read the execution model. A model with more than one
   program instance is P9999, as in the VM (REQ-EM-codegen-002). An event task
   is P4047. A layout larger than 32-bit linear memory is P9997.
2. **Layout** is computed once, before emission, and keyed by `VarId`,
   `TypeId` and `FieldIdx`. Every global and program instance gets a static
   address in a single linear memory. Address 0 is never allocated, so a
   future null check has a value to compare against. Values are stored at
   their storage width, little-endian and naturally aligned: a `BOOL` takes
   one byte and an `INT` takes two. An `INT` is loaded with `i32.load16_s`,
   and every operation runs at its `ScalarType` (ADR-0001). A string is a
   4-byte header (capacity `u16`, length `u16`) followed by its code units:
   one byte for `STRING`, two for `WSTRING`. The capacity comes from the
   value's own `StringShape` (ADR-0064). Every string `Call` gets its own
   static scratch buffer, sized from that call's shape. No call is recursive
   or re-entered (REQ-LOW-lowering-059), so no pool and no release is needed.
3. **Emission.** Each `PROGRAM` body becomes a function `(this: i32)`. A field
   of the instance is a load or store at `this` plus the field's offset
   (ADR-0062). `For` uses the IR's expansion (REQ-LOW-ir-068). `Case`
   evaluates its selector once into a local. A place is addressed once per
   statement that names it once. Every match over an IR enum names every
   variant.
4. **Meaning of operations** (§3.10). Where the WebAssembly instruction
   differs from the rule, the backend emits the difference:
   - signed `Div` and `Mod` are guarded for a divisor of zero (trap V4001)
     and for the most negative value divided by -1 (§3.10: that value, and
     0);
   - a real converts to an integer through `trunc_sat`;
   - the count of `SHL` and `SHR` is reduced modulo the operation width
     explicitly;
   - `Index` checks the flat position (V4005, ADR-0023);
   - `ConvertChecked` checks its range (V4007);
   - `EXPT` of a signed integer by a negative exponent traps (V4002).
   Every trap is emitted explicitly, even where WebAssembly would trap on its
   own, so that the trap carries its code.

**Module ABI**, stated in the design doc and pinned by its requirements:

- **Exports:** `memory`; `init()`, which runs the lowered `init` once;
  `round(time_us: i64)`, which stores the round time in a global and runs
  each program instance in order; and an `i32` global `trap_code`.
- **Traps:** before each trap the module stores the trap's number (4001 to
  4007) in `trap_code`, then executes `unreachable`. The host reads the code
  after the trap. As on the VM, the host runs no further round.
- **Imports:** only for real functions that have no WebAssembly instruction
  (`SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LN`, `LOG`, `EXP`, real
  `EXPT`), from module `ironplc`, and only the ones the program uses. Their
  results may differ from the VM's in the last bits (REQ-LOW-codegen-132).
- **Symbol map:** a custom section `ironplc.symbols`. It lists every global,
  program instance and field by its source name and owner, with its address
  in memory and its type (kind, width and signedness; for a string, its
  encoding and capacity; for an array, its dimensions and element). It is
  built only from the names the lowered program carries
  (REQ-LOW-lowering-006). This is what the tests read variables through by
  name, so a wrong symbol map fails the tests and not only a debugger.

Scheduling stays on the host: the module runs one round when asked. A task's
interval is not used, because the first pass accepts one program instance.

**Test engine: `wasmi`, as a dev-dependency only.** `wasmi` is a WebAssembly
interpreter written in pure Rust. The reasons for choosing it:

- **It builds the same way on Windows, macOS and Linux.** It needs no C
  toolchain and generates no machine code, so CI has no platform-specific JIT
  to break.
- **It is cheap to build.** It has a small dependency tree. `wasmtime` pulls
  in Cranelift, which costs build time and disk on every test binary and
  every `cargo llvm-cov` run of `cd compiler && just`. The project already
  works to keep those down (the 2026-05-02 plan to reduce llvm-cov binaries).
- **It has what the backend emits:** non-trapping float-to-int conversion,
  sign extension, mutable globals and multiple values. It validates a module
  when it loads it, so every test also checks that the module is valid
  WebAssembly.
- **It meters fuel**, which a later watchdog test can use to stop a
  non-terminating `FOR` (REQ-LOW-lowering-066) without wall-clock timing.

Alternatives considered:

- `wasmtime` would be the choice for a production runtime, not for a test
  dependency.
- `wasmer` has the same weight as `wasmtime`, with more back ends than the
  tests need.
- Running node or V8 as a subprocess would make `cargo test` depend on a tool
  outside cargo, which the build machines and this repository's environment
  cannot assume.

The encoder, `wasm-encoder`, is a regular dependency. It is pure Rust from the
Bytecode Alliance, has almost no dependencies, and gives typed instructions,
so the backend never writes raw opcodes.

**Tests.** The tests are the legs of the Backend Guide's Testing table:

- **Backend comparison.** It compiles each program of a corpus through both
  backends and compares every variable by name. Reals pass within the stated
  tolerance; every other value must be equal (REQ-LOW-codegen-132). It lives
  in `codegen/tests/it/compare_wasm_*.rs`, next to the VM harness
  (`Snapshot`, `Value`) it reuses, because an integration test's helpers
  cannot be imported from another crate. `ironplc-wasm` and `wasmi` become
  dev-dependencies of `ironplc-codegen`. Dev-dependencies between backends
  are allowed (Backend Guide). The corpus is taken from end-to-end programs
  whose statements all lower.
- **Meaning of operations:** one program per §3.10 rule, with the trap codes.
  These tests and the next two live in `compiler/wasm/tests/it` and check
  against stated values. The wasm crate's dev-dependencies are the front end
  (`ironplc-parser`, `ironplc-analyzer`, `ironplc-lowering`, to get a lowered
  program from source), `wasmi` and `spec_test_macro`.
- **Capability checks:** each target-limit diagnostic and its code.
- **Structure:** only what end-to-end cannot localize: layout offsets and the
  symbol map.

The shared end-to-end suite in `codegen/tests/it` (215 files) stays where it
is in the first pass. Making `Snapshot` and `Session` a trait, and marking the
tests a backend cannot run, is a follow-up issue. The design doc records that
decision as End-to-End Test Observation asks of the first new backend. Doing
it now would be a rewrite much larger than the feature (Development
Standards, "When not to prefactor"). The comparison corpus is how the first
pass gets the same assurance.

## Prefactoring

Four behaviour-preserving prefactors. Each makes something the backend needs
available without adding a dependency the backend may not have, or adds
enforcement the Backend Guide assumes is already there:

1. **The design doc** (documentation only): `specs/design/wasm-backend.md`
   with status `proposed`.
2. **The boundary check outside every backend.** The Backend Guide says a
   check reads each backend's manifest and crate root, but no such check
   exists yet: only codegen's lowered bridge tests its own lint. The check
   goes in `ironplc-test`. It finds backends by itself: a workspace member
   that depends on `ironplc-ir` and is not one of the pipeline crates
   (`analyzer`, `lowering`, `project`, the CLI front ends). It checks each
   backend's `[dependencies]` against the forbidden list, and checks that its
   crate root carries the lint. `ironplc-codegen` is exempt from the
   dependency half until its AST route is deleted (Delivery Constraints),
   with a comment citing the design. With this check in place, `ironplc-wasm`
   is covered from its first commit.
3. **The trap codes in `ironplc-ir`.** A `TrapCode` enum (V4001 to V4007),
   each variant with its number and name. A test in `ironplc-ir` checks them
   against `compiler/vm/resources/problem-codes.csv`, because the `no_std` VM
   (ADR-0010) cannot depend on `ironplc-ir`. Nothing uses the enum yet. The
   backend needs it so it never depends on `ironplc-vm`.
4. **A whole lowered `Program` in `ironplc-ir`.** Today `ProgramBuilder`
   holds tables, and lowering hands codegen one statement at a time. A
   backend needs the whole program. This prefactor adds:
   - `lowered::Program`: the types, variables, POUs with their bodies,
     `init`, and the execution model, with each program instance bound to the
     `VarId` of its instance and the `PouId` of its body;
   - `ProgramBuilder::finish`, which checks that every body is set;
   - the text printer for it.
   Nothing in the compiler builds one yet, so behaviour is unchanged.

A fifth candidate, moving `Value` and the VM's read-by-name out of
`codegen/tests/it/common` so that both crates' tests share them, is left out
on purpose. `value.rs` reads the container's type tags, so the move would
reshape codegen's harness for a single caller, and it is the start of the
trait refactor deferred above. Instead, the comparison leg lives in codegen's
tests. The symbol map is read through a public `SymbolMap::parse` in
`ironplc-wasm`, so the code that reads it is not duplicated: it is part of the
ABI that tools besides the tests will read. The `wasmi` runner, about fifty
lines, exists once in each crate's tests until the harness becomes a trait.

## Design doc reference

- [Lowered Program](../design/lowered-program.md): §3 (data model), §3.10
  (Meaning of operations), §7 (Backend Contract and its WebAssembly column),
  §9 (Testing), Delivery Constraints ("Target neutrality shown early").
- [Backend Guide](../steering/backend-guide.md).
- [The Resolved Execution Model](../design/execution-model.md), Backend
  capability checks.
- [End-to-End Test Observation](../design/end-to-end-test-observation.md),
  "No backend trait yet".
- ADR-0057 to ADR-0064, and ADR-0001, ADR-0005, ADR-0023, ADR-0065.
- New: `specs/design/wasm-backend.md` (Prefactor 1).

## File map

| File | Change |
|---|---|
| `specs/design/wasm-backend.md` | New: ABI, layout, the node-by-node realisation table, every §3.10 difference, capability checks, the test-engine choice, and `REQ-WASM-wasm-NNN` requirements |
| `specs/design/end-to-end-test-observation.md` | Record where the suite lives for the first pass, and that the trait is a follow-up |
| `compiler/test/tests/backend_boundary.rs` | New: the manifest and crate-root check |
| `compiler/test/Cargo.toml` | Dev-dependency for reading TOML, if needed |
| `compiler/ir/src/trap.rs`, `compiler/ir/src/lib.rs` | New `TrapCode`, and its test against the VM's CSV |
| `compiler/ir/src/lowered/program.rs`, `lowered.rs`, `variables.rs`, `print.rs`, `tests.rs` | `Program`, `ProgramBuilder::finish`, POU bodies, binding of program instances, printer |
| `compiler/lowering/src/lib.rs`, `compiler/lowering/src/program.rs` (new), `compiler/lowering/src/init.rs` (new), `*_tests.rs` | `lower_program`, and `init` from the completed initializers |
| `compiler/Cargo.toml` | Add `wasm` to `members` |
| `compiler/Cargo.lock` | New entries for `wasm-encoder` and `wasmi` (required by the new dependencies) |
| `compiler/wasm/Cargo.toml`, `build.rs` | New crate. Dependencies: `ironplc-ir`, `ironplc-problems`, `wasm-encoder`. Dev-dependencies: `wasmi`, `ironplc-parser`, `ironplc-analyzer`, `ironplc-lowering`, `spec_test_macro` |
| `compiler/codegen/Cargo.toml` | Dev-dependencies `ironplc-wasm`, `wasmi`, `ironplc-lowering` for the comparison leg |
| `compiler/codegen/tests/it/common/wasm.rs`, `compiler/codegen/tests/it/compare_wasm_*.rs` | The comparison leg: run a program on both backends and compare by name |
| `compiler/wasm/src/lib.rs` | Crate root, lint, `compile`, `WasmOptions`, `WasmModule` |
| `compiler/wasm/src/capability.rs` | Checks against the execution model |
| `compiler/wasm/src/layout.rs` | Addresses and widths, keyed by id |
| `compiler/wasm/src/emit/{mod,stmt,expr,place,intrinsic,string}.rs` | Emission, each module under 1000 lines |
| `compiler/wasm/src/runtime/string.rs` | String functions emitted as functions inside the module |
| `compiler/wasm/src/symbols.rs` | The `ironplc.symbols` custom section |
| `compiler/wasm/src/spec_conformance.rs` | Conformance tests for the requirements |
| `compiler/wasm/tests/it/{common,compare_*,meaning_*,capability,strings,...}.rs` | `wasmi` host harness and the test legs |
| `compiler/justfile` | Only if the new crate needs listing for coverage or packaging |

No user-facing documentation changes, because the first pass is not
selectable from `ironplcc`. There is no changelog file in the repository.
Release notes come from PR titles.

## Tasks

### Prefactor 1: Design the WebAssembly backend

- [ ] Write `specs/design/wasm-backend.md` (status `proposed`). It covers:
      scope and exclusions; the entry point; the capability checks; the
      layout; a node-by-node table; the module ABI (exports, traps, imports,
      symbol map); every difference between WebAssembly instructions and
      §3.10; the test engine and the reasons for it; and the test legs
- [ ] Give each testable claim a `REQ-WASM-wasm-NNN` ID. Areas:
      - ABI: exports, the trap code, imports used only when needed;
      - layout: computed before emission, keyed by id, address 0 unused,
        string size from its own capacity;
      - capability checks;
      - the symbol map;
      - P9999 for each construct the first pass excludes
- [ ] Record in `end-to-end-test-observation.md` that the suite stays in
      `codegen/tests` for the first pass, and that the trait and the
      "not yet supported" marking are a follow-up issue
- [ ] Credit #1846 as prior art for the ABI and the symbol map
- [ ] `cd specs && just`

### Prefactor 2: Check every backend's dependencies and lint from outside it

- [ ] Add `compiler/test/tests/backend_boundary.rs`. It reads the workspace
      `members`, treats as a backend every member that depends on
      `ironplc-ir` and is not a pipeline crate, fails on a forbidden crate in
      `[dependencies]`, and fails when the crate root lacks
      `#![deny(clippy::wildcard_enum_match_arm)]`
- [ ] Exempt `ironplc-codegen` from the dependency half, with a comment
      citing Lowered Program, Delivery Constraints. Keep the lint half, which
      codegen's lowered bridge already meets
- [ ] Test the check on fixture manifests: a backend with a forbidden
      dependency fails, one without the lint fails, and a clean one passes
- [ ] `cd compiler && just`

### Prefactor 3: Name the run-time trap codes in the IR crate

- [ ] Add `ironplc_ir::trap::TrapCode` for V4001 to V4007, each with its
      number and its problem-code name
- [ ] Add a test that each variant matches its row in
      `compiler/vm/resources/problem-codes.csv` (read with `include_str!`)
- [ ] `cd compiler && just`

### Prefactor 4: Hold a whole lowered program in the IR crate

- [ ] Add `lowered::Program` (types, variables, POUs with bodies, `init`, the
      execution model) and a program-instance binding (`VarId` of the
      instance, `PouId` of its body) for each instance in the model
- [ ] Add `ProgramBuilder::set_body`, `push_init` and `finish`. `finish`
      returns P9998 when a POU has no body or an instance is unbound
      (REQ-LOW-ir-081)
- [ ] Extend the text printer to a whole program, and test that it
      round-trips a hand-built program
- [ ] Test each refusal of `finish`
- [ ] `cd compiler && just`

### Core 1: Lower a whole program and its initial values

- [ ] Add `Lowering::lower_program(analysis, parse) -> Result<Program,
      Vec<Diagnostic>>`. It lowers every top-level statement of each
      reachable POU through the existing statement lowering. A statement it
      does not support is P9999 at the statement's span, and the whole walk
      finishes so that all of them are reported (REQ-LOW-lowering-110,
      REQ-LOW-lowering-111, REQ-LOW-lowering-112)
- [ ] Lower `init` from the completed initializers
      (`xform_resolve_initial_values`, run on a copy as codegen does) for
      globals and program instances: scalars, arrays, structures and strings.
      An initializer of a kind the first pass excludes (a function block
      instance, a reference) is P9999
- [ ] Bind each execution-model program instance to its instance variable and
      body
- [ ] Tests: a program lowers to its printed text; two unsupported statements
      in different POUs give two P9999s; each initializer kind lowers to its
      `Assign` or `AssignStr` statements
- [ ] Coordinate with the maintainer before starting. If the call-binding or
      function block refactor lands its own whole-program entry point first,
      this PR shrinks to the initial values, or is dropped
- [ ] `cd compiler && just`

### Core 2: Compile a program with integer arithmetic and control flow to WebAssembly

- [ ] Create `compiler/wasm` with the crate root, `compile`, `WasmOptions`
      and `WasmModule`, and add it to the workspace `members`
- [ ] Capability checks: more than one program instance is P9999, an event
      task is P4047, a layout over 4 GiB is P9997. Each is tested
- [ ] Layout for the elementary integer types and `BOOL`, the program
      instances and the globals. Emit `init`, `round`, `memory` and
      `trap_code`
- [ ] Emit `Assign`, `Const`, `Read`, `Truncate`, integer
      `Binary`/`Unary`/`Compare`, `ShortCircuit`, `If`, `Case` (selector
      evaluated once), `Loop`, `Exit`, `Continue`, `For` through its IR
      expansion, and `Return`
- [ ] Report every other node as P9999 at its span, through exhaustive
      matches with no wildcard arm
- [ ] Write the `ironplc.symbols` section, and a public `SymbolMap::parse`
      that reads it. Structure tests check its contents
- [ ] Add the `wasmi` runner in `compiler/wasm/tests/it/common`: it loads the
      module, provides the `ironplc` imports, runs `init` and `round`, reads a
      variable by name through `SymbolMap`, and reports `trap_code` after a
      trap
- [ ] Add the comparison leg in `codegen/tests/it/compare_wasm_*.rs`, with a
      first corpus of integer and control-flow programs taken from the
      end-to-end suite
- [ ] List the design doc in `build.rs`, and add `#[spec_test]`s for the
      requirements this PR meets
- [ ] `cd compiler && just`

### Core 3: Compile every elementary type and its operations to WebAssembly

- [ ] Layout and loads and stores for the 64-bit integers, `REAL`, `LREAL`,
      the bit strings, the temporal types, enumerations and subranges
- [ ] `Convert` (with `trunc_sat` from a real), `ConvertChecked` (V4007),
      real arithmetic and comparisons, including `<>` with a NaN
- [ ] Guards for signed `Div` and `Mod`: a zero divisor traps V4001, and the
      most negative value divided by -1 gives the §3.10 result
- [ ] The scalar `Intrinsic`s: shifts with the count reduced modulo the
      width, rotates, `ABS`, `EXPT` (V4002), `MIN`/`MAX`/`LIMIT`/`MUX` with
      the §3.10 NaN rules, BCD. Real functions go through the `ironplc`
      imports
- [ ] Meaning-of-operations tests: the most negative `DINT` divided by -1,
      `REAL_TO_DINT` of a NaN, `SHL` by 33, each trap and its code. Grow the
      comparison corpus
- [ ] `cd compiler && just`

### Core 4: Compile arrays and structures to WebAssembly

- [ ] Layout of arrays (element stride) and structures (field offsets),
      keyed by `TypeId` and `FieldIdx`
- [ ] `Field` and `Index` projections. Check the flat position (V4005,
      ADR-0023), and compute each place's address once per statement
- [ ] `AssignBits`, with the place evaluated once
- [ ] The symbol map covers elements and fields, and the harness reads
      `a[2]` and `s.f`
- [ ] Tests: out-of-bounds traps with V4005; a subscript with a side effect
      is evaluated once. Grow the comparison corpus
- [ ] `cd compiler && just`

### Core 5: Compile strings to WebAssembly

- [ ] String layout from each value's `StringShape`, a static scratch buffer
      for each string `Call`, literals in a data segment, and `AssignStr`
      truncating to the destination's capacity
- [ ] Emit `LEN`, `FIND`, `LEFT`, `RIGHT`, `MID`, `CONCAT`, `INSERT`,
      `DELETE`, `REPLACE` and `CompareString`/`CompareWString` as functions
      inside the module, for both encodings, with the §3.10 position and
      length rules
- [ ] Integer-to-string and string-to-integer conversions, using the policy
      each `Intrinsic` names, with V4006 on a string that does not convert.
      A `REAL` to or from a string is P9999
- [ ] The harness reads `STRING` and `WSTRING` values. Grow the comparison
      corpus with the string end-to-end programs that lower
- [ ] `cd compiler && just`

### Core 6: Complete the first pass of the WebAssembly backend

- [ ] Run the comparison corpus with any target-neutral pass both off and on
      (REQ-LOW-lowering-142)
- [ ] Check that every `REQ-WASM-wasm-NNN` has a real assertion
- [ ] Set the design doc to `partially implemented`, saying which parts are
      built
- [ ] Open the follow-up issues: function calls and function blocks once
      their refactor lands; nullable references; `REAL` to and from strings;
      the watchdog; the shared end-to-end trait and its "not yet supported"
      marking; the line map with `SourceLookup` moved to `ironplc-ir`; and
      wiring into `ironplc_project`, the CLI and the user docs
- [ ] `cd compiler && just`
