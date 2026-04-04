# Add Extension Library Documentation Section

## Context

IronPLC currently documents vendor extensions (like SIZEOF) mixed into the
Standard Library section. The Standard Library should only contain IEC 61131-3
standard items. This change creates a separate "Extension Library" section at
the same level as "Standard Library" under Reference.

## Changes

1. Create `docs/reference/extension-library/` with subsections for Functions,
   Function Blocks, and Variables
2. Move SIZEOF from `standard-library/functions/` to `extension-library/functions/`
3. Add system uptime variable documentation (`__SYSTEM_UP_TIME`, `__SYSTEM_UP_LTIME`)
   to the new Variables section
4. Remove the "Vendor Extensions" subsection from the standard library functions index
5. Add "Extension Library" to `docs/reference/index.rst` toctree
