# External IEC 61131-3 Test Corpora

**Status:** Research result
**Date:** 2026-08-23

A catalog of third-party IEC 61131-3 code that can serve as an independent
oracle for IronPLC defects — what exists, what it costs to use, and what
currently blocks each one.

How a defect found this way is allowed to become a change in this repository is
governed by
[External Corpus Defect Sourcing](steering/external-corpus-defect-sourcing.md).
Read that first. Nothing in this document authorises bringing external source
into the repository.

## Why

Codegen and runtime defects are uniquely corrosive. A wrong answer from
`ironplcvm` is indistinguishable, to the person running it, from a wrong answer
in their own program — so each one spends trust that is expensive to earn back.

Our own tests are written by the people who wrote the code under test, so they
inherit its blind spots. Test suites written by other people, for other
runtimes, carrying their own expected results, do not. That independence is the
whole value.

## Catalog

Counts were measured at the commits below. The commands that produce them are
in [Reproducing the counts](#reproducing-the-counts). "Assertions" counts
assert-family call sites; it is a size indicator, not an exact test count.

### Tier 1 — corpora carrying machine-checkable expected values

| Corpus | Repository @ commit | License | Format | Size | Expectations live in |
|---|---|---|---|---|---|
| **RuSTy lit suite** | `PLC-lang/rusty` @ `29d6e20` | LGPL-3.0 | plain `.st` | 533 files; **451 carry expected stdout**; 445 use `printf` | inline FileCheck `// CHECK:` directives |
| **RuSTy stdlib** | same commit | LGPL-3.0 | `.st` + Rust | 25 ST modules implementing IEC standard functions; 20 Rust test files | Rust `assert_eq!` |
| **struckig** | `stefanbesler/struckig` @ `115790a` | **GPL-3.0** (dual-licensed; commercial terms on request) | `.TcPOU` | 63 POUs (24 under `tests/`); 217 tests; **~2 033 assertions** | inline ST asserts; regenerable from the C++/Python `ruckig` reference |
| **TcUnit-Verifier** | `tcunit/TcUnit` @ `d447f95` | MIT | `.TcPOU` | 29 POUs (28 test); 270 tests; **562 assertions** | inline ST asserts |
| **fisothemes Dynamic Collections** | `fisothemes/TwinCat-Dynamic-Collections` @ `b8be8de` | MIT | `.TcPOU` | 59 POUs; 88 tests; **338 assertions**, typed `AssertEquals_*` | inline ST asserts |
| **TcUnit ExampleProjects** | `tcunit/ExampleProjects` @ `c2e644c` | ⚠ **none stated** | `.TcPOU` | 39 POUs; 22 tests; **120 assertions** | inline ST asserts |
| **SLAC LCLS** | `pcdshub/lcls-twincat-general` @ `e1acf72` | BSD-3-Clause (+ DOE rider) | `.TcPOU` | 43 POUs (7 test); 20 tests; **78 assertions** | inline ST asserts |
| **TcBlack fixtures** | `Roald87/TcBlack` @ `1022259` | MIT | `.TcPOU` | 16 files as input/expected pairs | paired golden files |

`tcunit/ExampleProjects` carries no `LICENSE` file and states no license in its
README, despite sitting in the otherwise MIT-licensed `tcunit` organisation.
Treat it as unlicensed — all rights reserved — until upstream clarifies.

RuSTy declares `license = "LGPL-3.0"` in its workspace manifest and ships both
`COPYING` and `COPYING.LESSER`, as an LGPL distribution does. Our own README
already notes that RuSTy's licensing is awkward for industrial use; that is a
reason to keep its corpus at arm's length, not a reason to avoid learning from
its behaviour.

The TcBlack fixtures are formatter input/expected pairs, which is structurally
the same shape as our `plc2plc` round-trip goldens. They are MIT, so they are
the one corpus here that could be vendored outright if in-tree fixtures are
ever wanted.

### Tier 2 — corpora without expected values (parse, render and compile pressure)

| Corpus | Repository @ commit | License | Content |
|---|---|---|---|
| **chathhorn/structured-text** | `chathhorn/structured-text` @ `19b3c86` | BSD-3 | 94 `.st`, 73 of them samples — including the **IEC 61131-3 Annex F** worked examples |
| **matiec** | `beremiz/matiec` @ `7949c0b` | GPL-2 | 19 Annex F programs in ST and IL; 12 golden identifier/syntax parse cases |
| **Stefan Henneken blog samples** | `stefanhenneken/Blog-*` (~114 repos) | BSD-2 | small TwinCAT/CODESYS projects on SOLID, state and observer patterns, abstract FB vs. interface — the best available OO stress set |
| **UniTest** | `tkucic/UniTest` @ `7d8bbb1` | MIT | a test framework in **pure IEC ST**, vendor-agnostic, delivered as PLCopen XML |
| **WengerAG/structured-text-utilities** @ `5afc27c`; **Intecre/twincat-utils** @ `585f495`; **fisothemes** Hashing @ `95e2270` and String-Kit @ `cd4ffdd` | | MIT | library ST with no tests. The hashing library implements MurmurHash3, CRC32 and FNV1a, all of which have published external test vectors — expectations can be obtained without touching upstream tests |
| **blark**; **iec-checker** | `klauer/blark` @ `7834929`; `jubnzv/iec-checker` @ `d3e5dae` | GPL-2 / LGPL-3 | 21 and 54 source files respectively; parser corpora |

### Negative results

Recorded so the search is not repeated.

- **There is no public IEC 61131-3 conformance suite.** PLCopen's compliance
  procedure is not open. CERN's PLCverif operates on proprietary CERN and GSI
  control programs that are explicitly not redistributable.
- **OSCAT ships no tests.** The Eclipse OSCAT organisation holds documentation
  repositories and an archived library dump; the libraries themselves carry no
  test suite.
- matiec's `lib/test_iec_std_lib.c`, which the name suggests would be a
  standard-library conformance test, is an empty stub whose `main` returns 0.
- **TcOpen** is large (510 POUs) but drives its tests from .NET rather than
  from ST assertions, so there is nothing for us to run.
- GitHub topic tags are not a useful discovery channel here — the `tcunit`
  topic lists four repositories. The curated lists that do work are
  `myutzy/awesome-structured-text` and `benhar-dev/twincat-resources`.
- The CODESYS-side equivalent of TcUnit (**CfUnit**, MIT) lives on CODESYS
  Forge under Subversion, not GitHub.

## What blocks us

The corpora are real. Most of them do not run on IronPLC today.

### A. Object orientation is the dominant blocker

Every TcUnit-derived suite is structurally a function block extending a
framework base class, with one method per test. IronPLC parses that shape and
resolves inherited fields, but:

- codegen returns `Diagnostic::todo` for method calls and **never compiles
  method bodies at all** — `compile_stmt.rs:438` for `StmtKind::MethodCall`,
  and `compile_expr.rs:854` and `:875` for `SymbolicVariableKind::SelfRef`,
  both under `compiler/codegen/src/`;
- `IMPLEMENTS`, `ABSTRACT`, `INTERFACE`, `THIS^` and `SUPER^` are P9999 in the
  analyzer (`compiler/analyzer/src/rule_unsupported_extension.rs`);
- `PROPERTY` does not tokenize at all.

`specs/plans/2026-08-12-oop-method-declarations-static-dispatch.md` (ADR-0041
Phase 1) is unstarted and gates this entire family.

### A′. TwinCAT method elements are silently dropped

`compiler/sources/src/parsers/twincat_parser.rs` reads only the declaration and
the ST implementation of a `.TcPOU`. Its `Method`, `Property`, `Get`, `Set` and
`Action` child elements are never visited — **dropped without a diagnostic**.
A TwinCAT POU with methods compiles today to something quieter and smaller than
what the author wrote.

Tracked as [#1418](https://github.com/ironplc/ironplc/issues/1418). It is
listed here because it has to be fixed before any TwinCAT corpus result means
anything: until the source reader keeps methods, a green run against a TcUnit
suite would be measuring a program with most of its behaviour removed.

### B. RuSTy's suite needs two small things

445 of its 533 files observe results through `printf`, and the entry point is a
function named `main` rather than a `PROGRAM` inside a `CONFIGURATION`. IronPLC
has neither a `printf` equivalent nor an entry-point wrapper. The nearest
observation channel, `ironplcvm --dump-vars`, does not render STRING contents.

### C. There is no file-driven execution harness

Round-trip tests read `.st` files; execution tests take inline `&str` literals.
Nothing reads an `.st` file, compiles it, runs N scans and asserts *named*
variable values in process. No test anywhere walks a directory — every fixture
is enumerated by hand as an `#[case]` or a `mod` line.

Under the separate-repository rule this harness is built **there**, not here.

### D. PLCopen XML delivery is lossy

`compiler/sources/src/xml/position.rs:50` pins `SUPPORTED_NAMESPACE` to
`tc6_0201` — UniTest is `tc6_0200`, and any other namespace is P6008.
`transform_variable` discards initial values outright
(`transform.rs:604`, still carrying its `TODO`) and drops STRING lengths, and
inline anonymous array, enum or struct types on a variable are `todo()`
(`transform.rs:380` onward). An XML-delivered library is unlikely to compile
unmodified.

### What is runnable today

Procedural ST: functions, function blocks, structs, arrays, strings, time and
date types, `REF_TO` / `POINTER TO` / `ADR`.

Of RuSTy's 533 lit files, **283 use neither OO nor pointer features**, and a
further 146 use pointers or references that IronPLC does support. The plain
subset concentrates in initialisation (39), enumerations (26), control flow
(19), stdlib overflow behaviour (14), functions (12) and addressing (9).

## Order of attack

0. **Differential sweep against RuSTy.** Feed *our own* ST programs to both
   implementations and compare. Nothing of theirs is read or copied — only
   behaviour is observed, and behaviour is not copyrightable. Highest value per
   unit of risk, and independent of every question above.
1. **RuSTy lit, plain subset (283 files).** The largest expected-value corpus
   reachable without new language features, and IronPLC already ships a `rusty`
   dialect built to match this compiler. Needs blocker B and the harness from
   blocker C.
2. **RuSTy lit, pointer and reference subset (146 files).** No new language
   features beyond the harness.
3. **TcUnit ExampleProjects (120 assertions).** The cheapest TwinCAT corpus —
   methods, inheritance and arrays only, no pointers, interfaces or properties.
   Gated on A′, on ADR-0041 Phase 1, and on a clean-room test-framework shim
   authored from the framework's public API surface. Its licensing must be
   resolved first.
4. **TcUnit-Verifier (562), fisothemes Collections (338), SLAC LCLS (78),
   struckig (~2 033).** Progressively deeper: generic `ANY` parameters, dynamic
   allocation, constructors, `VAR_IN_OUT`.
5. **TcBlack fixture pairs (16).** Independent of everything above; maps
   directly onto `plc2plc` round-trip testing.

## Reproducing the counts

Clone at the commit recorded in the table, into a working directory outside
this repository, then:

```sh
# .TcPOU corpora — POU count, test count, assertion count
find "$corpus" -name '*.TcPOU' | wc -l
grep -rho "TEST('" "$corpus" --include=*.TcPOU | wc -l
grep -rhoiE 'assert[a-z_]*[[:space:]]*\(' "$corpus" --include=*.TcPOU | wc -l

# RuSTy lit suite — total, expectation-carrying, printf-dependent
find "$rusty/tests/lit" -name '*.st' | wc -l
grep -rl '// CHECK' "$rusty/tests/lit" --include=*.st | wc -l
grep -rl 'printf' "$rusty/tests/lit" --include=*.st | wc -l

# RuSTy lit — files needing OO or pointer support (283 plain = 533 - this)
grep -rlE 'METHOD|INTERFACE|PROPERTY|EXTENDS|IMPLEMENTS|THIS\^|SUPER\^|REF_TO|POINTER TO|REFERENCE TO|ADR\(|REF\(' \
  "$rusty/tests/lit" --include=*.st | sort -u | wc -l
```

## See also

- [External Corpus Defect Sourcing](steering/external-corpus-defect-sourcing.md)
  — the rules for turning a finding into a change here.
- [Brainstorm: VM Integration & End-to-End Testing Strategy](brainstorm-vm-testing.md)
  — its "Idea 1.2: Golden file tests for the VM CLI" is the in-tree harness
  this catalog would feed.
