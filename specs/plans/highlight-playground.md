# Highlight IronPLC Playground

## Goal

Make the playground more discoverable through two approaches:
1. Add curated example programs to the playground UI itself
2. Add contextual "Try in playground" links in quick start tutorials

## Changes

### 1. Example programs dropdown in playground UI

**Files:** `playground/index.html`, `playground/app.js`, `playground/style.css`

Add a `<select>` dropdown in the toolbar (before the transport controls) with curated examples:

| Label | Description |
|-------|-------------|
| Counter | Current default — increment and double (keep as default) |
| Boolean Logic | AND/OR/NOT with BOOL variables |
| Arithmetic | Integer math with multiple operations |
| Timer Pattern | Use TON/TOF if supported, or manual time-based counting |
| Comparison | IF/THEN/ELSE with comparison operators |

When a user selects an example, it replaces the editor content. If the program is currently running, it stops first (same as manual edit behavior).

The dropdown should show "Examples" as the label/first option. Selecting an example replaces editor content but doesn't auto-run.

### 2. "Try in playground" tips in quick start tutorials

**Files:** `docs/quickstart/helloworld.rst`, `docs/quickstart/sense-control-actuate.rst`

Add a `.. tip::` admonition at the end of the "What This Program Does" section in each tutorial with a link to the playground pre-loaded with the tutorial's code. Use the playground URL parameter format (`?code=<base64>`).

For `helloworld.rst`: Link loads the counter program.
For `sense-control-actuate.rst`: The doorbell program uses I/O (`%IX1`, `%QX1`) which won't work in the playground since it runs a subset. Add a note that the playground doesn't support I/O variables, but link to the playground with a simplified version (just the boolean logic without AT addresses).

### 3. Tests

**File:** `playground/tests/e2e.spec.js`

Add test for the examples dropdown:
- Verify the dropdown is visible
- Verify selecting an example changes editor content
- Verify selecting an example while running stops execution
