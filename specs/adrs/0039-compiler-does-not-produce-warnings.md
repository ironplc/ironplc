# ADR-0039: The Compiler Does Not Produce Warnings

status: proposed
date: 2026-07-22

## Context and Problem Statement

IronPLC reports problems found in IEC 61131-3 source through a single
[`Diagnostic`] type (`compiler/dsl/src/diagnostic.rs`). Every diagnostic carries
a problem code (`P####`), a primary label, optional secondary labels, and help
notes — but it carries **no severity field**. As a result the compiler has
exactly one class of finding today: a problem is a problem, and every renderer
treats it as fatal.

* The command line maps every diagnostic to `Severity::Error`
  (`compiler/ironplc-cli/src/cli.rs`).
* The language server maps every diagnostic to `DiagnosticSeverity::ERROR`
  (`compiler/ironplc-cli/src/lsp_project.rs`).
* The MCP tools emit `"severity": "error"` for every diagnostic.

Pressure to add a *warning* tier recurs. A concrete example prompted this ADR:
[PR #1215] made project discovery resilient to one unresolvable `.plcproj`
entry (a case-sensitivity mismatch, or a genuinely missing referenced file) by
continuing to check the rest of the project instead of aborting. The first
implementation recorded each unresolvable entry as a *warning*. That surfaced
the underlying question, because IronPLC has no way to render a warning that is
anything other than a quieter error: the CLI, the LSP, and the MCP tools all
have a single fatal severity, and nothing downstream distinguishes "warning"
from "error" except tone. A missing referenced file is not a stylistic nicety
the user may ignore — it means part of the project went unchecked — so calling
it a warning would have *weakened* the signal, not clarified it. PR #1215 was
revised to rename `warnings` to `errors` and keep the command's overall failure.

The question this ADR settles: **does the IronPLC compiler produce warnings?**

## Decision Drivers

* **Safety-first (ADR-0005)** — IronPLC compiles programs that control physical
  processes and run unattended for years. A warning is, by definition, a finding
  the toolchain invites you to ignore. A default that lets a real problem be
  ignored is at odds with the standing safety-first principle.
* **One meaning per finding** — a diagnostic should mean exactly one thing:
  "this must be resolved." A two-tier default forces every reader (human, CI,
  LSP client, MCP consumer) to learn and agree on where the line between the
  tiers falls, and that line drifts over time.
* **No silent accommodation (ADR-0036)** — that ADR established that a rejected
  construct is made *actionable*, never silently smoothed over. A warning tier
  is the severity-space version of the same temptation: down-rank an
  inconvenient error rather than fix the code or gate it behind an explicit
  option.
* **Honest CI and tooling** — build/check should fail when, and only when, there
  is something to fix. If a warning does not fail CI it is noise that
  accumulates and is ignored; if it does fail CI it was an error all along.
* **Room to evolve** — some future finding genuinely may be advisory (a
  deprecation, a portability hint). The decision should leave a principled,
  opt-in path to that rather than foreclosing it.

## Considered Options

* **Errors only; per-code demotion is opt-in** — the compiler emits only errors.
  A future, explicit configuration may *demote* specific problem codes to
  warning, but nothing is a warning by default and demotion is always the user's
  choice.
* **Introduce a default warning tier** — add a severity field and classify some
  problem codes as warnings out of the box (the common compiler model).
* **Status quo, informally** — keep "errors only" as an unwritten convention
  with no stated conditions for ever adding warnings, and re-litigate it each
  time the pressure recurs (as it did in PR #1215).

## Decision Outcome

Chosen option: **Errors only; per-code demotion is opt-in.**

**The IronPLC compiler does not produce warnings.** Every diagnostic it emits is
an error: something that must be resolved, that fails `ironplcc check`, and that
every renderer shows as an error. There is no default advisory tier, and code
must not classify a finding as a "warning" as a way to make it non-fatal. If a
condition is worth reporting at all, it is reported as an error; if it is not
worth failing on, it is not reported.

This is a standing policy, not a one-time decision about `.plcproj` resolution.
It applies to every new diagnostic and to every integration surface (CLI, LSP,
MCP).

### Conditions under which warnings may be introduced

Warnings are not forbidden forever — they are gated. A warning tier may be added
only when **all** of the following hold:

1. **It must not violate ADR-0005 (safety-first).** A finding may be demotable to
   a warning only if letting a user ignore it does not weaken a safety guarantee.
   A problem whose being ignored could lead to wrong values, corrupted state, or
   unsafe control of a physical process must remain a non-demotable error. This
   is the gate that comes first: if demoting a code would violate safety-first,
   none of the conditions below can rescue it.
2. **Opt-in, never default.** No problem code is a warning out of the box. The
   default severity of every code is error. A warning exists only because a user
   explicitly *demoted* a specific code (e.g. via a future project/CLI
   configuration). The set of codes demoted is chosen by the user, not by the
   compiler.
3. **Per-code, explicit, and discoverable.** Demotion is expressed against a
   concrete `P####` code, so the user knows exactly what they chose to
   down-rank. It is not a blanket "treat category X as warnings" switch.
4. **Every renderer can faithfully distinguish the tier.** A demoted code
   renders as a genuine warning in the CLI, the LSP (`DiagnosticSeverity`), and
   the MCP severity field — not as an error with a softer message. Until all
   surfaces can represent the distinction, there is no warning tier to expose.
5. **The default build stays honest.** With no demotions configured, behavior is
   identical to today: every finding is an error and fails the build. Demotion
   only ever loosens what a specific user opted into loosening; it never changes
   the out-of-the-box contract.

Until a change satisfies all four, the answer stays "errors only."

### How this shaped PR #1215

* `DiscoveredProject::warnings` was renamed to `errors`: an unresolvable
  `.plcproj` entry is a real problem (part of the project is unchecked), so it
  is an error.
* Discovery still keeps the files that *did* resolve — they are loaded and
  checked and report their own diagnostics — while the unresolved reference sets
  the overall command to failure. This is the "don't abort discovery of the
  rest of the project, but still fail overall" behavior, expressed with errors
  rather than an invented warning tier.

### Consequences

* Good, because a diagnostic has exactly one meaning — "resolve this" — with no
  per-reader judgment about which tier a finding falls into.
* Good, because CI, the CLI exit code, the LSP, and MCP all agree by
  construction: there is nothing that is reported-but-ignorable by default.
* Good, because it is consistent with safety-first (ADR-0005) and with
  no-silent-accommodation (ADR-0036): the compiler does not offer a built-in way
  to down-rank a real problem into background noise.
* Good, because it leaves a principled path to warnings (explicit per-code,
  user opt-in demotion) if a genuinely advisory finding ever arrives, without
  weakening the default.
* Neutral, because advisory-feeling findings (a deprecation, a portability hint)
  are still reported — as errors — until and unless a user demotes their code;
  they are gated, not lost.
* Bad, because contributors cannot reach for "just make it a warning" to soften
  a noisy error; a finding that is too aggressive to be fatal must instead be
  narrowed, gated behind an `--allow-*` option, or not emitted — which is more
  work than down-ranking it. This friction is the point.

## More Information

### Relationship to ADR-0005 (Safety-First Design Principle)

ADR-0005 resolves design trade-offs in favor of the option that preserves strong
guarantees for a system that controls physical processes. A warning is a
guarantee deliberately relaxed to "you may ignore this." Making that the default
for any finding contradicts safety-first; requiring the user to opt in per code
keeps the relaxation explicit and owned by the person who chose it.

### Relationship to ADR-0036 (No IronPLC Dialect)

ADR-0036 refused to add a third "IronPLC accepts this even though nothing else
does" path to syntax acceptance, insisting rejections stay actionable. This ADR
is the severity-space analogue: it refuses a third "reported but ignorable by
default" path to diagnostics. In both cases the compiler declines to add a lax
default and instead keeps findings actionable.

### Precedent for opt-in, per-code severity

The "everything is an error unless you explicitly demote a specific code" model
mirrors how configurable diagnostic severity works elsewhere. Rust's lint system
attaches a level (`allow`/`warn`/`deny`) to each named lint and lets the user
override it per code; the notable difference here is the *default*, which for
IronPLC is error for everything rather than a mixed set. The reviewer's C#
`CS2001` analogy (a missing source file is a compiler error, not a warning) is
the same instinct applied to the case that prompted this ADR. IronPLC adopts the
configurable-per-code shape but keeps the strict default that its domain
requires.

[`Diagnostic`]: ../../compiler/dsl/src/diagnostic.rs
[PR #1215]: https://github.com/ironplc/ironplc/pull/1215
