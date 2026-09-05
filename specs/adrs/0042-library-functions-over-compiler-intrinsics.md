# Library Functions Over Compiler Intrinsics

status: accepted
date: 2026-08-08
amended: 2026-08-31 (rule 3 corrected to the mechanism that was built; accepted)

## Context and Problem Statement

IronPLC now has two ways to make a callable function name resolve:

1. **Compiler-provided (intrinsic).** The compiler seeds the name into the
   function environment (`analyzer/src/intermediates/stdlib_function.rs`) and
   codegen lowers calls to a `BUILTIN func_id` executed natively by the VM
   ([ADR-0008](0008-unified-builtin-opcode.md)). The name exists in every
   compilation (or behind an `--allow-*` flag), with no on-disk artifact.
2. **Library-provided.** A [compatibility library](../design/compatibility-libraries.md)
   — on-disk data (manifest + `.st` declarations) — declares the name under its
   exact vendor spelling. The name resolves only when the library is activated
   (project reference or `--library`), and the body is ordinary IEC 61131-3
   source compiled like user code.

Recent contributions added vendor functions (Beckhoff `Tc2_Math`'s `LTRUNC`,
`LMOD`, `MODABS`; `BOOL_TO_STRING`; `Tc2_Utilities`' `LREAL_TO_FMTSTR`) as
compiler intrinsics behind new per-family `--allow-*` flags. Review rejected
that shape, but the criteria were implicit. When someone proposes a new
function, which mechanism must it use?

## Decision Drivers

* **Compatibility** — the library mechanism's promise is that a name is in
  scope exactly when the corresponding real-world library is activated, so
  code that compiles in IronPLC also compiles in the environment it targets.
  An always-on vendor name in the compiler breaks that promise in both
  directions. (Dialect — syntax, [ADR-0036](0036-no-ironplc-dialect.md) — is
  a separate axis: IronPLC has no dialect of its own, but it may well ship
  its own libraries; the manifest format already admits `vendor = "IronPLC"`.
  What is decided is that such names arrive *as libraries*, with the same
  activation model, never as unconditional compiler names.)
* **Per-vendor behavior divergence** — different vendors give the same
  function name different signatures or behavior (TwinCAT's `FLOOR` takes
  `LREAL`, unlike base IEC; rounding and edge-case conventions differ). A
  library captures each vendor's behavior in that vendor's package; a single
  compiler intrinsic would have to pick one vendor's behavior for everyone
* **Scalability** — vendor runtimes define hundreds of functions; per-function
  compiler tables and flags do not scale, libraries-as-data do
* **Auditability and provenance** — library content carries manifest
  `references` and the clean-room authoring record
  ([authoring policy](../steering/compatibility-library-authoring.md));
  compiler tables carry neither
* **Wire-format stability** — every `BUILTIN` func_id is a permanent
  compiler/VM ABI commitment pinned by wire-format tests; ST bodies are not
* **Behavioral fidelity** — some semantics cannot be expressed in IEC 61131-3
  source at all and require a native implementation; the inexpressible set is
  small and enumerable (see *What IEC 61131-3 source cannot express* below)
* **Simplicity of contribution** — adding an ST function to a library touches
  no Rust; adding an intrinsic touches the analyzer, codegen, VM,
  disassembler, and wire-format tests
* **Testability** — the trade-off runs the other way: an intrinsic is
  directly unit-testable in Rust, while a library ST body is testable only
  end-to-end (activate → compile → run in the VM). Choosing libraries
  obligates the project to keep that end-to-end harness cheap to use

## Considered Options

* Compiler intrinsics for any function a target dialect provides, gated by
  `--allow-*` flags per function family
* Library functions for everything, including the IEC 61131-3 standard surface
* Compiler intrinsics only for the IEC 61131-3 standard surface and for
  behavior that cannot be a function; libraries for everything else

## Decision Outcome

Chosen option: "Compiler intrinsics only for the IEC 61131-3 standard surface
and for behavior that cannot be a function; libraries for everything else."

The rule, in order of application to a proposed function name:

1. **Vendor-defined names are always library functions.** If the name comes
   from a vendor runtime rather than IEC 61131-3, it ships in a compatibility
   library — never in the compiler's tables, never behind an `--allow-*` flag.
   (`--allow-*` flags gate *syntax*, not callable names — see the
   [glossary](../steering/glossary.md) dialect/vendor distinction.)
2. **Expressible bodies are ST bodies.** A library function whose semantics
   can be written in IEC 61131-3 source gets an ST body in the library. This
   is the default and preferred implementation medium (it is also the
   preferred clean-room medium per the authoring policy).
3. **Native implementations require inexpressibility.** Only when the
   semantics cannot be expressed in IEC 61131-3 source — IEEE-754 operations
   with no source-level equivalent, hardware access, compile-time knowledge —
   may the implementation be native. For a *standard* name, the compiler seeds
   the name and lowers to the builtin directly. For a *vendor* name, the vendor
   spelling stays an ST body in the library; the native capability it needs is
   reached by calling a **compiler intrinsic in the reserved `__` namespace**
   (`__TRUNC`, `__MOD`), which the library body invokes like any other function
   — `Tc2_Math`'s `LTRUNC` is `LTRUNC := __TRUNC(IN);`. `__` names are seeded
   unconditionally (library bodies are analyzed under the *user's* options, so a
   flag gate would break every library that uses them), are visibly
   non-portable, and collide with no IEC 61131-3 or vendor name. Both cases add
   to the compiler's function tables; no vendor spelling ever does.
4. **Function-like operators are the dialect axis, not functions.** Constructs
   that parse as calls but require compiler cooperation no function could have
   (`SIZEOF`'s compile-time type knowledge, `ADR`'s address-of) are syntax
   extensions: gated by `--allow-*` flags per the
   [syntax support guide](../steering/syntax-support-guide.md), implemented in
   the compiler because they *cannot* be library functions.

Existing compiler-seeded standard functions are conformant (they are IEC
61131-3 surface) and are not migrated. The rule governs additions.

### Consequences

* Good, because the compiler's out-of-the-box surface stays exactly the IEC
  61131-3 standard plus flag-gated syntax — no de-facto IronPLC dialect
* Good, because vendor function proposals become library PRs (data + ST +
  provenance), reviewable without compiler expertise and scalable to hundreds
  of functions
* Good, because a function like `BOOL_TO_STRING` — trivially expressible as
  `IF IN THEN BOOL_TO_STRING := 'TRUE'; ...` — costs no func_id, no VM arm,
  no wire-format entry, and no permanent ABI commitment
* Good, because the scarce resources ([ADR-0033](0033-opcode-encoding-by-class-and-type.md)
  op-class slots, func_id space, compiler table entries) are spent only where
  inexpressibility forces it
* Bad, because a library ST body is interpreted bytecode, slower than a native
  builtin — acceptable until profiling shows otherwise, and revisable per
  function by adding a binding without changing the library's interface
* Bad, because library functions are harder to test than intrinsics: an
  intrinsic gets a direct Rust unit test, while a library body is exercised
  only through the full activate → compile → VM-run path — mitigated by the
  existing end-to-end test harness (`codegen/tests/it/`) and conformance
  tests that load every bundled library
* Bad, because "cannot be expressed in IEC 61131-3 source" requires judgment;
  *What IEC 61131-3 source cannot express* below bounds the categories, and
  the review checklist is the tiebreaker

### Confirmation

For each new callable name in a PR, review confirms:

1. Vendor name → it appears only under `compiler/sources/resources/libs/`,
   with manifest `references`; no new entry in `stdlib_function.rs`, no new
   `--allow-*` flag.
2. ST-expressible → the body is ST; no new func_id exists for it.
3. Native → the PR demonstrates inexpressibility (what IEC 61131-3 construct
   is missing), and for vendor names the vendor spelling is still an ST body in
   the library. The native capability enters through a `__`-prefixed intrinsic,
   never through the vendor spelling and never behind an `--allow-*` flag.

## Pros and Cons of the Options

### Compiler intrinsics per target dialect, gated by `--allow-*` flags

Each vendor function family gets a flag (e.g. `--allow-extended-math-functions`)
and entries in the compiler's function tables.

* Good, because it reuses the existing `SIZEOF` conditional-seeding pattern
  with no new mechanism
* Bad, because flags gate syntax, not runtime surface — a flag per function
  family conflates the dialect and vendor axes the glossary separates
* Bad, because it does not scale: hundreds of vendor functions means hundreds
  of table entries, func_ids, and VM arms, all permanent ABI
* Bad, because activation would not match how real projects work — a TwinCAT
  project states its libraries in `.plcproj`; it does not pass compiler flags
* Bad, because compiler tables carry no provenance record, weakening the
  clean-room story

### Library functions for everything, including the standard surface

Ship the IEC 61131-3 standard functions themselves as a bundled library.

* Good, because one mechanism serves everything
* Bad, because the standard surface is the compiler's contract
  ([ADR-0036](0036-no-ironplc-dialect.md)): it must exist in every
  compilation with no activation step, which is precisely what libraries
  are designed not to do
* Bad, because many standard functions are generic over `ANY_*` categories,
  which IEC 61131-3 source cannot declare — they would need bindings for
  nearly every entry, gaining nothing over compiler tables
* Bad, because it would churn the entire existing, tested stdlib for no
  behavioral gain

### Intrinsics only for the standard surface and the inexpressible (chosen)

* Good, for the reasons in the decision outcome
* Neutral, because the boundary case — a standard-defined function that is
  also trivially expressible — defaults to the cheaper medium (ST in a
  library, or an ST-bodied addition later if the standard surface demands
  unconditional availability); the review checklist decides
* Bad, because two mechanisms coexist and contributors must pick — mitigated
  by the four-step rule being mechanical for the common cases

## More Information

### What IEC 61131-3 source cannot express

The set of behaviors that genuinely cannot be written as an ST function body
is small and falls into four categories. Anything outside them is expressible
— "laborious to write in ST" is not inexpressible, and defaults to an ST body.

1. **Arithmetic primitives absent from the operator/stdlib surface.**
   LREAL-preserving truncation (`TRUNC` clamps to `ANY_INT`, so the fractional
   cut cannot be taken without leaving `LREAL`'s range guarantees) and
   floating modulo (`MOD` is integer-only). These two are the motivating
   builtins here (`trunc_lreal`, `fmod_lreal`) — and once they exist, the
   category is essentially closed: `MODABS`, `FRAC`, `CEIL`, `FLOOR`, and
   round-half-away variants are all math-dictated compositions of them.
2. **Bit-level reinterpretation across type families.** Reading an `LREAL`'s
   bits as a `LWORD` (or back) without numeric conversion has no ST
   construct. Needed by checksum, serialization, and float-inspection
   functions some vendor libraries provide.
3. **Compile-time knowledge.** `SIZEOF`, `ADR`/`BITADR`/`INDEXOF`, and any
   type/layout introspection require the compiler's symbol and layout tables.
   These are not functions at all — they are function-like operators on the
   dialect axis (rule 4).
4. **Runtime-service access.** Wall-clock and system time, hardware I/O,
   file/persistence access, task control — anything that reaches outside the
   program's own data (e.g. `Tc2_System`'s file and event functions). These
   need VM or runtime services: FB-shaped ones follow
   [ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md) intrinsics;
   function-shaped ones need bindings.

Borderline case worth recording: numeric-to-string *formatting* with runtime
precision (`LREAL_TO_FMTSTR`) is expressible in principle (integer math plus
the string stdlib) but numerically-faithful float formatting is subtle enough
that neither medium is chosen yet — it lands declare-only, and the follow-up
decides ST versus a formatting builtin on fidelity grounds, not convenience.

### Rejected: manifest-bound unnamed builtins

An earlier form of rule 3 above required a vendor name's native implementation to
be an *unnamed* VM builtin, reached exclusively through a binding declared in the
library's manifest — the builtin would add no name to any scope, so the library
would be the only way to call it. It reads well, and it is what this ADR
originally specified.

Review rejected it on security grounds, and it was never built. A manifest
binding makes an on-disk data file an input to code *emission*: the compiler
would emit a `BUILTIN` opcode for a func_id named by a file it does not own, and
nothing structurally guarantees the library's declared signature matches that
builtin's stack behaviour. A mismatched binding — through error or through a
tampered manifest — would corrupt the operand stack.

The `__`-namespace intrinsic in rule 3 closes that class outright. Every
`BUILTIN` emission originates from a compiler-owned table, manifests stay pure
metadata, and the intrinsic's signature is type-checked by the analyzer like any
other stdlib function, so the mismatch cannot exist. The cost is that the
compiler's name table grows by one entry per inexpressible capability rather than
zero — accepted, because `__` names are not part of the callable surface any
portable program would use, and there are two of them.

The trade-off this gives up is real: an unnamed builtin adds nothing to any
scope, whereas `__TRUNC` and `__MOD` are resolvable from user code under any
dialect. That is the price of keeping code emission compiler-owned. The rationale
also lives next to the code, on `get_compiler_intrinsic_functions()` in
`analyzer/src/intermediates/stdlib_function.rs`.

### Related decisions

* [ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md) — the same
  principle for function blocks: intrinsics are a VM dispatch detail behind
  exact type IDs, never an instruction-set or language-surface feature
* [ADR-0008](0008-unified-builtin-opcode.md) — `BUILTIN func_id` is the single
  lowering target for native functions; new functions never add opcodes
* [Compatibility Libraries](../design/compatibility-libraries.md) — the
  library mechanism, its portability promise, and the bindings future goal
  this ADR's rule 3 relies on
* Worked examples under this rule: `LTRUNC`/`LMOD` (vendor, IEEE-754
  truncation/modulo inexpressible in ST → library POUs bound to unnamed VM
  builtins); `MODABS` (vendor, math-dictated from `LMOD` → library ST body);
  `BOOL_TO_STRING` (expressible → library ST body); `LREAL_TO_FMTSTR`
  (vendor, native formatting not yet built → library declare-only, calls
  fail compile); `ADR`/`SIZEOF` (function-like operators → dialect axis,
  `--allow-*` flags)
