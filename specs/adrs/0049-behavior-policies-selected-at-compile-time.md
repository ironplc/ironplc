# Behavior Policies Are Selected at Compile Time and Encoded in the Bytecode

status: proposed
date: 2026-09-03
supersedes: ADR-0002

## Context and Problem Statement

IEC 61131-3 leaves a number of runtime behaviors implementer-specific. Integer
overflow is one ([ADR-0002](0002-bytecode-overflow-behavior.md)); string-to-number
conversion is another, and it is the one that forced the question. Real
implementations disagree, and each disagreement is documented (or, worse,
documented as undefined):

| Implementation | `STRING_TO_INT('12abc')` | `STRING_TO_UDINT('4294967296')` | Signals an error? |
|---|---|---|---|
| CODESYS / TwinCAT (documented) | Must be a valid literal of the target type, "otherwise the result will be 0" | "Depends on the processor type and is therefore undefined" | No |
| CODESYS / TwinCAT (observed) | 12 — parsing stops at the first invalid character | Not documented | No |
| Siemens S7-1200/1500 `STRG_VAL` | Conversion "is interrupted by invalid characters" | Error | `ENO` false |
| Rockwell Logix `STOD` | Skips leading non-numeric characters, converts the first digit run | Not verified | Status flags |
| matiec (OpenPLC, Beremiz) | Ignores non-digits anywhere in the string | Wraps | No |
| RuSTy | 0 — trailing characters reject the whole string | 0 | No; "never fault" is a documented contract |
| IronPLC before this decision | 0 | 0 for 32-bit targets, **wraps** for 8- and 16-bit targets | No |

Issue [#1592](https://github.com/ironplc/ironplc/issues/1592) is the symptom:
`STRING_TO_UDINT` returned 0 for the entire upper half of the UDINT range, and
nothing in the language, the bytecode, or the documentation said what the
function does when the input is not convertible. The failure mode for a valid
literal that did not fit was identical to the failure mode for garbage, and
neither was written down.

IronPLC's promise is compatibility with real targets, so it cannot pick one
behavior and call it done; a TwinCAT project that relies on prefix parsing must
keep working when IronPLC compiles it. But IronPLC also cannot emulate
"undefined" or "depends on the processor", and it must never produce a silently
wrong number. So two questions need a standing answer, not a per-function one:

1. **Where is a behavior selected** — at compile time, at VM startup, or when the
   VM itself is built?
2. **What does IronPLC do where the vendor's behavior is undefined?**

ADR-0002 answered the first question for arithmetic overflow with a policy on
the VM instance, set at startup, applied uniformly to every instruction. That
decision was never implemented: the instruction-set specification records that
the VM is unconditionally wrapping and that no bytecode encodes a policy. This
ADR supersedes it.

## Decision Drivers

* **No undefined behavior.** Every operation has one documented, deterministic
  result for every input under every selectable configuration. "Depends on the
  target" is not an acceptable answer even where a vendor gives it.
* **Compatibility is per file.** A file belongs to exactly one dialect
  ([ADR-0012](0012-accept-vendor-dialect-files-as-is.md)), and a composition may
  mix user source with vendor library bodies compiled from ST
  ([ADR-0042](0042-library-functions-over-compiler-intrinsics.md)). A TwinCAT
  library body and a strict user file in the same program legitimately need
  different behavior for the same function name.
* **Behavior must be readable from the artifact.** A program's semantics should
  follow from its source plus its compiler options. A `.iplc` that produces one
  answer in the playground and another in production because the runtime was
  started differently is defined behavior in the letter and undefined in the
  spirit.
* **Safety-first** ([ADR-0005](0005-safety-first-design-principle.md)): encode
  invariants where the verifier and the VM can see them, in the opcode stream,
  rather than in configuration the bytecode cannot express.
* **Strict defaults, real presets** ([ADR-0036](0036-no-ironplc-dialect.md),
  [ADR-0038](0038-no-restrictions-on-flag-combinations.md)): the default
  configuration is the standard; every non-default configuration corresponds to
  a real target; individual selections compose freely.
* **The embedded VM is small** ([ADR-0010](0010-no-std-vm-for-embedded-targets.md)):
  a target that never runs vendor programs should not pay flash for vendor
  behaviors.
* **Wire-format stability**: every opcode and builtin func_id is a permanent
  compiler/VM commitment, so the encoding must not grow without bound.

## Considered Options

* **A — Compile time, encoded in the bytecode.** Behavior is selected by
  compiler options (overlaid on dialect presets and set by vendor project
  discovery), and codegen emits a distinct opcode or builtin func_id per
  selected behavior. The VM has no behavior configuration.
* **B — Runtime, a policy on the VM instance** (ADR-0002's decision). One
  encoding; the VM applies a policy chosen at startup, uniformly.
* **C — Build time, a policy per VM build.** One encoding; the VM crate is built
  with a feature selecting the behavior, producing one VM binary per behavior.
* **D — No selection: the strict behavior only.** Vendor behavior is available
  only under a different function name.

## Decision Outcome

Chosen option: **A — compile time, encoded in the bytecode.**

The rule, in the order it is applied when a behavior turns out to be
implementer-specific:

1. **Name it.** Each independently selectable behavior is a *behavior policy*: a
   named choice among enumerated, documented, deterministic alternatives for
   one semantic of a standard operation. A policy is not an
   [extension](../steering/glossary.md#extension) — extensions are syntax the
   parser recognizes; a policy is what an operation *does*. It lives on the
   vendor axis of the glossary ("something you emulate the runtime of"), and the
   glossary gains the term when the first policy lands.
2. **Select it at compile time.** A policy is a field on `CompilerOptions`,
   overlaid on dialect presets exactly as `--allow-*` flags are, settable per
   file, and also set by vendor project discovery (the channel that already
   activates a vendor's implicit libraries from a `.plcproj`). Policies compose
   freely (ADR-0038); a preset names the combination a real target uses.
3. **Encode it in the bytecode.** Codegen emits a distinct opcode or builtin
   func_id per selected alternative, following the instruction set's own
   encoding rule 3 (a sub-opcode in the operand stream, or a func_id, names the
   family member). The VM dispatches on what it decodes; it holds no policy
   state and exposes no policy setting. The disassembler shows the alternative.
4. **Default to the standard, and to a trap where the standard says "error".**
   Where IEC 61131-3 defines the result, the default is that result. Where the
   standard calls a case an error and leaves the handling to the implementer,
   the default is a trap with a documented `V4xxx` problem code. A silent
   substitute value (0, wrap, saturate) may be the default only where the
   surveyed targets agree on it and document it; wrapping integer arithmetic
   is the one such case today, and a new policy does not inherit that
   exception by analogy.
5. **Emulate what is documented; trap what is not.** A vendor preset selects the
   vendor's *documented* behavior. Where the vendor documents the result as
   undefined or processor-dependent, IronPLC traps, and the documentation for
   that preset states the divergence. IronPLC does not offer an alternative
   that no real target documents.
6. **A VM build may decline to carry an alternative.** Because the alternative
   is a distinct opcode or func_id, a VM built without it traps
   `V9007 InvalidBuiltinFunction` (or `V9003 InvalidInstruction`) exactly as it
   does for any unknown encoding. Build-time subsetting is a size optimization
   layered on this decision, never a selection mechanism.

### Consequences

* Good, because a `.iplc` file means the same thing on every VM that runs it:
  the desktop VM, the playground, and an embedded target agree, and the
  disassembler can show which behavior was compiled in.
* Good, because a composition can mix behaviors per file, which is what
  compiling a vendor library body next to strict user code requires.
* Good, because the bytecode verifier and the wire-format tests see every
  alternative as a distinct, pinned encoding (ADR-0005).
* Good, because there is no VM configuration surface to plumb through the CLI,
  the debugger launch path, the playground, and every embedder.
* Good, because the trap default means a ported program that depended on a
  vendor behavior fails loudly at the first affected input rather than
  silently computing a different number.
* Bad, because each alternative costs an opcode or func_id. Rule 3 of the
  instruction-set encoding bounds this: a family of alternatives shares one
  op-class slot or one func_id block, and the failure alternative can often be
  expressed by what codegen emits *after* the operation rather than by a
  second copy of it.
* Bad, because a compiled program cannot have its behavior changed at
  deployment. That was ADR-0002's stated benefit; this ADR judges it a
  liability (see *Runtime* below).
* Bad, because the trap default makes an ordinary bad input (a mistyped HMI
  field parsed by `STRING_TO_INT`) halt the program unless the program checks
  first. Every policy whose default is a trap therefore ships with a
  non-trapping way to test the input, provided as a library function
  (ADR-0042) rather than as another intrinsic.
* Neutral, because arithmetic overflow keeps its implemented behavior: wrapping
  for `ADD_*`, `SUB_*`, `MUL_*`, `NEG_*`, and `TRUNC_*`. ADR-0002's own survey
  shows wrapping is what CODESYS, TwinCAT, and Allen-Bradley do, which is the
  agreement rule 4 requires for a substitute default. If a checked or
  saturating alternative is ever wanted, it arrives as opcodes selected at
  compile time under this ADR, not as a VM setting.

### Confirmation

1. The same source compiled under two policy selections produces bytecode that
   differs only in the encoded alternative, and the disassembler names each.
2. The same `.iplc` produces identical results on every VM build that carries
   the encoded alternative; a build that omits it traps `V9007` or `V9003`.
3. `ironplc-vm`'s public API has no type or function whose name or purpose is a
   behavior policy. A grep for the policy's name in `compiler/vm/src/` finds
   only per-encoding dispatch arms.
4. Every policy has an end-to-end test per alternative, and the strict default's
   trap is tested through `execute()` with its `V4xxx` code, per the
   [VM testing design](../design/vm-testing.md).
5. `docs/` documents each policy on the page for the operation it governs, with
   the result for every alternative and the divergence from any vendor whose
   behavior is undefined.

### First application: string-to-number conversion

`STRING_TO_<numeric>` has two independent policies. Both are selected per rule
2 and encoded per rule 3; the specific func_id scheme is a design-document
matter, not an ADR matter.

| Policy | Alternatives | Default | Selected by |
|---|---|---|---|
| **Scan** — what counts as convertible | *whole*: after trimming surrounding whitespace, the entire string is a valid IEC literal of the target type; *prefix*: the longest leading run that is a valid literal, with the rest ignored | *whole* | `codesys` preset and TwinCAT project discovery select *prefix* |
| **Failure** — what happens when it is not | *trap*: `V4xxx` with the offending value; *zero*: the target type's zero | *trap* | `codesys` preset and TwinCAT project discovery select *zero* |

Under both scans the accepted literal syntax is the IEC literal syntax of the
target type: based literals (`2#`, `8#`, `16#`) and underscores for integers,
exponent notation for reals. CODESYS documents the input as "a valid literal of
the target type", and CODESYS, TwinCAT, matiec, and RuSTy all accept `16#FF`, so
this is not a policy; it is the meaning of "convertible".

Out of range is a failure, not a scan question: `STRING_TO_SINT('300')` is not
convertible to SINT, so sub-32-bit targets are range-checked before truncation
and never wrap. The *prefix* + *zero* combination reproduces TwinCAT's
documented result for a string that is "not valid in the target type". For a
value that parses but does not fit, the CODESYS documentation says the result
"depends on the processor type and is therefore undefined"; the preset yields
*zero* there too, because the failure policy is one choice, not one per cause,
and the docs for the preset state that this is IronPLC's choice, not an
emulation.

Wrapping is not offered. The only surveyed implementation that wraps is matiec,
and that is an artifact of accumulating in 64 bits and casting, not a documented
behavior.

## Pros and Cons of the Options

### A — Compile time, encoded in the bytecode

* Good, because behavior is a property of the artifact, verifiable and
  disassemblable.
* Good, because selection is per file, which per-file dialects and mixed
  compositions require.
* Good, because the VM stays configuration-free, which keeps the embedded
  target, the playground, and the debugger simple.
* Bad, because alternatives cost encoding space; bounded by the encoding rules.
* Bad, because deployment cannot change behavior without recompiling.

### B — Runtime, a policy on the VM instance (ADR-0002)

* Good, because one encoding serves every behavior and the bytecode never
  grows.
* Good, because a deployment could run existing bytecode under a stricter
  policy without recompiling.
* Bad, because behavior is uniform per VM: ADR-0002 itself lists "cannot mix
  policies within a program" as a limitation, and per-file compatibility makes
  that limitation disqualifying.
* Bad, because the artifact does not determine its own behavior; a program that
  passes in one environment can trap in another with no change to the file.
* Bad, because every embedder, the CLI, the debugger, and the playground must
  expose and plumb the setting.
* Bad, because in a non-wrap mode every arithmetic instruction gains a runtime
  branch on the policy, in the hot path ADR-0001 keeps branch-free.
* Never implemented; the instruction-set specification records the VM as
  unconditionally wrapping.

### C — Build time, a policy per VM build

* Good, because the bytecode does not grow and a build carries only the code
  for its behavior.
* Bad, for the same reasons as B: behavior is uniform per VM and not
  determined by the artifact. Nothing in the container says which build it
  needs, so a mismatch is silent unless a profile is added to the container
  header and checked at start, which is more machinery than an encoding.
* Bad, because it multiplies release artifacts and the test matrix, and the
  playground is a single WASM build.
* Its useful part survives as rule 6: subsetting an encoding-selected
  behavior out of a build is fine; selecting behavior by build is not.

### D — No selection, strict behavior only

* Good, because it is the simplest to build and document, and it can never
  produce a silently wrong number.
* Bad, because it abandons compatibility for the affected operation: a
  discovered `.plcproj` runs differently from TwinCAT, and the only remedy is
  editing vendor source, which the portability invariant forbids.
* Its useful part survives as the trap default and the companion library
  function.

## More Information

### Runtime versus compile time

ADR-0002's case for a runtime policy was that a safety deployment could run the
same compiled program in fault mode. That deployment recompiles anyway, and
compiling in the fault alternative gives it bytecode that says so. Against that
one benefit stand three costs: uniform-per-VM behavior, an artifact that does
not determine its own semantics, and a configuration surface on every runtime.
The goal is to match a real target's behavior, and the target is known when
the source is compiled.

### Relationship to other decisions

* [ADR-0001](0001-bytecode-integer-arithmetic-type-strategy.md) says the
  narrowing instructions "apply the configured overflow policy (see ADR-0002)".
  There is no configured policy; `TRUNC_*` wraps, and any alternative arrives
  as opcodes under this ADR.
* [ADR-0005](0005-safety-first-design-principle.md) is the principle this ADR
  applies: encode the invariant in the opcode.
* [ADR-0012](0012-accept-vendor-dialect-files-as-is.md) and
  [ADR-0036](0036-no-ironplc-dialect.md) supply the per-file and
  strict-default requirements.
* [ADR-0038](0038-no-restrictions-on-flag-combinations.md): policies compose
  like flags; presets express real combinations.
* [ADR-0042](0042-library-functions-over-compiler-intrinsics.md): the
  companion non-trapping validator is a library function; the policy-bearing
  standard operation remains an intrinsic because it is standard surface.
* [ADR-0010](0010-no-std-vm-for-embedded-targets.md): rule 6 is how an
  embedded build stays small.

### Sources for the survey

* Beckhoff, [STRING_TO conversions (TwinCAT 2)](https://infosys.beckhoff.com/content/1033/tcplccontrol/925582731.html)
  and [String conversion (TwinCAT 3)](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529074315.html)
* CODESYS, [Conversion: STRING, WSTRING](https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_string_to.html)
* Siemens, [STRG_VAL](https://docs.tia.siemens.cloud/r/en-us/v20/extended-instructions-s7-1200-s7-1500/string-char-s7-1200-s7-1500/strg_val-convert-character-string-to-numerical-value-s7-1200-s7-1500)
* Rockwell, [STOD](https://www.rockwellautomation.com/en-in/docs/studio-5000-logix-designer/37-00/contents-ditamap/instruction-set/ascii-conversion-instructions/string-to-dint-stod.html)
* matiec, [`iec_std_lib.h`](https://github.com/thiagoralves/OpenPLC_v3/blob/master/utils/matiec_src/lib/C/iec_std_lib.h)
* RuSTy, [`string_to_conversions.rs`](https://github.com/PLC-lang/rusty/blob/master/libs/stdlib/src/string_to_conversions.rs)

The commercial rows are taken from the vendors' own documentation pages as
excerpted in search results; the open-source rows are from reading the source.
