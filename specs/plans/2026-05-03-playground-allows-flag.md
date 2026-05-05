# Plan: Allow per-example feature flags in the playground

## Goal

Let documentation embed the IronPLC Playground with extra `--allow-*`
feature flags layered on top of the selected dialect. The motivating use
case is the SIZEOF example in
`docs/reference/extension-library/functions/sizeof.rst`, which needs
`allow_sizeof` to compile under a strict dialect. Mirror the existing
`dialect` URL parameter and Sphinx directive option, and surface the
active flags in the toolbar so embedders can see what is enabled.

## Approach

Extend the existing dialect plumbing with a parallel `allows` channel:

- A comma-separated list of feature short names (the part after
  `--allow-`), e.g. `sizeof,c-style-comments`.
- Threaded as a URL parameter, web-worker message field, WASM function
  argument, and `:allows:` Sphinx directive option.
- WASM looks each name up in `CompilerOptions::FEATURE_DESCRIPTORS`
  and toggles the matching field on top of the dialect preset.
- When `allows` is non-empty, the toolbar shows a "Custom" badge whose
  `title` attribute lists the enabled flags so a hover reveals them.

## File Map

| File | Change |
|------|--------|
| `compiler/playground/src/lib.rs` | New `compiler_options_from` helper that takes both dialect and allows; threaded into `compile`, `run_source`, `load_program` exports. New tests for the allows parameter. |
| `playground/worker.js` | Forward `allows` from message into WASM calls. |
| `playground/app.js` | Read `allows` URL param, pass it through `postCommand`, show "Custom" badge with hover listing the flags. |
| `playground/index.html` | (Reuses existing `dialect-badge` element — no markup change.) |
| `docs/extensions/ironplc_playground.py` | Accept `:allows:` option on all three directives; add `allows=` to the playground URL. |
| `docs/reference/extension-library/functions/sizeof.rst` | Declare `s : DINT` and add `:allows: sizeof` so the example compiles. |

## Tasks

- [x] Write plan
- [ ] Extend WASM `compile`, `run_source`, `load_program` to accept `allows`
- [ ] Forward `allows` through `worker.js`
- [ ] Wire `allows` URL param + custom badge in `app.js`
- [ ] Add `:allows:` option to Sphinx directives
- [ ] Fix the SIZEOF example to compile
- [ ] Run full CI (`cd compiler && just`)
