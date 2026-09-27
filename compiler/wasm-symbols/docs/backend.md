# Using the WebAssembly back end from another front end

The code generator of the WebAssembly target does not depend on the IronPLC
parser nor on its semantic analysis. Any front end (another IEC 61131-3 compiler, a graphical
language, a generator of test programs) can build a module of the
intermediate representation and obtain a logic module of the logic module
ABI 1.1 ([spec/abi.md](spec/abi.md)) with its symbol map
([spec/symbol-map.md](spec/symbol-map.md)).

| Crate | Depends on | Role |
|---|---|---|
| `ironplc-wasm-ir` | nothing | IR types, text form, validator |
| `ironplc-wasm-symbols` | serde, ciborium | symbol map, JSON and CBOR |
| `ironplc-wasm-codegen` | the two above, wasm-encoder, wasmparser | WebAssembly emission and validation |
| `ironplc-wasm` | IR, IronPLC analyzer | lowers an analysed IronPLC program to the IR |

`compiler/wasm-codegen/tests/standalone.rs` builds a module by hand and
runs the result; it is the smallest complete example.

## The module

A [`Module`] holds:

- **objects**: static memory, each with a region (`Static`, `Retain`,
  `Input`, `Output`, `Marker`), an alignment and its initial bytes, whose
  length is its size. The code generator places the objects of each region
  together from address 1024 and publishes the regions (ABI section 5);
  `plc_init` copies every initial image.
- **functions**: programs (no parameter, absolute addresses), function
  blocks (three `i32` parameters: the addresses of the plain, `RETAIN` and
  `NON_RETAIN` parts of the instance) and functions (a static
  frame, reset by the caller). Recursion is not supported: frames are
  static.
- **tasks**: name, interval, priority and the program functions they run in
  order; `plc_task_run(i)` runs task `i`.
- **sites**: source spans, the `sites` of the symbol map, referenced by
  traps and debug hooks.
- **leaves**: the variables published in the symbol map: path, type name,
  object and offset, size, flags, location, declaration site, enumeration
  values, array descriptor.

## Values and places

- Scalars ([`Scalar`]): `bool` (one byte, 0 or 1), signed and unsigned
  integers of 8 to 64 bits, bit strings, reals of 32 and 64 bits, durations
  of 64 bits. Values narrower than 32 bits are normalised after every
  operation (sign or zero extension), so integer arithmetic wraps.
- Addresses ([`Addr`]): a base (an object, a part of the current instance,
  the address stored at another address for `VAR_IN_OUT`, or an address plus
  an offset computed at run time for array elements) and a constant offset.
- Expressions carry their type and span. There is no implicit conversion:
  [`ExprKind::Convert`] with a [`ConvMode`] does every conversion (widen,
  wrap, not zero, round to nearest with ties to even and saturation,
  truncate, float, day and time of day of a date).
- Integer `/` and `MOD` take a site: division by zero traps with code 2 and
  the minimum divided by -1 with code 5 (ABI section 10). [`ExprKind::Index`]
  with a site traps with code 3 outside the bounds, [`ExprKind::Checked`]
  with code 4 outside a subrange.
- [`Intrinsic`] gives the operations that are not operators: `ABS`, `MAX`,
  `MIN`, `LIMIT`, power, shifts and rotations, `MUX`, extensible
  comparisons, `SQRT`, `LN`, `LOG`, `EXP`, and the cycle time. Their
  semantics are identical on every engine.
- Strings are zero-terminated in memory; [`StrOp`] and the string
  expressions (`StrLen`, `StrFind`, `StrCmp`, `StrChar`) are computed by
  helpers emitted only when used.

## Statements and calls

- Structured control flow only: `If`, `Switch` (inclusive ranges), `Block`,
  `Loop`, `Break` and `Continue` to a label, `Return`. The labels must be in
  scope; a WebAssembly module never needs a relooper.
- A [`Call`] evaluates its inputs left to right, resets the frame of a
  function, writes the inputs, the `VAR_IN_OUT` addresses and the copied
  arrays, structures and strings, calls, then copies the outputs.
- `Check`, `DebugSite`, `Reset`, `Copy` (`memory.copy`) complete the set.

## Generation

```rust
use ironplc_wasm_codegen::{CodegenOptions, generate};

ironplc_wasm_ir::validate(&module)?;          // a front end bug is found here
let out = generate(&module, &CodegenOptions {
    fuel: false,                             // plc_fuel, ABI-070
    debug_hooks: false,                      // plc_rt.debug_hook, ABI-032
    bounds_checks: true,                     // recorded in plc.build
    static_limit: 16 << 20,
    compiler: "my-front-end 1.0".into(),
    files: vec![("main.st".into(), source)], // hashed into plc.build
})?;
// out.wasm: the validated module; out.symbols: its symbol map
```

The module uses only the WebAssembly features of ABI-001 and is validated
before it is returned. It runs on wasmtime, wasmi 2.0.0 and browsers; it
contains no `select` instruction.

## Stability

The IR follows the version of IronPLC and may change between versions. A front end should pin the version and run
`validate` on every module it builds.
