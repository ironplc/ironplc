# Library Functions Over Compiler Intrinsics

status: proposed
date: 2026-08-08

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

* **No IronPLC dialect** ([ADR-0036](0036-no-ironplc-dialect.md)) — an
  always-on vendor name would create a configuration no real toolchain accepts
* **Scalability** — vendor runtimes define hundreds of functions; per-function
  compiler tables and flags do not scale, libraries-as-data do
* **Auditability and provenance** — library content carries manifest
  `references` and the clean-room authoring record
  ([authoring policy](../steering/compatibility-library-authoring.md));
  compiler tables carry neither
* **Wire-format stability** — every `BUILTIN` func_id is a permanent
  compiler/VM ABI commitment pinned by wire-format tests; ST bodies are not
* **Behavioral fidelity** — some semantics (hardware access, IEEE-754
  operations like floating truncation/modulo) cannot be expressed in IEC
  61131-3 source and require a native implementation
* **Simplicity of contribution** — adding an ST function to a library touches
  no Rust; adding an intrinsic touches the analyzer, codegen, VM,
  disassembler, and wire-format tests

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
   may the implementation be native. For a *vendor* name, the native
   implementation is an **unnamed VM builtin** reached exclusively through the
   library's manifest binding (see
   [Compatibility Library Format](../design/compatibility-library-format.md));
   the builtin adds no name to any scope, so the library remains the only way
   to call it. For a *standard* name, the compiler seeds the name and lowers
   to the builtin directly — this is the only case that adds to the compiler's
   function tables.
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
* Bad, because "cannot be expressed in IEC 61131-3 source" requires judgment;
  the review checklist below is the tiebreaker

### Confirmation

For each new callable name in a PR, review confirms:

1. Vendor name → it appears only under `compiler/sources/resources/libs/`,
   with manifest `references`; no new entry in `stdlib_function.rs`, no new
   `--allow-*` flag.
2. ST-expressible → the body is ST; no new func_id exists for it.
3. Native → the PR demonstrates inexpressibility (what IEC 61131-3 construct
   is missing), and for vendor names the func_id is reachable only via a
   manifest binding (no compiler-seeded name resolves to it).

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
