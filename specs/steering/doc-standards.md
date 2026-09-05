## Documentation Standards

### Documentation Quadrants Framework
All IronPLC documentation follows the **Documentation Quadrants** approach, organizing content into four distinct types:

#### 1. Tutorials (Learning-Oriented)
- **Purpose**: Guide newcomers through their first successful experience
- **Audience**: People studying and learning
- **Content**: Step-by-step lessons that work reliably
- **Examples**: "Getting Started with IronPLC", "Your First PLC Program"
- **Location**: `docs/quickstart/`

#### 2. How-To Guides (Problem-Oriented)
- **Purpose**: Show how to solve specific real-world problems
- **Audience**: Practitioners at work who need to accomplish something
- **Content**: Series of steps focused on achieving a goal
- **Examples**: "How to Debug Compilation Errors", "How to Add a New Data Type"
- **Location**: `docs/how-to-guides/`

#### 3. Technical Reference (Information-Oriented)
- **Purpose**: Describe the machinery and how to operate it
- **Audience**: Practitioners at work who need accurate information
- **Content**: Structured descriptions of APIs, commands, and features
- **Examples**: "Compiler CLI Reference", "Problem Code Reference", "Language Grammar"
- **Location**: `docs/reference/`

#### 4. Explanation (Understanding-Oriented)
- **Purpose**: Clarify and illuminate topics for deeper understanding
- **Audience**: People studying who want to understand concepts
- **Content**: Discussions of design decisions, alternatives, and context
- **Examples**: "IEC 61131-3 Compliance Strategy", "Compiler Architecture Overview"
- **Location**: `docs/explanation/`

### Documentation Relationships
- **Tutorials + How-To Guides**: Both describe practical steps
- **How-To Guides + Reference**: Both serve practitioners at work
- **Reference + Explanation**: Both provide theoretical knowledge
- **Tutorials + Explanation**: Both support learning and study

### Writing Style

All documentation follows these writing style principles to keep content clear, direct, and easy to scan.

#### Voice and Tense

- **Use active voice.** Name the actor (the compiler, the runtime, IEC 61131-3, the user) as the subject of the sentence.
  - ✅ "Edition 3 introduced the following features"
  - ❌ "The following features were introduced in Edition 3"
  - ✅ "IronPLC supports the following platforms"
  - ❌ "IronPLC is supported on the following platforms"
- **Use present tense.** Describe what the software *does*, not what it *will do* or *did*.
  - ✅ "The compiler reports an error when..."
  - ❌ "The compiler will report an error when..."
- **Passive voice is acceptable** in two cases:
  1. The actor is genuinely unknown or irrelevant (e.g., "the file may be corrupted").
  2. Active voice would blame the user for an error (e.g., "This error occurs when a comment is not properly closed" is better than "You forgot to close the comment").

#### Person and Mood

- **Use second person ("you")** in tutorials and how-to guides.
- **Use imperative mood** for instructions: "Run the command", not "You should run the command" or "The command can be run".
- **Avoid third-person references to the reader**: "the user", "the developer", "one".

#### Sentence Structure

- **Lead with the action or result**, not the context. Put the most important information first.
  - ✅ "Set the ``--std`` flag to enable Edition 3 features."
  - ❌ "In order to enable Edition 3 features, you need to set the ``--std`` flag."
- **Keep sentences short.** Prefer one idea per sentence. If a sentence has more than one comma-separated clause, consider splitting it.
- **Avoid nominalizations.** Use verbs instead of noun forms of verbs.
  - ✅ "The compiler validates the program"
  - ❌ "The compiler performs validation of the program"

### RST Annotation Conventions

All Sphinx documentation must use the correct RST roles for consistent rendering. **Never use plain text or double backticks for elements that have a dedicated role.**

| Element | Role | Example |
|---------|------|---------|
| Menu paths | `:menuselection:` | `:menuselection:\`File --> New File...\`` |
| UI elements (buttons, panels) | `:guilabel:` | `:guilabel:\`Install\`` |
| Keyboard shortcuts | `:kbd:` | `:kbd:\`Ctrl+Shift+P\`` |
| File names and extensions | `:file:` | `:file:\`main.st\``, `:file:\`.st\`` |
| Commands and executables | `:program:` | `:program:\`ironplcc --version\`` |
| Code keywords | `:code:` | `:code:\`PROGRAM\`` |
| User-typed text | `:samp:` | `:samp:\`IronPLC\`` |
| Cross-document links | `:doc:` | `:doc:\`/reference/compiler/problems/index\`` |

**Menu paths** use ` --> ` as separator: `:menuselection:\`File --> Preferences --> Settings\``

**Platform-specific keyboard shortcuts** use separate `:kbd:` roles: `:kbd:\`Ctrl+Shift+X\`` for Windows/Linux, `:kbd:\`⌘+Shift+X\`` for macOS.

### Interactive Examples in Documentation

Documentation pages that include IEC 61131-3 code examples **should** use interactive playground directives instead of static `.. code-block::` when the example is a valid, compilable program or snippet. This lets readers edit and run the code directly in the browser.

Two Sphinx directives are available (defined in `docs/extensions/ironplc_playground.py`):

| Directive | Use when |
|-----------|----------|
| `.. playground::` | The example is a complete program (includes `PROGRAM`/`END_PROGRAM`) |
| `.. playground-with-program::` | The example is a code snippet that should be auto-wrapped in a `PROGRAM` scaffold |

**`playground-with-program` options:**
- `:vars:` — semicolon-separated variable declarations (e.g., `:vars: result : DINT; value : REAL;`)
- `:height:` — custom iframe height (auto-calculated by default)

**Example** (from a standard library function page):
```rst
.. playground-with-program::
   :vars: result : DINT;

   result := ABS(-42);    (* result = 42 *)
```

**When NOT to use playground directives:**
- Problem code documentation (`docs/reference/compiler/problems/`) — these show invalid code that would fail compilation
- Partial syntax fragments that are not runnable

**Source of truth for the playground:**
- Frontend: `playground/` (HTML/JS/CSS single-page app)
- WASM compiler crate: `compiler/playground/` (Rust compiled to WebAssembly via wasm-pack)
- Sphinx extension: `docs/extensions/ironplc_playground.py` (directive implementation)
- Build system: `playground/justfile`

### Documentation Content Guidelines

- **Describe features as they are, not as future plans.** The website documents current capabilities. Do not include forward-looking statements like "this page will be updated when..." or "future versions will support...". If a feature is not yet supported, say so plainly (e.g., "Not yet supported") without speculating about when it will be added.
- **Do not document architecture or internals** in user-facing reference docs. Architecture belongs in `docs/explanation/` if anywhere.
- **Do not explain standard VS Code concepts** (e.g., workspace vs. user settings). Assume the reader knows VS Code.
- **Use platform tabs** (via `sphinx_inline_tabs`) for platform-specific instructions.

### Edition-Gated Features

Some IEC 61131-3 features require the user to enable a specific edition of the standard. When documenting an edition-gated feature:

1. **Use the reusable include** — add `.. include:: ../../../includes/requires-edition3.rst` near the top of the page (after the description, before the detail table). Do not write a custom note.
2. **Link to the edition support matrix** — in the feature's detail table, use `:doc:\`Edition 3 </reference/language/edition-support>\`` in the Support row instead of hardcoding the flag name.
3. **Update the matrix** — add the feature to `docs/reference/language/edition-support.rst`.

The centralized explanation page at `docs/explanation/enabling-dialects-and-features.rst` covers how to enable editions in both the CLI and VS Code. Individual feature pages link there rather than duplicating instructions.

### Problem Documentation Format
Each problem code has a corresponding `.rst` file under
`docs/reference/compiler/problems/` (P-codes),
`docs/vscode/problems/` (E-codes), or `docs/reference/runtime/problems/`
(V-codes). The `.rst` templates and the full add-a-code lifecycle live in
[problem-code-management.md](problem-code-management.md) — follow the template
there rather than an inline copy.

### Supported File Format Synchronization
File extensions and format details are listed in **two** canonical locations. All other docs cross-reference these rather than repeating extension lists. When adding or modifying a supported source file format, update:

1. **Compiler source** - `compiler/sources/src/file_type.rs` (the source of truth for detection)
2. **VS Code extension** - `integrations/vscode/package.json` (language contributions) and `integrations/vscode/src/extension.ts` (document selector)
3. **Source format reference page** - the format-specific page in `docs/reference/compiler/source-formats/` (e.g., `twincat.rst`)
4. **Editor overview** - `docs/reference/editor/overview.rst` (Supported Languages section)

### Example Synchronization
**Important**: Examples in documentation should also exist as tests in the Rust compiler to ensure documentation accuracy. Follow the existing naming conventions for test examples.