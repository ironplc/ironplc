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

Manifests are tiered `.sln` > `.tsproj` > `.plcproj`, so a solution folder
containing a stray `.plcproj` still resolves through the `.sln`. Only the
directory itself is examined — never a subtree.

| Input | Behavior |
|---|---|
| file argument: `.sln` / `.tsproj` / `.plcproj` | authoritative, use it |
| folder, exactly one manifest at the top tier present | authoritative; a resolution failure is a diagnostic, never a heuristic |
| folder, multiple manifests at the same tier | diagnostic naming them; the user passes one explicitly |
| folder, no manifest, one found nested below | diagnostic pointing at it |
| folder, no manifest anywhere | unstructured: enumerate loose source files (unchanged) |

The recursion the walk used to provide is replaced by traversal *by
reference*: a `.sln` names its `.tsproj` files, a `.tsproj` names its
`.plcproj` files, and a `.plcproj` names its sources. The unstated
convention the walk was compensating for becomes explicit — **open the
folder containing the manifest**.

The nested-manifest hint (row 4) is the only remaining use of `walk_files`
in the TwinCAT path, and it is purely for the error message: it never
selects sources. It fires only when the directory holds nothing of its
own, so a directory of loose `.st` files that happens to contain an
unrelated solution in a subfolder does not get a spurious diagnostic.

"Nothing of its own" is measured against the nested manifest's directory
rather than by an empty fallback enumeration. A real project's sources sit
*beside* its manifest and are themselves supported file types
(`.TcPOU`, `.TcGVL`, ...), so an empty-enumeration test would never fire
for the case row 4 exists to catch — opening the tree above a TwinCAT
project. Enumerating those files and calling the directory unstructured
would compile the project's sources while ignoring the manifest that says
which of them belong, which is the same guess the manifest exists to
settle.

Bare-`.st` directories are unaffected: they never reach the TwinCAT
detector, and `detect_fallback` stays recursive — with no manifest format
in play, enumeration *is* the project definition.

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
    NotDetectedWithHint { manifest: PathBuf, diagnostic: Diagnostic },
    Detected(Box<DiscoveredProject>),
    Failed(Diagnostic),
}
```

`Failed` never falls through; `NotDetected*` always does. `discover` owns
the policy for when a hint is surfaced, so a detector never needs to know
the fallback's outcome — the hint carries the manifest path it names,
which is the only input that policy needs.

### Breaking change

Anyone whose setup works today by accident — pointed the tool at a repo
root and the walk happened to find the right thing — gets a diagnostic
instead of results. Accepted deliberately in favor of correctness (see the
review discussion on #1342).

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
- `compiler/problems/resources/problem-codes.csv` — P6012, P6013, P6014

Created:

- `docs/reference/compiler/problems/P6012.rst`
- `docs/reference/compiler/problems/P6013.rst`
- `docs/reference/compiler/problems/P6014.rst`

## Tasks

- [x] Add problem codes P6012 (ambiguous manifests), P6013 (manifest found
      but unresolvable), P6014 (no manifest in directory, one nested below)
- [x] Rework `sln.rs`: non-recursive manifest lookup; `Result`-returning
      `.sln` and `.tsproj` resolution
- [x] Add the `Detection` enum and rewrite `detect_twincat` to tier
      `.sln` > `.tsproj` > `.plcproj` over the directory only
- [x] Delete `collect_plcproj_via_walk`
- [x] Teach `discover` the fall-through policy, including the
      "hint only when the directory holds nothing of its own" rule
- [x] Add `is_manifest` / `discover_from_manifest` and wire them into
      `enumerate_files`
- [x] Tests: each row of the rule table, per manifest tier; the
      stale-rename regression; a manifest file argument for each of the
      three extensions
- [x] Write the three problem doc pages
- [x] Full CI (`cd compiler && just`)
