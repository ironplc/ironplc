# Playground Line Number Gutter

## Goal

Add an automatic line-number gutter to the left of the playground code editor so users can easily reference lines while writing or reading a program.

## Context

The playground (`/playground/`) uses a plain `<textarea id="editor">` — no CodeMirror, Monaco, or other editor library. Today there is no visible line numbering. Diagnostics surface raw byte offsets (`app.js:649`), which are hard to correlate back to source lines. A visual gutter is the first step toward readable line references.

## Changes

### 1. Wrap the textarea with a gutter

**File:** `playground/index.html`

Wrap `<textarea id="editor">` in `<div class="editor-wrapper">` and add a sibling gutter `<div>` before the textarea:

```html
<div class="editor-wrapper">
  <div class="editor-gutter" id="editor-gutter" aria-hidden="true"></div>
  <textarea id="editor" spellcheck="false" wrap="off" data-testid="editor">...</textarea>
</div>
```

`wrap="off"` disables soft-wrap so each `\n` in the source corresponds to exactly one visual line — line numbers always align with their source line.

### 2. Gutter styling

**File:** `playground/style.css`

- `.editor-wrapper`: flex container that fills `.editor-section`.
- `.editor-gutter`: same font stack, size, and line-height as `#editor` so numbers align; right-aligned muted color; border-right separator; `user-select: none`; `overflow: hidden` (scroll is driven by the textarea).
- `#editor`: add `overflow: auto` and `white-space: pre` so it scrolls horizontally when `wrap="off"`.
- `body.embed .editor-gutter`: shrink padding/font to match `body.embed #editor`.

### 3. Render and sync line numbers

**File:** `playground/app.js`

- `renderLineNumbers()` helper: counts `editor.value.split("\n").length`, writes `"1\n2\n…\nN"` into the gutter, and sets `minWidth` in `ch` based on digit count so the gutter widens for 3+ digit line counts.
- Called once at startup (after URL-param handling), in the `editor` `input` listener, and in the examples-select `change` listener.
- `editor` `scroll` listener mirrors `scrollTop` to the gutter so numbers stay aligned during vertical scroll.

### 4. Tests

**File:** `playground/tests/e2e.spec.js`

- `editor_when_loaded_then_shows_line_number_gutter`: gutter is visible, first line shows `"1"`, line count matches the default program's `\n` count.
- `editor_when_content_changed_then_line_numbers_update`: filling the editor with a 3-line program produces gutter text `"1\n2\n3"`.

## Out of scope

- Mapping diagnostic byte offsets to line numbers / clickable "jump to line".
- Current-line highlighting.
- Replacing the textarea with CodeMirror/Monaco.
