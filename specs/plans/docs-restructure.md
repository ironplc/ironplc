# Documentation Restructure Plan

**Goal:** Restructure the website URL hierarchy to align with the Diátaxis framework by moving reference content under `/reference/` and relocating troubleshooting into how-to guides.

**Rationale:** The homepage already groups content by Diátaxis quadrants (Tutorials, How-to guides, Reference) in prose, but the URL structure does not reflect that organization — `/compiler/`, `/vscode/`, `/how-to-guides/`, and `/quickstart/` are all top-level peers. This restructure makes the hierarchy self-documenting and fixes a quadrant violation where troubleshooting (a how-to guide) lives under a reference URL.

**Tech Stack:** Sphinx (reStructuredText), Furo theme, custom `ironplc_problemcode` extension, Rust compiler crate, VS Code extension (TypeScript)

---

## Analysis

### Cross-references that need updating

Six files contain cross-references to paths that will change:

| File | Reference | New value |
|------|-----------|-----------|
| `docs/index.rst` | `:doc:\`compiler/index\`` | `:doc:\`reference/compiler/index\`` |
| `docs/index.rst` | `:doc:\`vscode/index\`` | `:doc:\`reference/vscode/index\`` |
| `docs/vscode/troubleshooting.rst` | `:doc:\`overview\`` | `:doc:\`/reference/vscode/overview\`` |
| `docs/vscode/troubleshooting.rst` | `:doc:\`problems/E0001\`` | `:doc:\`/reference/vscode/problems/E0001\`` |
| `docs/vscode/overview.rst` | `:doc:\`/compiler/problems/index\`` | `:doc:\`/reference/compiler/problems/index\`` |
| `docs/how-to-guides/check-beremiz-projects.rst` | `:doc:\`/compiler/source-formats/plcopen-xml\`` | `:doc:\`/reference/compiler/source-formats/plcopen-xml\`` |
| `docs/how-to-guides/check-twincat-projects.rst` | `:doc:\`/compiler/source-formats/twincat\`` | `:doc:\`/reference/compiler/source-formats/twincat\`` |

### Sphinx extension paths that need updating

The custom Sphinx extension `docs/extensions/ironplc_problemcode.py` has three hardcoded paths:

| Line | Current | New |
|------|---------|-----|
| 16 | `join('compiler', 'problems')` | `join('reference', 'compiler', 'problems')` |
| 17 | `join('vscode', 'problems')` | `join('reference', 'vscode', 'problems')` |
| 74 | `srcdir / 'compiler' / 'problems'` | `srcdir / 'reference' / 'compiler' / 'problems'` |

### Runtime documentation URLs that need updating

The compiler and VS Code extension construct URLs to the documentation website at runtime. These hardcoded URL bases must be updated:

| File | Line | Current | New |
|------|------|---------|-----|
| `compiler/plc2x/src/lsp_project.rs` | 452 | `https://www.ironplc.com/compiler/problems/{}.html` | `https://www.ironplc.com/reference/compiler/problems/{}.html` |
| `integrations/vscode/src/extension.ts` | 24 | `https://www.ironplc.com/vscode/problems/' + code + '.html` | `https://www.ironplc.com/reference/vscode/problems/' + code + '.html` |

### What is unaffected

- **Explicit `:ref:` targets** (`:ref:\`how to guides target\``, `:ref:\`installation steps target\``) — none are in documents being moved.
- **`autosectionlabel_prefix_document`** — all current cross-references use explicit targets, not auto-generated section labels.
- **Include directives** — `.. include:: ../includes/requires-compiler.rst` in how-to guides uses filesystem-relative paths and those guides aren't moving.
- **`docs/conf.py`** — no configuration changes needed.
- **`docs/reference/vscode/problems/E0001.rst`** — references `/quickstart/installation` which isn't moving.

---

## URL Changes

### Compiler reference → `/reference/compiler/`

| Before | After |
|--------|-------|
| `/compiler/` | `/reference/compiler/` |
| `/compiler/basicusage.html` | `/reference/compiler/basicusage.html` |
| `/compiler/source-formats/*` | `/reference/compiler/source-formats/*` |
| `/compiler/problems/*` | `/reference/compiler/problems/*` |

### VS Code extension reference → `/reference/vscode/`

| Before | After |
|--------|-------|
| `/vscode/` | `/reference/vscode/` |
| `/vscode/overview.html` | `/reference/vscode/overview.html` |
| `/vscode/settings.html` | `/reference/vscode/settings.html` |
| `/vscode/problems/*` | `/reference/vscode/problems/*` |

### Troubleshooting → how-to guides

| Before | After |
|--------|-------|
| `/vscode/troubleshooting.html` | `/how-to-guides/troubleshoot-vscode.html` |

### Unchanged

- `/quickstart/` — stays at current URL
- `/how-to-guides/` — structure unchanged (except gaining `troubleshoot-vscode.html`)

---

## Tasks

### Task 1: Move files with git mv

Create the `docs/reference/` directory and move files. The troubleshooting file must be moved before the vscode directory.

```
mkdir -p docs/reference
git mv docs/compiler docs/reference/compiler
git mv docs/vscode/troubleshooting.rst docs/how-to-guides/troubleshoot-vscode.rst
git mv docs/vscode docs/reference/vscode
```

**Result:**
```
docs/reference/compiler/          (was docs/compiler/)
docs/reference/vscode/            (was docs/vscode/, minus troubleshooting.rst)
docs/how-to-guides/troubleshoot-vscode.rst  (was docs/vscode/troubleshooting.rst)
```

### Task 2: Create `docs/reference/index.rst`

Create a new index page for the reference section to serve as the parent toctree node:

```rst
=========
Reference
=========

Technical reference material for IronPLC tools.

.. toctree::
   :maxdepth: 1

   Compiler <compiler/index>
   Visual Studio Code Extension <vscode/index>
```

### Task 3: Update `docs/index.rst`

**Toctree** — replace the two separate reference entries with a single reference entry and change `maxdepth` to 2 so the sidebar shows compiler and vscode children under Reference:

```rst
.. toctree::
   :maxdepth: 2
   :hidden:

   Quick start <quickstart/index>
   How-to guides <how-to-guides/index>
   Reference <reference/index>
```

**Grid card** — update the `:doc:` references in the Reference card:

```rst
* :doc:`reference/compiler/index`
* :doc:`reference/vscode/index`
```

### Task 4: Update `docs/how-to-guides/index.rst`

Change the troubleshooting toctree entry from a cross-directory absolute reference to a local file:

```rst
Troubleshoot the VS Code Extension <troubleshoot-vscode>
```

### Task 5: Update cross-references in moved/affected files

**`docs/how-to-guides/troubleshoot-vscode.rst`** (two relative references that relied on being inside `vscode/`):

- `:doc:\`overview\`` → `:doc:\`/reference/vscode/overview\``
- `:doc:\`problems/E0001\`` → `:doc:\`/reference/vscode/problems/E0001\``

**`docs/reference/vscode/overview.rst`**:

- `:doc:\`/compiler/problems/index\`` → `:doc:\`/reference/compiler/problems/index\``

**`docs/how-to-guides/check-beremiz-projects.rst`**:

- `:doc:\`/compiler/source-formats/plcopen-xml\`` → `:doc:\`/reference/compiler/source-formats/plcopen-xml\``

**`docs/how-to-guides/check-twincat-projects.rst`**:

- `:doc:\`/compiler/source-formats/twincat\`` → `:doc:\`/reference/compiler/source-formats/twincat\``

### Task 6: Update `docs/extensions/ironplc_problemcode.py`

Update the three hardcoded directory paths:

- Line 16: `join('compiler', 'problems')` → `join('reference', 'compiler', 'problems')`
- Line 17: `join('vscode', 'problems')` → `join('reference', 'vscode', 'problems')`
- Line 74: `srcdir / 'compiler' / 'problems'` → `srcdir / 'reference' / 'compiler' / 'problems'`

### Task 7: Update runtime documentation URLs

**`compiler/plc2x/src/lsp_project.rs`** (line 452):

```rust
// Before
"https://www.ironplc.com/compiler/problems/{}.html"
// After
"https://www.ironplc.com/reference/compiler/problems/{}.html"
```

**`integrations/vscode/src/extension.ts`** (line 24):

```typescript
// Before
'https://www.ironplc.com/vscode/problems/' + code + '.html'
// After
'https://www.ironplc.com/reference/vscode/problems/' + code + '.html'
```

### Task 8: Verify the build

```bash
cd docs && just compile
```

The build uses `-W -n` flags (warnings as errors, nitpicky mode), so any broken reference will cause a build failure.
