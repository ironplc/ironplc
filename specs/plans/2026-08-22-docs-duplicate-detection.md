# Documentation Duplicate Detection

## Goal

Make "do not duplicate content" an explicit project rule that covers the
documentation website, and provide an automated check that finds duplicated
prose in `docs/` without flagging the mechanisms the docs already use to
*avoid* duplication (`.. include::`, substitutions, cross-references).

## Off-the-shelf evaluation

Every candidate below was run against `docs/` (378 `.rst` files, 24k lines)
before deciding to build anything. Two reference findings were used to
measure recall, because both are the case the rule most wants caught -- text
that already exists as a shared include, copied inline instead:
`includes/report-internal-vm-error.rst` (inlined in 13 `V90xx` pages) and
`includes/requires-compiler.rst` (inlined in the AI-agents guide).

| Tool | Invocation | Result on `docs/` | Finds the inlined includes? |
|------|------------|-------------------|-----------------------------|
| jscpd 5.0.16 | `npx jscpd docs` | Analyzed 0 `.rst` files -- the `rest` format exists but is not mapped to the extension | n/a |
| jscpd | `--formats-exts rest:rst --min-tokens 20` | 1030 clones; 54% anchored on a section underline, 22% list-table/directive scaffolding, 23% prose | Yes, but buried |
| jscpd | `--formats-exts rest:rst --min-tokens 50` | 167 clones (14% prose) | **No** |
| jscpd | `--formats-exts rest:rst --min-tokens 80` | 54 clones (22% prose) | **No** |
| dcd 1.1.0 | `dcd -i rst -m 6` | 6661 blocks / 25531 line matches (`-m 10`: 5016 blocks); line-exact, so re-wrapped prose does not match | **No** -- zero hits in `docs/includes/` |
| copydetect 0.5.0 | `-e rst` | 12,685 file pairs above 0.4, dominated by the legitimately parallel `eq`/`ne`/`lt`/`gt` reference pages | File-level only |
| RedPen 1.10.4 | `DuplicatedSection`, `-f rest` | 150,800 errors at threshold 0.7, 7167 at 0.95; needs `--add-opens` to run on a current JVM; output never names the other file | Not determinable |
| PMD CPD 7 | -- | No plain-text, Markdown, or RST input language | n/a |
| Vale 3 | -- | All extension points are within-document; `repetition` matches repeated tokens, not duplicated paragraphs | n/a |
| doc8, sphinx-lint, rstcheck, restructuredtext-lint | -- | Syntax and style only; no duplication check | n/a |
| Sphinx / docutils | -- | Duplicate *labels and targets* only, never prose | n/a |

No published Sphinx- or RST-aware duplication checker exists on PyPI or
GitHub.

The decisive result is jscpd's trade-off: at a threshold low enough to catch
the inlined includes it reports 1030 clones of which roughly three quarters
are RST punctuation, and at a threshold quiet enough to read it no longer
reports them at all. The noise is structural -- section underlines and
`list-table` markup are the most repeated byte sequences in an RST tree --
so it cannot be tuned away by a token count.

One correction to the original premise: `.. include::` is only a false
positive for a tool that renders RST or scans built HTML. Every tool above
reads source, where an include is a single line, so shared includes cost
nothing. The RST awareness that is actually needed is for adornments,
directives and tables.

## Decision

Adopt jscpd, cut the noise with path exclusions, pin a threshold at today's
level, and record the gap as a repository issue whose resolution is an
upstream contribution. The custom prototype is removed: the project should not
own a reStructuredText parser when the fix belongs in the tool everyone uses.

The measured consequence of that choice, recorded here because the numbers do
not appear anywhere else:

| Configuration | Clones | Duplicated lines |
|---------------|--------|------------------|
| No exclusions | 1028 | 31.04% |
| Excluding `reference/standard-library/**` and `reference/compatibility-libraries/**` (adopted) | 535 | 18.75% |
| Excluding all of `reference/**` | 166 | 17.79% |

Per-clone suppression was evaluated and rejected. `--ignore-pattern` on
section adornments changes nothing at all (clone boundaries span the ignored
byte ranges), the same pattern plus directives and options only reaches 720,
and inline `jscpd:ignore-start/end` markers would mean hundreds of comments
scattered through the documentation. The threshold is therefore a regression
ratchet, not a quality bar.

## Architecture

Three parts:

1. **Rule** — an "Avoid Duplication" section in
   `specs/steering/development-standards.md` that states the DRY expectation
   for code *and* documentation, names the approved documentation
   de-duplication mechanisms in preference order (`.. include::`,
   substitution, cross-reference), and lists the duplication that is
   acceptable. Referenced from the Critical Rules in `CLAUDE.md` and
   `CURSOR.md`; the Kiro pointer file is generic and needs no change.

2. **Fixes** — the copies that already had a shared include but did not use
   it. `includes/report-internal-vm-error.rst` is narrowed to the paragraph
   invariant across all seventeen `V90xx` pages, and the cause paragraph
   shared by eight of them moves to `includes/internal-vm-error-cause.rst`.
   Pages with tailored cause sentences (`V9003`, `V9007`, `V9011`, `V9013`)
   keep their own wording; `V9014` and `V9015` gain inline the paragraph they
   previously drew from the wider include, so no rendered text changes.

3. **Check** — jscpd, pinned in `docs/package.json` and configured in
   `docs/.jscpd.json`: `.rst` mapped to the `rest` format, the two symmetric
   reference trees excluded, and a threshold pinned at the current figure.
   `just duplicates` runs it and `just ci` depends on it, so the docs job
   needs Node alongside Python.

Note on the original premise: `.. include::` is only a false-positive source
for a tool that renders RST or scans built HTML. jscpd reads source, where an
include is one line, so shared includes cost nothing. The RST awareness that
is actually missing is for adornments, directives and tables — which is what
issue #1409 tracks.

## File map

| File | Change |
|------|--------|
| `specs/steering/development-standards.md` | Add the "Avoid Duplication" rule |
| `CLAUDE.md`, `CURSOR.md` | Add the rule to Critical Rules |
| `docs/.jscpd.json` | New: jscpd configuration (format mapping, exclusions, threshold) |
| `docs/package.json`, `docs/package-lock.json` | New: pin jscpd for reproducible runs |
| `docs/justfile` | New `duplicates` recipe, wired into `ci`; `npm ci` in `setup` |
| `.github/workflows/partial_website.yaml` | Add Node.js setup for the docs job |
| `docs/includes/report-internal-vm-error.rst` | Narrow to the invariant paragraph |
| `docs/includes/internal-vm-error-cause.rst` | New: the shared cause paragraph |
| `docs/reference/runtime/problems/V90*.rst` | Use the includes instead of inline copies |
| `docs/how-to-guides/ai-agents/…` | Use the `requires-compiler` include |
| `docs/CONTRIBUTING.md` | Document the check and how to read its threshold |

## Tasks

- [x] Write this plan and commit it
- [x] Evaluate the off-the-shelf tools against `docs/` and record the results
- [x] Convert the inlined includes: 12 `V90xx` pages and the AI-agents guide
- [x] Add the jscpd configuration, recipe, and CI wiring
- [x] Remove the prototype checker
- [x] Add the "Avoid Duplication" rule to the steering files and entry points
- [x] Document the check and its limits in `docs/CONTRIBUTING.md`
- [x] File the repository issue tracking the upstream contribution ([#1409](https://github.com/ironplc/ironplc/issues/1409))
