# Reject Variable Redeclaration Where the Compiler Has No Executable Path

status: accepted
date: 2026-09-07

## Context and Problem Statement

IEC 61131-3 code can declare the same variable name in two scopes, one
enclosing or inheriting from the other. IronPLC's documentation and analyzer
disagreed about which of those pairs are legal, and the compiler's back end
disagreed with both. Issue
[#1562](https://github.com/ironplc/ironplc/issues/1562) surfaced one pair:
the explanation page on object orientation called a derived function block's
redeclaration of an inherited field legal hiding, while `P4044` rejects it on
every dialect, including strict Edition 3.

Surveying every scope pair the compiler accepts gave this picture:

| Inner declaration | Outer declaration | Analyzer | Codegen and VM |
|---|---|---|---|
| Derived FB field | Base FB field via `EXTENDS` | Rejected (`P4044`) | Inherited fields are never laid out; a derived body that names one fails with `P4007` |
| Program local | Global variable | Accepted | **Silently wrong**: the global's initial value lands in the local's slot; a function or function block that reads the global fails with `P4007` |
| Function local | Global variable | Accepted | Correct |
| Function block field | Global variable | Accepted | Correct |
| Method local or parameter | Function block field | Accepted | Correct, end-to-end tested |

The codegen behaviour follows from one flat name-to-slot table: assigning a
program's locals into the same table the globals were assigned to overwrites
the global's entry, and the later passes that rebuild a function's view of the
globals filter that table by slot index, so the overwritten global is gone.

Vendor behaviour for the first pair is settled. Beckhoff's TwinCAT
documentation states that a derived function block must not declare variables
with the same names as its base, and the compiler (shared with CODESYS)
reports `C0097`; the one exception is a `VAR_TEMP` in the base, which a
derived block may redeclare. No compiler known to the project accepts the
redeclaration. For the other pairs, CODESYS and TwinCAT allow the inner
declaration to hide the outer one and offer an optional static-analysis
warning (`SA0013`).

The text of IEC 61131-3:2013 §6.6.5.5 could not be retrieved while this
decision was made. The project's reading is that a derived type inherits all
variables and methods, may add its own and may override methods, and that
nothing provides for redeclaring an inherited variable; the `THIS` mechanism
is motivated by a method local hiding an instance variable, not by a derived
type hiding a base one. That reading should be confirmed against the standard
and this ADR amended if it is wrong.

## Decision Drivers

* A program that analysis accepts must compile to code that does what the
  source says. Silently wrong output is the worst outcome.
* Compatibility with real dialects ([ADR-0012](0012-accept-vendor-dialect-files-as-is.md)):
  a TwinCAT file that compiles in TwinCAT should not be rejected without a
  reason the user can act on.
* The compatibility libraries ([ADR-0042](0042-library-functions-over-compiler-intrinsics.md))
  declare globals such as `Tc2_System.PI`, and user code commonly declares
  its own constant of the same name; the design requirement
  `REQ-CL-analyzer-004` says the user's declaration wins.
* The documentation, the analyzer and the back end must tell one story.

## Considered Options

* **Reject the pairs that have no executable path; leave the rest.** Keep
  `P4044` unconditional and add a rule rejecting a program local named like a
  global. Hiding that compiles correctly stays allowed.
* **Reject every cross-scope redeclaration.** One rule for all pairs.
* **Fix codegen so every pair compiles as hiding.** Give the back end scoped
  name tables and gate `P4044` to the CODESYS and TwinCAT dialects.

## Decision Outcome

Chosen option: **reject the pairs that have no executable path; leave the
rest**.

* `P4044` stays an unconditional error. `EXTENDS` is Edition 3 syntax, not a
  vendor extension, so the rule applies on the strict dialect too. The
  explanation page is rewritten to say a derived type cannot redeclare an
  inherited variable, and the `P4044` page no longer calls `EXTENDS` an
  extension.
* A new rule, `rule_program_var_hides_global` (`P4050`), rejects a program
  variable whose name matches any global variable in the merged library. A
  program names a global through `VAR_EXTERNAL`; there is no other correct
  reading of a program reusing the name, and there was no correct compilation
  of it.
* A function, function block or method local may still hide an outer name.
  Those compile correctly, are tested end to end, and `REQ-CL-analyzer-004`
  depends on the function block case.

Rejecting every pair was not chosen because it discards code that compiles
correctly today and breaks the library-constant requirement. Fixing codegen
was not chosen now because no user code depends on the broken pairs working:
a program local hiding a global has never produced a correct program, and
inherited fields have never reached the back end. Rejection is the smaller,
safer change; a later decision can relax `P4050` behind a dialect flag once
the back end resolves names per scope.

### Consequences

* Good, because every scope pair the analyzer accepts now compiles to correct
  code, and the docs describe what the compiler does.
* Good, because the diagnostic names the global's declaration, so the fix
  (`VAR_EXTERNAL` or a rename) is obvious.
* Bad, because a TwinCAT program that declares a local constant with the same
  name as a library global, such as `PI`, is now rejected under IronPLC
  although TwinCAT accepts it. Before this decision the same program compiled
  with the library's global left uninitialised, which was worse. A dialect
  flag to allow it is the natural follow-up once codegen can honour it.
* Bad, because `P4044` also rejects a derived block redeclaring a base
  `VAR_TEMP`, which TwinCAT permits. No project file has needed it; relax the
  rule when one does.
* Neutral, because the rule is asymmetric between programs and other units.
  The asymmetry follows the back end, and the `P4050` page states it.

## More Information

* Issue [#1562](https://github.com/ironplc/ironplc/issues/1562)
* `compiler/analyzer/src/rule_extends_field_duplicated.rs` (`P4044`) and
  `compiler/analyzer/src/rule_program_var_hides_global.rs` (`P4050`)
* `compiler/codegen/src/compile_setup.rs` and `compile_fn.rs` for the flat
  name table this decision works around
* [Beckhoff: Inheritance principle](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/3537661579.html)
