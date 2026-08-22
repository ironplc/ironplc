# Plan: Documenting Debugger Support on the Docs Website

## Goal

Add debugger documentation to the docs website (`docs/`), covering all four
documentation quadrants, so that a user can discover that IronPLC debugs
Structured Text, learn it on a worked example, look up every launch attribute
and limit, and understand why debugging a scan cycle is not like debugging an
ordinary program.

The debugger shipped across #1364 (Layer-1 debug-info swap), #1385 (always
build and ship `ironplcdap`), and #1388 (scan count in the `Runtime` scope).
The docs website says nothing about it — and the landing page actively says the
opposite.

## Design doc reference

- `specs/design/debugger-support.md` — the debugger architecture and, critically,
  the **v1 scope decisions** table. That table is the authority for what these
  pages may claim.
- `specs/design/debug-info-in-iplc-container.md` — the debug section format.
- `specs/steering/development-standards.md` §"Documentation Standards" — the
  quadrant framework, writing style, and RST role conventions these pages follow.

## Current state

### What is implemented and user-visible

| Surface | Detail |
|---------|--------|
| DAP server | `ironplcdap` (`compiler/vm-cli/src/dap_main.rs`), no CLI arguments, speaks DAP on stdin/stdout, built and shipped unconditionally |
| Editor debug type | `ironplc`, registered for the `61131-3-st` and `twincat-pou` languages |
| Launch attributes | `program` (an `.st`/`.TcPOU` source **or** a compiled `.iplc`), `stopOnEntry`, `scanLimit` |
| Breakpoints | Source-line, snapped to the nearest executable line; the editor's dot moves to the bound line |
| Stack trace | Real POU names with the paused source line highlighted |
| Variables | Two scopes — `Program` (named, typed, `counter : DINT = 42`, STRING rendered as a quoted literal) and `Runtime` (`scanCount : ULINT`) |
| Execution control | `continue`, `next`, `stepIn`, `stepOut`, `configurationDone`, `disconnect` |
| Source-to-container | The extension compiles a source `program` to a container before launching |
| Setting | `ironplc.dapServerPath` overrides DAP-server discovery |
| Debug info | Always emitted by `ironplcc compile`; there is no `--debug` flag and no strip flag |

### What is explicitly *not* in v1

This list is the reason the docs need writing carefully rather than quickly. Each
item is a documented cut, not a bug:

- **No variable forcing or writing** while paused. Replaced (eventually) by
  logpoints, which are also not implemented.
- **No multi-instance debugging.** `launch` refuses a container with more than
  one program instance (`V6010`).
- **No pause-while-running.** The `pause` request answers
  `requestNotApplicable`; the DAP loop is single-threaded and services requests
  at natural stop points.
- **No `evaluate`**, so no watch expressions, no debug-console expressions, and
  no hover-to-inspect.
- **`WSTRING` renders `<not available>`.**
- **FB instance fields and FB type names** are not resolved (design gaps 5–6).
- **Column-level breakpoints** are deferred; breakpoints are line-level.
- **The `Step Scan Cycle` toolbar button does not work.** The command and the
  `debug/toolBar` contribution ship in `package.json`, but the server rejects
  `ironplc/stepScan` as an unknown command, so the extension shows a warning.
  See "Blocking issue" below.

### What the docs say today

| Location | Problem |
|----------|---------|
| `docs/index.rst:23` | "doesn't yet provide I/O mapping or debugging capabilities" — **factually wrong now** |
| `docs/reference/editor/overview.rst` | Lists Commands, Build Tasks, Bytecode Viewer, Diagnostics, Run Program — no Debugging section |
| `docs/reference/editor/settings.rst` | Documents `ironplc.path`, `logLevel`, `logFile`, `dialect` — omits `ironplc.dapServerPath` |
| `docs/how-to-guides/troubleshoot-editor.rst` | No debugger section |
| `docs/quickstart/` | No debug chapter |
| `docs/explanation/` | Nothing on debugging; `execution-cycle.rst:33` points at `ironplcvm` for "test and debug" |
| `docs/reference/runtime/index.rst` | Describes `ironplcvm` only; `ironplcdap` ships beside it and is unmentioned |

The **only** debugger documentation that exists is the problem-code reference:
`V6008`, `V6009`, `V6010` (runtime) and `E0004`–`E0007` (editor). Those pages
are already written and linked. They are the floor, not the ceiling: a user who
has not been told the debugger exists never reaches them.

## Blocking issue to resolve before writing

`Step Scan Cycle` is contributed to the debug toolbar but always fails. Writing
it up as "not yet implemented" documents a broken button; omitting it leaves a
visible button undocumented. Neither is good. **Recommendation: hide the
contribution behind implementation** — remove the `debug/toolBar` and
`commandPalette` entries until the server handles `ironplc/stepScan` — and file
the server-side work separately. That is a small extension change, not a docs
change, and it should land first. If the button stays, the reference page must
list it as a known limitation.

## Approach: permanent pages, one shared banner, limitations in context

### Every page is permanent

`specs/steering/coming-from-guide-authoring.md` §"URL stability policy" already
states the rule and it governs here too: **published URLs are permanent.** No
page in this plan is a staging area for the debugger's current maturity. That
has three consequences for how these pages get written:

- **No temporary pages.** There is no "debugger limitations" page, no "preview"
  page, and no page whose reason to exist disappears when a capability lands.
  Every page in the file map is one a mature debugger still needs.
- **No maturity words in slugs.** No `preview`, `experimental`, `v1`, or `new`
  in any file name. The slugs are task-first and expected to live forever.
- **Maturity lives in content, not in structure.** When a capability lands, the
  change is an edit inside an existing page — never a new page, a rename, or a
  deletion.

### One shared banner

`docs/includes/debugging-in-development.rst` (already written) holds the single
statement that debugging is early:

```rst
.. note::
   Debugging is early and in development. Breakpoints, stepping, and variable
   inspection work for Structured Text programs, but some capabilities are
   missing or limited. Each section notes the limits that apply to it.
```

Every debugger page opens with `.. include:: /includes/debugging-in-development.rst`
directly under its title. **The text is never copied into a page.** This follows
the existing pattern — `enabled-by-flag.rst`, `compat-library-independence.rst`,
and `report-internal-vm-error.rst` are all included, never pasted — and it means
one edit retires the banner from every page at once.

`docs/includes/` is in `conf.py`'s `exclude_patterns`, so the include is never
built as a standalone page and never needs a toctree entry.

### Limitations go where the user meets them

An earlier draft of this plan proposed a second include: one big v1
capability/limitation table, shared across pages. That is exactly the temporary
artifact this section rules out — a table whose rows all delete themselves as
the debugger matures, sitting in a block a reader has to mentally join back to
the feature it constrains.

Instead each limitation is stated **once, inline, in the section that raises
it**, in the same present-tense voice as the rest of the page:

| Limitation | Where it is stated |
|------------|--------------------|
| No variable forcing or writing | Reference §Variables; Explanation (with the reasoning) |
| No watch expressions or `evaluate` | Reference §Variables |
| No pause-while-running | Reference §Execution control, next to `scanLimit` |
| Single program instance only | Reference §Launch preconditions; How-to §Before you start |
| `WSTRING` shows `<not available>` | Reference §Variables |
| FB fields and FB type names unresolved | Reference §Variables; Tutorial, where the doorbell's `TON` makes it concrete |
| Breakpoints are line-level, and snap | Reference §Breakpoints |

Retiring a limitation is then a one-paragraph edit in one page, and the page
around it stays correct. Retiring the last of them is deleting the include, the
`.. include::` lines, and nothing else.

## Quadrant mapping

### 1. Tutorial (learning-oriented) — `docs/quickstart/`

**New chapter: `docs/quickstart/debugging.rst` — "Find a Bug with the Debugger"**

Placed after `configuring.rst` and before `multiple-files.rst`: the learner has
a complete, configured, single-instance program at that point, which is exactly
the shape v1 debugs.

The chapter opens with the shared banner like every other debugger page, then
*teaches through a fault* rather than touring a UI. Sketch:

1. Introduce a small, deliberate bug into the quickstart doorbell program (a
   counter that never resets, or an inverted condition) — the learner sees wrong
   output first.
2. Set a breakpoint by clicking the gutter. Note that the dot may snap down to
   the next executable line, and why.
3. Press F5. The program pauses; the paused line highlights.
4. Read the `Program` scope: names and types, not slots.
5. Read the `Runtime` scope: watch `scanCount` increment on each `continue` —
   this is the moment the scan-cycle mental model lands.
6. Step with F10, watch the variable change, spot the fault.
7. Fix it, re-run, confirm.

Constraints: single program instance throughout; no watch expressions; no
editing values while paused (the learner must fix the source and re-launch,
which is worth stating plainly so it does not read as a missing button).

One wrinkle specific to this program: the doorbell declares
`PulseTimer : TON`, and FB instance fields do not resolve yet (design gaps
5–6). The learner cannot inspect `PulseTimer.ET` in the Variables pane, which
is the value they would most want to watch. Steer the chapter's breakpoint and
its narrative onto `Button` and `Buzzer` — plain `BOOL`s that render correctly
— and reach for `scanCount` to show time passing, rather than the timer's
elapsed time.

**Also update** `docs/quickstart/index.rst` toctree and the chapter's own
"next steps" links.

### 2. How-to guides (problem-oriented) — `docs/how-to-guides/`

**New: `docs/how-to-guides/getting-started/debug-a-program.rst` —
"Debug a Program in Your Editor"**

Task-shaped, no narrative, for someone who already knows what a debugger is:

- Create a launch configuration (and note that the editor offers one
  automatically from `initialConfigurations`).
- Debug the active file vs. debug a pre-built `.iplc` (`program` accepts both).
- `stopOnEntry` — pause before the first scan.
- `scanLimit` — stop a runaway program after N scans, and why a PLC program
  otherwise never ends.
- Breakpoints in a multi-file project (the source-file table resolves them per
  file).
- Debug a TwinCAT `.TcPOU` (the debug type is registered for `twincat-pou`).
- Point at the reference page for the full attribute list.

**New: `docs/how-to-guides/getting-started/debug-without-vs-code.rst` —
"Debug from Another Editor"** *(second priority; ship if capacity allows)*

`ironplcdap` is a plain DAP server over stdio, so Neovim (`nvim-dap`), Emacs
(`dap-mode`), and other DAP clients can drive it. This guide gives the adapter
definition and the launch-request JSON, and states the one thing those clients
do not get for free: they must compile the source to `.iplc` themselves,
because that step lives in the VS Code extension, not in the server.

**Extend: `docs/how-to-guides/troubleshoot-editor.rst`**

Add a "The debugger does not start" section keyed to the existing problem codes,
so symptoms route to `E0004`–`E0007`, `V6008`–`V6010` rather than duplicating
them: server not found → `E0007` and `ironplc.dapServerPath`; wrong file type →
`E0005`; compile failed → `E0006` and the "IronPLC Debug" output channel;
multi-instance → `V6010`.

**Also update** `docs/how-to-guides/getting-started/index.rst` toctree.

### 3. Reference (information-oriented) — `docs/reference/`

The reference section is organized **by tool**, so the debugger splits in two
rather than getting its own top-level section. (An alternative — a standalone
`reference/debugger/` section — reads well but breaks the "one section per
executable" convention and orphans the settings that belong to the extension.)

**New: `docs/reference/editor/debugging.rst`**

The editor-facing contract:

- The `ironplc` debug type and the languages it covers.
- Every `launch.json` attribute: `program`, `stopOnEntry`, `scanLimit` — type,
  default, and behavior, in a table.
- What a breakpoint binds to and the snapping rule.
- The `Program` and `Runtime` scopes and what each contains.
- Which editor debug actions work (continue, step over/in/out) and which do not
  (pause, watch, set-value), each stated in the section it belongs to.
- Links to `E0004`–`E0007`.

This page carries the most inline limitations of the four, because it is where a
user looks when something they expected is not there.

**New: `docs/reference/runtime/ironplcdap.rst`**

The server-facing contract, mirroring the shape of the existing
`reference/runtime/ironplcvm.rst`:

- What `ironplcdap` is, that it takes no arguments, and that it is installed
  beside `ironplcc`/`ironplcvm`.
- The supported DAP request set, and the requests that answer
  `requestNotApplicable`.
- The two launch preconditions (debug section present; exactly one program
  instance) and the codes they raise.
- Links to `V6008`–`V6010`.

**Edit: `docs/reference/editor/settings.rst`** — add an `ironplc.dapServerPath`
subsection matching the existing per-setting format, and add it to the combined
example block.

**Edit: `docs/reference/editor/overview.rst`** — add a `Debugging` section after
`Run Program`, drawing the distinction that page currently leaves implicit:
`Run Program` executes; the debugger pauses.

**Edit: `docs/reference/runtime/index.rst`** — the section intro currently
describes `ironplcvm` alone; name `ironplcdap` as the second binary and add it
to the toctree.

**Edit: `docs/reference/compiler/ironplcc.rst`** — one line stating that
`compile` always emits debug information; there is no separate debug build.

**Edit: `docs/reference/runtime/problems/V6009.rst`** — solution 1 says
"recompile with debug information enabled", which implies a flag that does not
exist. Reword to the real causes: a container produced by a different tool, a
hand-stripped container, or a stale container from an older compiler.

### 4. Explanation (understanding-oriented) — `docs/explanation/`

**New: `docs/explanation/debugging-a-scan-cycle.rst` — "Debugging a Scan Cycle"**

The page that makes the v1 limits make sense instead of look arbitrary. This is
the highest-value page for a PLC audience and the one most likely to be skipped
under time pressure — do not skip it.

- A breakpoint in a PLC program fires *every scan*, not once. What "paused"
  means when the world outside the PLC keeps moving.
- Why `scanCount` is the clock you actually watch.
- What debug information is, that it lives in the container's debug section,
  and that it costs nothing at run time when no debugger is attached.
- Why forcing a variable is absent: the industrial "force" means "held across
  scans", and a paused-only write would be silently overwritten on the next
  scan — a half-answer here trains users to distrust the debugger.
- Why one program instance: a global breakpoint under multiple instances would
  report state from mixed scans.
- Cross-link `execution-cycle.rst` (which already sends readers to `ironplcvm`
  for "test and debug" and should now send them here too).

**Also update** `docs/explanation/index.rst` toctree.

### Cross-cutting: the landing page

`docs/index.rst:23` claims IronPLC "doesn't yet provide I/O mapping or debugging
capabilities". Correct it to name debugging as supported but early (scoped to
Structured Text, single-instance) and keep the I/O-mapping caveat. The landing
page states the maturity in its own prose rather than carrying the banner — an
admonition on the front page is heavier than that sentence needs. Check the landing grid
and `.. meta:: :description:` for the same claim. `docs/explanation/
ironplc-ecosystem.rst:20` cites debugging as something full IDEs have and should
be revisited in the same pass.

This edit is one line and it is the single highest-value change in the plan —
it is currently telling every visitor the feature does not exist.

## Screenshots

The docs use captured screenshots (`docs/images/screenshots/`) produced by the
Playwright/Electron harness in `integrations/vscode/src/screenshots/`. Four new
captures, added to `captureScreenshots.ts` with a fixture program under
`src/screenshots/fixtures/`:

| Image | Shows |
|-------|-------|
| `debug-breakpoint-hit.png` | Paused line highlighted with a breakpoint in the gutter |
| `debug-variables.png` | `Program` and `Runtime` scopes with named, typed values |
| `debug-call-stack.png` | Call stack with real POU names |
| `debug-toolbar.png` | The debug toolbar (only after the Step Scan Cycle question is settled) |

Every `.. figure::` needs an `:alt:`, as the existing pages do.

## Phasing

**Phase 0 — Correctness (ship alone, immediately).** Fix `docs/index.rst`; fix
`V6009`'s misleading solution; resolve the Step Scan Cycle button question.
Small, no new pages, removes an actively false statement.

**Phase 1 — Reference.** `reference/editor/debugging.rst`,
`reference/runtime/ironplcdap.rst`, the
`settings.rst`/`overview.rst`/`runtime/index.rst`/`ironplcc.rst` edits. Reference
first because the other three quadrants link into it.

**Phase 2 — How-to.** `debug-a-program.rst` and the `troubleshoot-editor.rst`
section. This is what a user searching "IronPLC breakpoint" needs.

**Phase 3 — Tutorial + screenshots.** `quickstart/debugging.rst` and the four
captures. Last because it is the most expensive to write and the most expensive
to keep correct, and because it needs the screenshots.

**Phase 4 — Explanation.** `explanation/debugging-a-scan-cycle.rst`.

**Phase 5 — Optional.** `debug-without-vs-code.rst`.

Phases 1–4 are each independently shippable.

## File map

**New**

- `docs/includes/debugging-in-development.rst` *(written)*
- `docs/quickstart/debugging.rst`
- `docs/how-to-guides/getting-started/debug-a-program.rst`
- `docs/how-to-guides/getting-started/debug-without-vs-code.rst` *(Phase 5)*
- `docs/reference/editor/debugging.rst`
- `docs/reference/runtime/ironplcdap.rst`
- `docs/explanation/debugging-a-scan-cycle.rst`
- `docs/images/screenshots/debug-*.png` (4 files)

**Modified**

- `docs/index.rst`
- `docs/quickstart/index.rst`
- `docs/how-to-guides/getting-started/index.rst`
- `docs/how-to-guides/troubleshoot-editor.rst`
- `docs/reference/editor/index.rst`
- `docs/reference/editor/overview.rst`
- `docs/reference/editor/settings.rst`
- `docs/reference/runtime/index.rst`
- `docs/reference/compiler/ironplcc.rst`
- `docs/reference/runtime/problems/V6009.rst`
- `docs/explanation/index.rst`
- `docs/explanation/execution-cycle.rst`
- `docs/explanation/ironplc-ecosystem.rst`
- `integrations/vscode/src/screenshots/captureScreenshots.ts` and a fixture

## Tasks

- [x] Write the shared banner `docs/includes/debugging-in-development.rst`
- [ ] Decide the Step Scan Cycle question (hide the button, or document the gap)
- [ ] Phase 0: fix the landing-page claim and the `V6009` solution text
- [ ] Phase 1: write the two reference pages, each opening with the banner
      include and stating its limitations inline
- [ ] Phase 1: apply the settings/overview/index/`ironplcc` reference edits
- [ ] Phase 2: write `debug-a-program.rst`; extend `troubleshoot-editor.rst`
- [ ] Phase 3: add screenshot captures and the fixture program
- [ ] Phase 3: write `quickstart/debugging.rst` and slot it into the toctree
- [ ] Phase 4: write `debugging-a-scan-cycle.rst`; cross-link `execution-cycle.rst`
- [ ] Phase 5 (optional): write `debug-without-vs-code.rst`
- [ ] Verify `cd docs && just compile` passes (`-W -n`: warnings are errors and
      every reference must resolve)
- [ ] Walk the tutorial end-to-end on a clean install before merging

## Verification

`docs/justfile` builds with `sphinx-build -a -W -n`, so an unreferenced page, a
broken `:doc:` link, or a missing toctree entry fails the build. That catches
structure but not accuracy — the tutorial has to be walked by hand.

## Non-goals

- Documenting the debugger's internals. `specs/design/debugger-support.md` owns
  that and must never move into `docs/`.
- Documenting deferred features (forcing, logpoints, watch expressions,
  multi-instance) as anything other than named limitations, stated inline where
  a user would look for them.
- Any page, section, or slug that exists only to hold the debugger's current
  maturity and would be deleted once the debugger matures.
- Adding new problem codes. `V6008`–`V6010` and `E0004`–`E0007` already cover
  every failure the debugger can report today.

## Open questions

1. **Step Scan Cycle** — hide the button or document it as a known limitation?
2. **Reference layout** — per-tool split (recommended, matches the existing
   section) or a standalone `reference/debugger/` section?
3. **Tutorial placement** — after `configuring.rst` (recommended: the program is
   complete and single-instance) or as a final optional chapter?
4. **The doorbell's `TON` instance.** The tutorial can route around unresolved FB
   fields (see above), but a learner who clicks `PulseTimer` open still sees
   something unhelpful. Accept that for now, or wait for FB field-name support?

Checked and settled: the quickstart program after `configuring.rst` declares
exactly one program instance (`PROGRAM plc_task_instance WITH plc_task : main`),
so it satisfies the v1 single-instance precondition.
