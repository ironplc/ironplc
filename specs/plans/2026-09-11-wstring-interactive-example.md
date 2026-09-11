# Interactive Example for the WSTRING Data Type Page

Issue: https://github.com/ironplc/ironplc/issues/1684

## Goal

`docs/reference/language/data-types/elementary/wstring.rst` is the only one of
the twenty-five elementary data type pages without a `playground-with-program`
example, so the data types index tip ("Examples on supported data type pages are
interactive") is wrong for exactly that page. Give WSTRING one interactive
example that shows what distinguishes it from `STRING` — the wide encoding — and
flip ADR-0018 to `accepted`.

## Architecture

The example follows ADR-0018's design principles: one concept for the type
family (text outside ASCII, measured in code units), three variables, three
statements, typed literals matching the page's Literals section, and names from
an HMI caption scenario. `CONCAT` and `LEN` over a `WSTRING` are covered
end to end by `compiler/codegen/tests/it/end_to_end_wstring.rs`, so the example
runs.

## Prefactoring

The wide encoding cannot be shown without a character outside ASCII, and today
such a character does not survive the trip into the playground: the docs
directive base64-encodes UTF-8 (`b64encode(code.encode())` in
`docs/extensions/ironplc_playground.py`) but `playground/src/app.ts` decodes
with bare `atob`, which yields one character per *byte*. `"°C"` arrives in the
editor as `"Â°C"`. Fix that decode first — the example is only honest once a
non-ASCII literal loads as written.

No simplification of the RST page itself is needed; the change is one new
section in the shape the other twenty-four pages already use.

## File map

- `playground/src/app.ts` — decode the `code` and `vars` parameters as UTF-8
- `playground/tests/e2e.spec.ts` — a non-ASCII `code` parameter loads as written
- `docs/reference/language/data-types/elementary/wstring.rst` — the example
- `compiler/playground/src/lib.rs` — drop the stale "WSTRING is not yet
  implemented" claim from the `valid` field's doc comment; the shared
  `VariableRenderer` renders WSTRING, so the playground shows its value
- `specs/adrs/0018-interactive-data-type-examples.md` — `status: accepted`, plus
  a dated postscript for the premise that STRING/WSTRING/date types are
  unsupported, which has since become false

## Tasks

- [ ] Prefactor: UTF-8 base64 decode in the playground, with an e2e test
- [ ] Add the Example section to `wstring.rst`
- [ ] Verify the example compiles and runs (`ironplcc` + `ironplcvm --dump-vars`)
- [ ] Fix the stale WSTRING comment in the playground crate
- [ ] Flip ADR-0018 to `accepted` and append the dated postscript
- [ ] `cd docs && just compile`; `cd compiler && just`
- [ ] `git rm` this plan
