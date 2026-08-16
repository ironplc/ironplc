# Manifest-only TwinCAT discovery (remove the `.plcproj` walk)

## Goal

Stop guessing which `.plcproj` belongs to a TwinCAT project. Remove the
recursive `.plcproj` search from TwinCAT discovery entirely and resolve
projects only through the manifest chain `.sln` → `.tsproj` →
`PrjFilePath`. When a manifest is present but cannot be resolved
unambiguously, emit a diagnostic instead of falling back to a heuristic.

This is the follow-up requested in the review of
[#1342](https://github.com/ironplc/ironplc/pull/1342). That PR added the
`.sln` → `.tsproj` → `PrjFilePath` chain but kept `collect_plcproj_via_walk`
as a fallback, which silently reinstates the exact failure the PR set out
to fix (alphabetical sort deciding between a live and a stale `.plcproj`)
whenever an authoritative manifest is found but not resolved.

## Architecture

### The rule

A path is either the project manifest itself or the folder holding it;
both say the same thing, so an editor that can only open a folder is as
well served as a command line that can name a file. Checked in order,
first match wins:

| # | Input | Behavior |
|---|---|---|
| 1 | a `.sln` or `.plcproj` file | TwinCAT |
| 2 | a folder holding exactly one `.sln` or `.plcproj` | TwinCAT |
| 3 | a `plc.xml` file, or a folder holding one | Beremiz |
| 4 | anything else | unstructured: enumerate loose source files |

Rule 2 counts `.sln` and `.plcproj` together: a folder holding one of
each, or two of either, names no single project and falls to rule 4. That
is not an error — nothing is being selected, so there is nothing to guess
between, and a user who meant one of them says so by naming it (rule 1).

Once a manifest *is* found, failing to follow it is an error (P6012), not
a reason to fall back to rule 4: enumerating the folder would compile the
project's sources while ignoring the manifest that says which of them
belong, which is the same guess the manifest exists to settle.

`.tsproj` is not an entry point. It remains part of the resolution chain —
a `.sln` reaches its `.plcproj` files through one — but nobody opens a
solution by naming its system project.

The recursion the walk used to provide is replaced by traversal *by
reference*: a `.sln` names its `.tsproj` files, a `.tsproj` names its
`.plcproj` files, and a `.plcproj` names its sources. The unstated
convention the walk was compensating for becomes explicit — **open the
folder containing the manifest**.

Recursion now happens in exactly one place: `detect_fallback`, rule 4,
where no manifest format is in play at all and enumeration *is* the
project definition. The TwinCAT and Beremiz detectors read the given
folder and nothing below it.

### The detector result type

`detect_twincat`'s `Option<Result<DiscoveredProject, Diagnostic>>` cannot
express the rule: it has no way to say "not my project, but here is
something worth telling the user", and it does not distinguish "no
manifest here" from "manifest found but unresolvable" — precisely the
distinction that decides whether falling through to the next detector is
legitimate. Replaced by:

```rust
enum Detection {
    NotDetected,
    Detected(Box<DiscoveredProject>),
    Failed(Diagnostic),
}
```

`Failed` never falls through; `NotDetected` always does.

### Breaking change

Anyone whose setup works today by accident — pointed the tool at a repo
root and the walk happened to find the right thing — now gets that folder
enumerated as loose source files instead of the project the walk guessed
at. Opening the folder that holds the manifest (rule 2) is the fix, and is
what an IDE does by default.

## File map

Modified:

- `compiler/sources/src/discovery/mod.rs` — `Detection` enum; tiered
  `detect_twincat`; `discover` fall-through policy; `detect_beremiz`
  returns `Detection`; new `discover_from_manifest` / `is_manifest`
  public entry points for file arguments
- `compiler/sources/src/discovery/sln.rs` — non-recursive manifest lookup
  by extension; `.sln` / `.tsproj` resolution returns `Result` so an
  unreadable or malformed manifest becomes a diagnostic instead of an
  empty vec
- `compiler/sources/src/discovery/plcproj.rs` — drop
  `collect_plcproj_via_walk`; absorb the `.plcproj` parsing tests from
  `mod.rs` to keep every module under the 1000-line limit
- `compiler/ironplc-cli/src/cli.rs` — `enumerate_files` accepts a manifest
  as a file argument instead of loading it as source text
- `compiler/problems/resources/problem-codes.csv` — P6012

Created:

- `docs/reference/compiler/problems/P6012.rst`

## Tasks

- [x] Add problem code P6012 (manifest found but unresolvable)
- [x] Rework `sln.rs`: non-recursive manifest lookup; `Result`-returning
      `.sln` and `.tsproj` resolution
- [x] Add the `Detection` enum and rewrite `detect_twincat` for rules 1-2
- [x] Delete `collect_plcproj_via_walk`
- [x] Teach `discover` to take a manifest or a folder, and `detect_beremiz`
      to do the same for `plc.xml`
- [x] Collapse `enumerate_files` onto the single `discover` entry point
- [x] Tests: each row of the rule table; the stale-rename regression; a
      manifest named directly for each entry-point extension
- [x] Write the problem doc page
- [x] Full CI (`cd compiler && just`)
