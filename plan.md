# Plan: Capability Flags Documentation

## Problem

Edition 3 features (LTIME, LDATE, etc.) have duplicated `.. note::` blocks that hardcode CLI instructions. This doesn't scale as more flags are added, doesn't mention VS Code settings, and gives no broader context about why flags exist.

## Approach

Three new docs artifacts + updates to existing pages. The per-page notes stay **minimal** and link to a centralized explanation page. This keeps individual feature pages clean and lets the explanation page handle the full complexity (multiple flags, CLI vs VS Code, future editions).

### 1. New explanation page: `docs/explanation/language-support.rst`

**Brief** page covering:
- IEC 61131-3 has multiple editions (Edition 2 = 2003 baseline, Edition 3 = 2013)
- IronPLC defaults to Edition 2 for maximum portability
- Some features require enabling specific capabilities via flags/settings
- How to enable capabilities:
  - **CLI**: `--std-iec-61131-3=2013`
  - **VS Code**: `ironplc.std61131Version` setting
- Brief note that this will expand (more editions, more flags)
- Link to the edition support matrix for the full list

This is the single page that explains "I got an error about editions, what do I do?" It covers both CLI and VS Code in one place.

Add to `docs/explanation/index.rst` toctree after "What is IEC 61131-3?"

### 2. New reference page: `docs/reference/language/edition-support.rst`

A feature-to-capability matrix. Table with columns:
- Feature name (linked to its reference page)
- Category (Data type, etc.)
- Required capabilities (e.g., "Edition 3 (2013)")

This is information-oriented — no how-to instructions. Just "what needs what." Links back to the explanation page for how to enable capabilities.

Add to `docs/reference/language/index.rst` toctree.

### 3. New include: `docs/includes/requires-edition3.rst`

Minimal note that links to the explanation page:

```rst
.. note::

   This feature requires IEC 61131-3 Edition 3.
   See :doc:`/explanation/language-support` for how to enable it.
```

Deliberately simple — no CLI/VS Code instructions inline. The explanation page handles that. Future includes (e.g., `requires-edition4.rst` or `requires-some-other-flag.rst`) follow the same pattern.

### 4. Update existing pages

- **LTIME, LDATE, LTIME_OF_DAY, LDATE_AND_TIME**: Replace duplicated `.. note::` blocks with `.. include:: ../../../includes/requires-edition3.rst`
- **Settings reference** (`docs/reference/editor/settings.rst`): Add the missing `ironplc.std61131Version` setting documentation
- **P0010**: Add mention of VS Code setting as alternative fix, link to explanation page
- **Data types index**: Link "(Edition 3)" markers to the edition support matrix

### File changes summary

| File | Action |
|------|--------|
| `docs/explanation/language-support.rst` | Create |
| `docs/explanation/index.rst` | Add toctree entry |
| `docs/reference/language/edition-support.rst` | Create |
| `docs/reference/language/index.rst` | Add toctree entry |
| `docs/includes/requires-edition3.rst` | Create |
| `docs/reference/language/data-types/ltime.rst` | Replace note with include |
| `docs/reference/language/data-types/ldate.rst` | Replace note with include |
| `docs/reference/language/data-types/ltime-of-day.rst` | Replace note with include |
| `docs/reference/language/data-types/ldate-and-time.rst` | Replace note with include |
| `docs/reference/editor/settings.rst` | Add std61131Version setting |
| `docs/reference/compiler/problems/P0010.rst` | Add VS Code setting + link to explanation |
| `docs/reference/language/data-types/index.rst` | Link Edition 3 markers to matrix |
