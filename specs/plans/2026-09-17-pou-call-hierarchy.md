# Enforce the POU Call Hierarchy (P4051)

Follow-up to #1728, which deleted `rule_pou_hierarchy.rs`. That file's name
and doc comment promised the IEC 61131-3 call hierarchy but implemented only
duplicate-name detection; its one hierarchy test ended in a commented-out
assertion. The hierarchy has never been enforced: a `FUNCTION` that declares
a function block instance and invokes it passes `ironplcc check` today.

## Goal

A function has no state (IEC 61131-3 Ed.2 §2.5.1, Ed.3 §6.6.1). So a
`FUNCTION` may not declare a function block instance, nor invoke one, nor
call a method on one. The one exception is an instance passed in through
`VAR_IN_OUT`, which Ed.3 permits: the instance belongs to the caller, so the
function stays stateless. Every violation is `P4051
FunctionBlockInFunction`, one diagnostic per offending declaration and one
per offending invocation, so both the declaration and each call site are
marked.

Programs and function blocks may declare and invoke function blocks and call
functions, so nothing is reported for them. The other half of the hierarchy,
that nobody invokes a program, is not expressible in the syntax: a program
name is not a type and a program is not callable.

Out of scope: `VAR_IN_OUT` in a function is accepted by the parser but not
by the function signature (a call with the argument reports `P4018`). The
rule honours the standard's exemption so it does not become the wrong error
once that support lands.

## Architecture

`rule_pou_hierarchy.rs`, a `DiagnosticVisitor` in the usual shape. Inside
a `FUNCTION` it records each function block instance declaration with its
block kind (type resolution has already turned every instance declaration
into a `FunctionBlock` initializer, the same test `InstanceTypes` uses),
reports those outside `VAR_IN_OUT`, then reports every `FbCall` and every
`MethodCall` whose receiver is one of the reported instances. It does not
recurse into function blocks or programs at all.

## Prefactoring

None. The rule reuses the initializer-kind test the analyzer already relies
on and adds no branch to any existing pass. The map it keeps (instance name
to variable kind) differs from `InstanceTypes` (instance name to block
type) in what it stores, so neither replaces the other.

## Design doc reference

None; `specs/steering/iec-61131-3-compliance.md` covers the standard.

## File map

- `compiler/problems/resources/problem-codes.csv` — `P4051`
- `compiler/analyzer/src/rule_pou_hierarchy.rs` — new
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` —
  register it
- `docs/reference/compiler/problems/P4051.rst` — problem page
- `docs/reference/language/pous/function.rst` — one sentence

## Tasks

- [ ] Commit this plan
- [ ] Problem code, rule with tests, registration
- [ ] Problem page and reference sentence
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] `git rm` this plan
