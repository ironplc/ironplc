# ADR Status Audit Plan

Audit every ADR still at `status: proposed` against the implementation, per
[issue #1586](https://github.com/ironplc/ironplc/issues/1586). Each ADR lands in
one of three buckets: shipped and enforced (`accepted`), genuinely not built
(stays `proposed`, with what actually landed recorded), or partly built / built
differently (the expensive bucket — the record is corrected).

## Problem

`specs/steering/development-standards.md` defines four ADR statuses and makes the
pull request that lands the work responsible for flipping `proposed` to
`accepted`. The convention was applied only to the ADRs #1496 named. Thirty-three
ADRs are still `proposed` and nobody has checked which of those are accurate.

A `proposed` status on shipped work tells a reader the decision is speculative
when the compiler already enforces it. Worse, ADR-0011 sat for six months while a
design document cited it for the opposite of what it said, and ADR-0042
instructed contributors to build a mechanism that had been rejected. Nothing has
checked the rest for the same failure.

## Prefactor

ADR-0025 is still in the pre-front-matter format (`# ADR-0025: Title` with a
`## Status` section). It is invisible to any audit that reads `status:` front
matter — which is why the issue's census counts 46 ADRs when there are 52.
Converting it first makes the corpus uniform, so the audit reads one shape.

The convention then gets a gate. `specs/justfile` already polices ADR numbers and
plan citations; it does not police the front matter that the status convention is
written in. A new `adr-front-matter` recipe fails when an ADR is missing
`status:`/`date:` directly under its H1, or carries a status outside the four
documented values. That keeps ADR-0025's defect from recurring and makes the
next audit a matter of reading statuses rather than reconstructing them.

## Method

For each ADR: read the Decision Outcome, check it against the code, check whether
the Confirmation criteria are satisfiable, and check what cites it for what.
Status is the last thing decided, not the first.

## Findings

### Bucket 1 — shipped and enforced → `accepted`

| ADR | Evidence in the tree |
|---|---|
| 0000 stack-based VM | `compiler/vm/` is the execution model; `compiler/benchmarks/` measures it |
| 0001 two-width arithmetic | `ADD_I32`/`ADD_I64` op-classes plus `OP_CLASS_TRUNC` |
| 0003 standard FBs as intrinsics | `compiler/vm/src/intrinsic.rs` behind `FB_CALL` |
| 0009 typestate lifecycle | `Vm` → `VmReady` → `VmRunning`, setup transitions consume `self` |
| 0013 `Expr` wrapper | `Expr { kind, resolved_type }` in `compiler/dsl/src/textual.rs` |
| 0014 V-code categories | `VmError`, `Trap::v_code`/`exit_code`, generated from `problem-codes.csv` |
| 0016 string encoding | `CharWidth::Narrow` (Latin-1) / `Wide` (UTF-16LE) |
| 0017 unified data region | `data_region_bytes`, `num_temp_bufs`, `max_temp_buf_bytes` in the header |
| 0021 TIME/LTIME | `IntermediateType::Time { size }`, millisecond literals |
| 0023 array bounds | `LOAD_ARRAY`/`STORE_ARRAY` + `Trap::ArrayIndexOutOfBounds` |
| 0026 structure layout | 8-byte slot per field in `compile_struct.rs` |
| 0027 compile-time field offsets | `StructVarInfo`, offsets folded at compile time |
| 0028 literal inference | `compiler/analyzer/src/type_compat.rs` |
| 0029 integer widening | same, cited from the rule modules |
| 0031 expanded widening | same, plus `--allow-cross-family-widening` |
| 0032 CI gating | `pr-ci` / `production` environments; `partial_upload_release_artifacts.yaml` cites the ADR |
| 0034 string via operand typing | one `STR_*` family, `char_width` in the data |
| 0035 6-byte string header | `STRING_HEADER_BYTES = 6` is the single definition |
| 0036 no IronPLC dialect | `Dialect` presets, strict Edition 2 default |
| 0039 no warnings | every diagnostic renders `Severity::Error` / `DiagnosticSeverity::ERROR` |
| 0040 policy-phase diagnostics | `plc_parser` names `CompilerOptions` zero times; demotion and token rules exist |
| 0041 staged dispatch | Phase 1 only, exactly as the ADR scopes itself |
| 0046 flat variable table | cited from `compile_fn.rs`, which relies on its stale-value consequence |

### Bucket 2 — genuinely not built → stays `proposed`

- **ADR-0006** (verification requirement, #1582) — `verify_stack_balance` runs in
  codegen, on the compiler's own output. Nothing calls it at VM load, and the VM
  has no notion of verified-or-signed bytecode. Record it.
- **ADR-0007** (dual signatures, #1583) — of the three cryptographic elements,
  only the per-file source hashes landed. `content_hash`, `debug_hash` and
  `layout_hash` are header fields that are always zero; `sig_section_offset` and
  `debug_sig_offset` are reserved slots nothing writes or checks. Record it.
- **ADR-0010** (no-std VM) — already annotated by #1573; left alone.

### Bucket 3 — partly built, or built differently

- **ADR-0005 (safety-first)** — the principle holds and is cited across the design
  documents, so it is `accepted`. But its "How this principle was applied" table
  has two rows describing mechanisms that do not exist: STRING/WSTRING via
  "distinct BUILTIN func_id ranges" (ADR-0034 replaced this with one `STR_*`
  family) and "dedicated TIME_ADD/TIME_SUB" (ADR-0021 made TIME arithmetic
  ordinary integer arithmetic, and `bytecode-instruction-set.md` says so). Its
  opcode-budget premise — "157 of 256 opcodes used, 99 slots available" — is the
  pre-ADR-0033 encoding; the census today is 63 of 64 op-classes with one free.
  Postscript, not rewrite: the decision is unchanged, the illustrations aged.
- **ADR-0008 (unified BUILTIN)** — the opcode landed and carries the numeric
  standard library, so it is `accepted`. Its Scope claims string functions
  (LEN, CONCAT, LEFT, …) dispatch through BUILTIN by func_id range; they do not —
  `LEN_STR`, `FIND_STR` and `CONCAT_STR` are their own op-classes. Confirmation
  items 1–3 test a STRING/WSTRING func_id split that never existed. Amend.
- **ADR-0019 (debug type tags)** — Option C shipped, so `accepted`. The tag table
  is stale: the ADR assigns 20 = TIME_OF_DAY and 21 = DATE_AND_TIME, the code
  assigns 20 = LDATE, 21 = TIME_OF_DAY, and 25–27 to STRUCT/ARRAY/FB_INSTANCE.
  ADR-0021 even said "ADR-0019's table should be updated accordingly" and it never
  was. Amend the table to match, and name the code as its source of truth.
- **ADR-0020 (test strategy)** — the three categories landed, so `accepted`. The
  mechanism for the backwards-compatibility category did not: the ADR requires
  hardcoded hex in `compile_*.rs` and forbids symbolic opcode constants; what was
  built is symbolic `bc::*` helpers in `compile_*.rs` with the wire format pinned
  in one canonical `wire_format.rs`. Amend to the mechanism that was built — the
  ADR-0042 shape. (The ADR's rule would have made ADR-0033's renumbering
  impossible to land.)
- **ADR-0022 (Edition 3 flag)** — the decision (gate Edition 3 constructs behind
  an off-by-default flag) is in force, so `accepted`; the spelling is not what
  was built. There is no `--std=iec-61131-3:2013`, no
  `CompilerOptions::allow_iec_61131_3_2013`, and no `rule_token_no_std_2013.rs`.
  LTIME is gated by `--allow-long-time-types` and bundled into the
  `iec61131-3-ed3` preset. `specs/design/time-literals.md` REQ-TL-003 still cites
  the field that does not exist — the ADR-0011 failure, live. Amend both.
- **ADR-0012 (accept vendor files as-is)** — stays `proposed`. TwinCAT landed
  (`.TcPOU`, `.TcGVL`, `.TcDUT`, and `.TcIO` beyond the ADR's list); Siemens SCL
  did not — `.scl` is not a recognized extension in `FileType::from_path`. And
  dialect is chosen by `--dialect`, not detected per file extension as the
  "Dialect detection strategy" section describes. Record what landed.
- **ADR-0018 (interactive examples)** — stays `proposed`. Twenty-four of the
  twenty-five elementary type pages carry a `playground-with-program` example;
  `wstring.rst` has static code blocks only. Record the gap rather than flipping.

## Work

1. Prefactor: convert ADR-0025 to front matter; add the `adr-front-matter` gate
   to `specs/justfile` and wire it into that justfile's default recipe.
2. Flip the bucket-1 ADRs to `accepted`.
3. Amend the bucket-3 ADRs (0005, 0008, 0019, 0020, 0022) with `amended:` lines,
   and flip them to `accepted`.
4. Add Implementation Status sections to 0006, 0007, 0012 and 0018; leave them
   `proposed`.
5. Fix `specs/design/time-literals.md` REQ-TL-003, which cites a field that does
   not exist.
6. Delete this plan; run `cd specs && just` and `cd compiler && just`.

## Not delivered

The WSTRING documentation gap (ADR-0018) and the `.scl` gap (ADR-0012) are
recorded in the ADRs, not fixed here — both are product work, not record
correction. They need issues before this plan is deleted.
