# Plan: Merge multiple `.plcproj` sub-projects into one compilation unit

## Problem

`detect_twincat` in `compiler/sources/src/discovery/mod.rs` already walks
the whole directory tree and finds *every* `.plcproj` file, but then
keeps only the first one (sorted by path) and silently discards the
rest:

```rust
let mut candidates: Vec<PathBuf> = files....collect();
candidates.sort();
let plcproj_path = candidates.into_iter().next()?;
```

A real TwinCAT solution commonly has more than one `.plcproj` under one
checkout -- a main PLC project plus one or more library/shared
sub-projects. Only ever loading one of them means types declared in the
others are never parsed at all, so any reference to them from the
loaded project fails type resolution (`P2008
UndeclaredUnknownType`) -- reported by `garretfick` on issue #1199 as
the second-largest error bucket in a real corpus, right after the OOP
gate.

There is already a direct precedent for the fix: PR #1216 (merged) did
the equivalent thing for LSP workspace folders -- "a multi-sub-project
TwinCAT solution needs all of them loaded together for cross-project
type resolution," merging every workspace folder into one compilation
unit instead of using only the first. This applies the same principle
one level down, inside `discover()` itself, so it also covers
`ironplcc check`/`ironplc-cli` usage against a single directory, not
just the LSP multi-workspace-folder case.

## Complication: same-directory duplicates are a different case

An *existing* test (`discover_when_multiple_plcproj_candidates_then_picks_deterministically`)
documents a different real corpus pattern: two `.plcproj` files in the
*same* directory, which the comment identifies as "an apparent stale
rename artifact" -- i.e. duplicates of the same project, not separate
sub-projects. Merging blindly by file count would treat that case as
two sub-projects and likely double-load the same POUs, producing
spurious duplicate-declaration errors where today there's a clean
(if arbitrary) single pick.

The two cases are structurally distinguishable: genuine sub-projects
live in *different* directories; accidental duplicates share a
directory. So the fix groups candidates by parent directory first:

1. Collect every `.plcproj` found by the existing recursive walk.
2. Group by parent directory; within each directory, keep only the
   first (sorted) -- this preserves today's "pick deterministically"
   behavior for the same-directory-duplicate case exactly as before.
3. Sort the resulting per-directory-deduplicated list of `.plcproj`
   paths.
4. Parse every one of them (each still resolves its own `<Compile>`
   paths relative to its own directory, unchanged) and merge:
   - `files`: concatenated in sorted-plcproj order
   - `errors`: concatenated
5. `root_dir`: if exactly one `.plcproj` was selected, keep the
   existing behavior (`root_dir` = that file's own directory -- an
   existing test asserts this, since a `.plcproj`'s own further
   subdirectory references need it). If more than one was selected,
   use the top-level directory originally passed to `discover()`,
   matching `detect_fallback`'s existing convention -- there is no
   single meaningful "the" project directory once multiple are merged,
   and nothing downstream consumes `root_dir` from the returned
   `DiscoveredProject` besides tests (confirmed: only `.files` and
   `.errors` are read at the `ironplc-cli` call site).
6. Deduplicate the final `files` list by resolved path (stable, first
   occurrence wins) in case two sub-projects happen to reference the
   same physical file -- avoids loading + declaring the same POU twice
   and getting a spurious duplicate-name diagnostic for something that
   isn't actually a user error.

## Files

- `compiler/sources/src/discovery/mod.rs` -- `detect_twincat`,
  `parse_plcproj` (unchanged signature, called once per selected
  `.plcproj`), doc comments.

## Tests

- New: `discover_when_multiple_plcproj_in_different_directories_then_merges_all`
  -- two `.plcproj` in sibling directories, each with its own POU;
  asserts both files are present in the result.
- New: `discover_when_multiple_plcproj_in_different_directories_then_root_dir_is_top_level`
  -- asserts `root_dir` falls back to the directory passed to
  `discover()` once more than one `.plcproj` is merged.
- New: `discover_when_same_file_referenced_by_two_plcproj_then_deduplicated`
  -- two sub-projects whose `<Compile>` entries happen to resolve to
  the same physical file; asserts it appears once in `files`.
- Existing `discover_when_multiple_plcproj_candidates_then_picks_deterministically`
  (same-directory duplicates) must keep passing unchanged -- confirms
  the directory-grouping step preserves that behavior.
- Existing single-`.plcproj` tests (`root_dir` = plcproj's own
  directory, nested discovery, subdirectory path resolution, missing
  `<Compile>` entries) must all keep passing unchanged.

## Out of scope

- Explicit `<ProjectReference>`-style modeling (referencing only
  specific other `.plcproj` files rather than merging everything found
  under the tree). Not pursued: the existing `detect_twincat` docstring
  already established a "no disambiguation, deterministic pick" stance
  for directory contents, and PR #1216 already set the precedent of
  merging *everything discovered* rather than modeling explicit
  references, for the equivalent LSP case. Consistent with that,
  scope this to "everything found under the directory tree
  participates," not "only explicitly cross-referenced projects."
