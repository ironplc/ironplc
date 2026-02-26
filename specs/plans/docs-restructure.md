# Documentation Restructure Plan

**Goal:** Restructure the website URL hierarchy to align with the Diátaxis framework by moving reference content under `/reference/` and relocating troubleshooting into how-to guides. Additionally, rename the `vscode` path segment to `editor` to be editor-agnostic.

**Rationale:** The homepage already groups content by Diátaxis quadrants (Tutorials, How-to guides, Reference) in prose, but the URL structure does not reflect that organization — `/compiler/`, `/vscode/`, `/how-to-guides/`, and `/quickstart/` are all top-level peers. This restructure makes the hierarchy self-documenting and fixes a quadrant violation where troubleshooting (a how-to guide) lives under a reference URL. The `vscode` → `editor` rename future-proofs the URL structure as the extension works in VS Code, Cursor, Windsurf, and other VS Code-based editors.

**Tech Stack:** Sphinx (reStructuredText), Furo theme, custom `ironplc_problemcode` extension, Rust compiler crate, VS Code extension (TypeScript)

---

## Analysis

### Cross-references that need updating

Six files contain cross-references to paths that will change:

| File | Reference | New value |
|------|-----------|-----------|
| `docs/index.rst` | `:doc:\`compiler/index\`` | `:doc:\`reference/compiler/index\`` |
| `docs/index.rst` | `:doc:\`vscode/index\`` | `:doc:\`reference/editor/index\`` |
| `docs/vscode/troubleshooting.rst` | `:doc:\`overview\`` | `:doc:\`/reference/editor/overview\`` |
| `docs/vscode/troubleshooting.rst` | `:doc:\`problems/E0001\`` | `:doc:\`/reference/editor/problems/E0001\`` |
| `docs/vscode/overview.rst` | `:doc:\`/compiler/problems/index\`` | `:doc:\`/reference/compiler/problems/index\`` |
| `docs/how-to-guides/check-beremiz-projects.rst` | `:doc:\`/compiler/source-formats/plcopen-xml\`` | `:doc:\`/reference/compiler/source-formats/plcopen-xml\`` |
| `docs/how-to-guides/check-twincat-projects.rst` | `:doc:\`/compiler/source-formats/twincat\`` | `:doc:\`/reference/compiler/source-formats/twincat\`` |

### Sphinx extension paths that need updating

The custom Sphinx extension `docs/extensions/ironplc_problemcode.py` has three hardcoded paths:

| Line | Current | New |
|------|---------|-----|
| 16 | `join('compiler', 'problems')` | `join('reference', 'compiler', 'problems')` |
| 17 | `join('vscode', 'problems')` | `join('reference', 'editor', 'problems')` |
| 74 | `srcdir / 'compiler' / 'problems'` | `srcdir / 'reference' / 'compiler' / 'problems'` |

### Runtime documentation URLs that need updating

The compiler and VS Code extension construct URLs to the documentation website at runtime. These hardcoded URL bases must be updated:

| File | Line | Current | New |
|------|------|---------|-----|
| `compiler/plc2x/src/lsp_project.rs` | 452 | `https://www.ironplc.com/compiler/problems/{}.html` | `https://www.ironplc.com/reference/compiler/problems/{}.html` |
| `integrations/vscode/src/extension.ts` | 24 | `https://www.ironplc.com/vscode/problems/' + code + '.html` | `https://www.ironplc.com/reference/editor/problems/' + code + '.html` |

### What is unaffected

- **Explicit `:ref:` targets** (`:ref:\`how to guides target\``, `:ref:\`installation steps target\``) — none are in documents being moved.
- **`autosectionlabel_prefix_document`** — all current cross-references use explicit targets, not auto-generated section labels.
- **Include directives** — `.. include:: ../includes/requires-compiler.rst` in how-to guides uses filesystem-relative paths and those guides aren't moving.
- **`docs/conf.py`** — no configuration changes needed.
- **`docs/reference/editor/problems/E0001.rst`** — references `/quickstart/installation` which isn't moving.
- **Page titles and prose** — documents still say "Visual Studio Code Extension"; only URL path segments change.

---

## URL Changes

### Compiler reference → `/reference/compiler/`

| Before | After |
|--------|-------|
| `/compiler/` | `/reference/compiler/` |
| `/compiler/basicusage.html` | `/reference/compiler/basicusage.html` |
| `/compiler/source-formats/*` | `/reference/compiler/source-formats/*` |
| `/compiler/problems/*` | `/reference/compiler/problems/*` |

### VS Code extension reference → `/reference/editor/`

| Before | After |
|--------|-------|
| `/vscode/` | `/reference/editor/` |
| `/vscode/overview.html` | `/reference/editor/overview.html` |
| `/vscode/settings.html` | `/reference/editor/settings.html` |
| `/vscode/problems/*` | `/reference/editor/problems/*` |

### Troubleshooting → how-to guides

| Before | After |
|--------|-------|
| `/vscode/troubleshooting.html` | `/how-to-guides/troubleshoot-editor.html` |

### Unchanged

- `/quickstart/` — stays at current URL
- `/how-to-guides/` — structure unchanged (except gaining `troubleshoot-editor.html`)

---

## Tasks

### Task 1: Move and rename files with git mv

Create the `docs/reference/` directory and move files. The troubleshooting file must be moved before the vscode directory.

```
mkdir -p docs/reference
git mv docs/compiler docs/reference/compiler
git mv docs/vscode/troubleshooting.rst docs/how-to-guides/troubleshoot-editor.rst
git mv docs/vscode docs/reference/editor
```

**Result:**
```
docs/reference/compiler/          (was docs/compiler/)
docs/reference/editor/            (was docs/vscode/, minus troubleshooting.rst)
docs/how-to-guides/troubleshoot-editor.rst  (was docs/vscode/troubleshooting.rst)
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
   Editor Extension <editor/index>
```

### Task 3: Update `docs/index.rst`

**Toctree** — replace the two separate reference entries with a single reference entry and change `maxdepth` to 2 so the sidebar shows compiler and editor children under Reference:

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
* :doc:`reference/editor/index`
```

### Task 4: Update `docs/how-to-guides/index.rst`

Change the troubleshooting toctree entry from a cross-directory absolute reference to a local file:

```rst
Troubleshoot the Editor Extension <troubleshoot-editor>
```

### Task 5: Update cross-references in moved/affected files

**`docs/how-to-guides/troubleshoot-editor.rst`** (two relative references that relied on being inside `vscode/`):

- `:doc:\`overview\`` → `:doc:\`/reference/editor/overview\``
- `:doc:\`problems/E0001\`` → `:doc:\`/reference/editor/problems/E0001\``

**`docs/reference/editor/overview.rst`**:

- `:doc:\`/compiler/problems/index\`` → `:doc:\`/reference/compiler/problems/index\``

**`docs/how-to-guides/check-beremiz-projects.rst`**:

- `:doc:\`/compiler/source-formats/plcopen-xml\`` → `:doc:\`/reference/compiler/source-formats/plcopen-xml\``

**`docs/how-to-guides/check-twincat-projects.rst`**:

- `:doc:\`/compiler/source-formats/twincat\`` → `:doc:\`/reference/compiler/source-formats/twincat\``

### Task 6: Update `docs/extensions/ironplc_problemcode.py`

Update the three hardcoded directory paths:

- Line 16: `join('compiler', 'problems')` → `join('reference', 'compiler', 'problems')`
- Line 17: `join('vscode', 'problems')` → `join('reference', 'editor', 'problems')`
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
'https://www.ironplc.com/reference/editor/problems/' + code + '.html'
```

### Task 8: Generate redirect pages for old problem code URLs

Older versions of the compiler and editor extension hardcode URLs to `/compiler/problems/P*.html` and `/vscode/problems/E*.html`. Users who haven't updated will still follow those links, so the old URLs must redirect to the new locations. The same applies to anyone who bookmarked a problem code page.

Add a `generate_redirects` function to `docs/extensions/ironplc_problemcode.py` that hooks into Sphinx's `build-finished` event. It scans the build output for problem code HTML files at their new paths and creates minimal redirect pages at the old paths.

**Redirect mapping:**

| Old path (in `_build/`) | New path (in `_build/`) |
|---|---|
| `compiler/problems/P*.html` | `reference/compiler/problems/P*.html` |
| `compiler/problems/index.html` | `reference/compiler/problems/index.html` |
| `vscode/problems/E*.html` | `reference/editor/problems/E*.html` |
| `vscode/problems/index.html` | `reference/editor/problems/index.html` |

**Redirect HTML template** — each generated file uses both `<meta http-equiv="refresh">` (works without JS) and a JavaScript redirect for immediate navigation:

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Redirecting…</title>
  <link rel="canonical" href="/reference/compiler/problems/P0001.html">
  <meta http-equiv="refresh" content="0; url=/reference/compiler/problems/P0001.html">
  <script>window.location.replace('/reference/compiler/problems/P0001.html');</script>
</head>
<body>
  <p>This page has moved to
     <a href="/reference/compiler/problems/P0001.html">/reference/compiler/problems/P0001.html</a>.</p>
</body>
</html>
```

**Implementation** — add to `ironplc_problemcode.py`:

```python
REDIRECT_TEMPLATE = """\
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Redirecting\u2026</title>
  <link rel="canonical" href="/{new_path}">
  <meta http-equiv="refresh" content="0; url=/{new_path}">
  <script>window.location.replace('/{new_path}');</script>
</head>
<body>
  <p>This page has moved to <a href="/{new_path}">/{new_path}</a>.</p>
</body>
</html>
"""

# Old path prefix -> new path prefix (relative to build output root)
PROBLEM_REDIRECTS = [
    ('compiler/problems', 'reference/compiler/problems'),
    ('vscode/problems',   'reference/editor/problems'),
]

def generate_redirects(app, exception):
    """Generate redirect HTML pages at old URLs after a successful build."""
    if exception:
        return

    from sphinx.util import logging
    logger = logging.getLogger(__name__)

    outdir = Path(app.outdir)

    for old_prefix, new_prefix in PROBLEM_REDIRECTS:
        new_dir = outdir / new_prefix
        if not new_dir.exists():
            logger.warning(f"Expected output directory not found: {new_dir}")
            continue

        old_dir = outdir / old_prefix
        old_dir.mkdir(parents=True, exist_ok=True)

        for html_file in new_dir.glob('*.html'):
            new_path = f"{new_prefix}/{html_file.name}"
            redirect_file = old_dir / html_file.name
            redirect_file.write_text(
                REDIRECT_TEMPLATE.format(new_path=new_path),
                encoding='utf-8',
            )

        logger.info(f"Generated redirects: {old_prefix}/ -> {new_prefix}/")
```

Register the event handler in `setup()`:

```python
app.connect('build-finished', generate_redirects)
```

### Task 9: Add tests that documentation paths match runtime URLs

The runtime URLs in Task 7 are plain strings with no connection to the Sphinx build. Add tests that assert the URL path segments correspond to actual directories containing `.rst` files, so future path moves cause a test failure.

**Rust test** — add to the `#[cfg(test)] mod test` block in `compiler/plc2x/src/lsp_project.rs`:

Extract the URL path segment `reference/compiler/problems` into a constant (or use it inline in the test). The test navigates from `CARGO_MANIFEST_DIR` (`compiler/plc2x/`) up to the repo root and asserts the docs directory exists and contains problem code `.rst` files:

```rust
#[test]
fn map_diagnostic_when_problem_code_url_then_docs_directory_exists() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // compiler/plc2x/ -> repo root
    path.push("../..");
    path.push("docs/reference/compiler/problems");
    assert!(path.is_dir(), "Documentation directory for compiler problem codes does not exist: {}", path.display());

    // Verify at least one problem code .rst file exists
    let has_problem_files = std::fs::read_dir(&path)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with('P') && name.ends_with(".rst")
        });
    assert!(has_problem_files, "No P*.rst files found in {}", path.display());
}
```

**TypeScript test** — add a new file `integrations/vscode/src/test/unit/problemUrls.test.ts`:

The test navigates from the test file's location up to the repo root and asserts the docs directory exists with E-code `.rst` files:

```typescript
import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';

suite('problemUrls', () => {
  test('openProblemInBrowser_when_url_path_then_docs_directory_exists', () => {
    // From out/test/unit/ -> repo root is 5 levels up (out/test/unit -> out/test -> out -> vscode -> integrations -> root)
    const repoRoot = path.resolve(__dirname, '..', '..', '..', '..', '..');
    const docsDir = path.join(repoRoot, 'docs', 'reference', 'editor', 'problems');
    assert.ok(fs.existsSync(docsDir), `Documentation directory does not exist: ${docsDir}`);

    const files = fs.readdirSync(docsDir);
    const hasErrorFiles = files.some(f => f.startsWith('E') && f.endsWith('.rst'));
    assert.ok(hasErrorFiles, `No E*.rst files found in ${docsDir}`);
  });
});
```

### Task 10: Verify the build


```bash
cd docs && just compile
```

The build uses `-W -n` flags (warnings as errors, nitpicky mode), so any broken reference will cause a build failure.
