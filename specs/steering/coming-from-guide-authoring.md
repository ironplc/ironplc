# "Coming from X" Guide Authoring

This file governs how the documentation website's **"Coming from X" how-to
sections** (`docs/how-to-guides/<product>/`) are written, so that every
section reads the same way regardless of which source product it covers and
regardless of who — human or AI — writes it. The TwinCAT section is the
reference implementation of this pattern; the Beremiz section predates parts
of it and converges over time.

## Purpose and audience

A "Coming from X" section is for an engineer who **already uses product X**
and wants to use IronPLC *alongside* it — not replace it. The section's job
is to walk their existing, unmodified project through the IronPLC journey:

1. **Check** — get diagnostics on the project as-is
2. **Run** — execute the project's logic without the vendor's runtime,
   license, or hardware
3. **Libraries** — use the vendor libraries the project references
   (only when IronPLC bundles compatibility libraries for that vendor)

These are how-to guides in the [Diátaxis](https://diataxis.fr/) sense:
task-oriented, one goal per page, no teaching of concepts. Concepts belong
in `docs/explanation/`, exhaustive detail in `docs/reference/`.

## URL stability policy

**Published URLs are permanent. Never delete or rename a page that has
shipped.** External sites, search engines, and diagnostics link to these
pages; a removed page is a broken link we cannot fix from our side.

- Improve a page by **rewriting its content in place**, keeping its file
  name (and therefore its URL).
- Add capability coverage as **new pages** with slugs from the standard set
  below — never by renaming an existing page.
- If a page's task becomes genuinely obsolete, keep the page as a short
  stub that states what changed and links to the replacement. Do not
  remove the file.
- Choose new slugs expecting them to live forever: task-first, no version
  numbers, no words tied to a capability's current maturity (avoid e.g.
  `preview`, `experimental`, `new`).

## Standard page set and slugs

Every "Coming from X" section uses the same slugs so the sections stay
parallel across products (`<product>` is the lowercase directory name,
e.g. `twincat`):

| Slug | Task | When present |
|---|---|---|
| `index` | Section landing page | Always |
| `check-<product>-projects` | Check a project for problems | Always |
| `run-<product>-projects` | Compile and run without the vendor runtime | When the product's projects can execute on the IronPLC VM |
| `use-<vendor>-libraries` | Use vendor libraries the project references | When compatibility libraries exist for the vendor (see the [glossary](glossary.md) for *vendor* vs. *dialect*) |

Additional tasks get their own pages with **verb-first slugs**
(`<verb>-<object>`), added to the section's toctree in journey order:
check, then run, then libraries, then anything else.

## The index page

The landing page must contain, in order:

1. **The promise, concretely.** One short paragraph: IronPLC reads the
   product's projects **as-is** — name the actual file and project
   extensions — and what the reader gets (check it, run it, use its
   libraries). Lead with what works on their existing files, not with
   what IronPLC is.
2. **The journey.** The toctree, in journey order, with explicit titles.
3. **What works today.** A short, honest expectations paragraph: the
   maturity of support (e.g. which language IronPLC executes, that library
   support is early), linking to the reference pages that carry the
   details. Understating and delivering beats overstating; a reader who
   hits an undocumented wall does not come back.

## Task page structure

Each task page follows this shape:

- **Title**: the task as an imperative or gerund phrase matching the slug.
- **Goal sentence**: one sentence stating what the reader accomplishes.
- The `requires-compiler` include (or the appropriate prerequisite
  include from `docs/includes/`).
- **Lead with the realistic case.** The first command operates on the
  reader's *whole project or solution directory*, exactly as the vendor
  tool laid it out — not on a single hand-made file. Single-file and
  special-case variants come after.
- **Show both surfaces** where both exist: the command line and the
  VS Code extension (settings, not flags, for the editor).
- **Show expected output** when seeing it builds confidence (a variable
  dump, a specific diagnostic) — readers use it to confirm they are on
  the happy path.
- **See Also**: links to the relevant reference pages, so the task page
  can stay short.

## Content rules

- **Do not enumerate syntax support.** The product's dialect preset
  (`--dialect <name>`) makes vendor syntax work by default; that is one
  sentence and a link to
  `docs/explanation/enabling-dialects-and-features.rst`. Per-feature
  syntax lists live in the reference section and go stale in how-to prose.
- **Every command must be verified before it ships.** Run each documented
  command against a checked-in fixture (prefer the end-to-end fixtures
  under `compiler/*/resources/test/`) and confirm the documented output.
  A how-to whose first command fails costs more trust than no how-to.
- **State the failure mode next to the feature.** When a capability has a
  visible boundary (e.g. a referenced library IronPLC does not bundle),
  say what the reader will see — including the problem code — and what to
  do about it.
- **Name what is not supported** when a reader coming from the product
  will predictably look for it (e.g. precompiled library files). One
  sentence, plus the workaround if one exists.
- **Trademarks.** Use the vendor's product names only nominatively — to
  identify the files and libraries being read. On pages that reproduce a
  vendor library surface, include the independence disclaimer (IronPLC is
  independent and not affiliated with or endorsed by the vendor). Keep
  `docs/trademarks.rst` in sync when referencing a mark it does not yet
  list. See also
  [compatibility-library-authoring.md](compatibility-library-authoring.md)
  for what may legally ship in library content itself.
- **House style**: Sphinx roles as used elsewhere in `docs/` —
  `:file:` for file names and extensions, `:program:` for tools,
  `:doc:` for internal links; `code-block:: shell` for commands. Match
  the section-underline characters of the existing pages.

## Checklist: adding a new "Coming from X" section

1. Create `docs/how-to-guides/<product>/index.rst` plus at least
   `check-<product>-projects.rst`, following the templates above.
2. Add a grid card **and** a hidden-toctree entry in
   `docs/how-to-guides/index.rst` ("Coming from X").
3. Confirm a reference page exists for the product's source format under
   `docs/reference/compiler/source-formats/` and link it from the check
   page.
4. Verify every command against fixtures; add a fixture if none exists.
5. Check `docs/trademarks.rst` covers the product's marks.
6. Build the docs and fix any new warnings before pushing.
