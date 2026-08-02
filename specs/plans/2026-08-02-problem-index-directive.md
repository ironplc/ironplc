# Plan: Auto-generate problem-code indexes via a directive

## Problem

`docs/reference/{compiler,runtime}/problems/index.rst` are auto-generated at
build time by the `ironplc_problemcode` extension (the `config-inited` hook
writes them into the source tree), yet the generated output is also committed
to git. Every branch that adds a problem-code page regenerates the committed
index, so two such branches conflict on the same toctree list. The editor
index is maintained by hand and drifts for the same reason.

## Approach

Stop materializing a file. Replace the generated content with a Sphinx
directive, `.. problem-index:: <prefix>`, that enumerates the sibling
`<prefix>####.rst` pages and emits the `toctree` node at parse time. The
committed `index.rst` becomes a stable 4-line stub (title + directive) that
never needs updating and cannot conflict.

## Changes

1. `docs/extensions/ironplc_problemcode.py`
   - Add a `ProblemIndex` directive that builds a `toctree` node from the
     sibling problem pages, sorted by numeric code.
   - Register it in `setup`.
   - Remove `generate_problem_index` / `_generate_index_for` and the
     `config-inited` connection (no longer writing files into srcdir).
2. Replace the three committed indexes with stubs using the directive:
   - `reference/compiler/problems/index.rst`  (`P`)
   - `reference/editor/problems/index.rst`    (`E`)
   - `reference/runtime/problems/index.rst`   (`V`)

## Verification

- `cd docs && just ci` builds clean with `-W -n` (warnings-as-errors,
  nitpicky) — the toctree resolves and every problem page is reachable.
- The rendered index lists the same codes in the same order as before.
