# Assign stable error codes to playground host/embedding-layer errors

Closes #1201. Follow-up to #1200.

## Goal

Give every host/embedding-layer error surfaced by the playground WASM wrapper
(`compiler/playground/src/lib.rs`) a stable, documented error code, so that
every UI-visible error carries a code (analytics aggregation, doc anchors) and
the empty-code special-case in `renderDiagnostics` can be removed.

## Decision: the `H####` (Host) family

The existing taxonomies do not fit: `P####` are compiler problems, `E####` are
editor problems, `V####` are VM traps. Host/embedding-layer errors are a new
category, so they get a new family `H####` generated through the same
`resources/problem-codes.csv` → `build.rs` → `docs/reference/.../problems/*.rst`
pipeline the V-codes use.

Split by audience, mirroring the V4xxx (user) / V9xxx (internal) convention:

- **`H1xxx` — user-facing.** The caller supplied input the host cannot process;
  the user can act on it.
- **`H9xxx` — internal / contract violation.** A frontend↔WASM contract
  violation or an internal failure that should never reach an end user.

### Assignments

| Code  | Name                  | Class    | Site(s) in `lib.rs`                                   |
|-------|-----------------------|----------|------------------------------------------------------|
| H1001 | InvalidBase64         | user     | `run_inner` base64 decode failure                    |
| H1002 | InvalidContainer      | user     | `run_bytes` container read (user-supplied `.iplc`)   |
| H9001 | BytecodeLoadFailed    | internal | `load_program_inner` + `step_inner` container read of host-produced bytes |
| H9002 | NoProgramLoaded       | internal | `step_inner` with no session                         |
| H9003 | SessionFaulted        | internal | `step_inner` after a fault                           |
| H9004 | SerializationError    | internal | serde-to-JSON fallback in every `#[wasm_bindgen]` entry point (incl. the `compile` fallback that currently uses the `"INTERNAL"` pseudo-code) |
| H9005 | BytecodeSerializeFailed | internal | `compile_inner` `container.write_to` failure (currently `"INTERNAL"`) |

The undocumented `"INTERNAL"` pseudo-code is removed entirely so the taxonomy is
complete.

## Architecture

- New `compiler/playground/resources/problem-codes.csv` (`Code,Name,Message`),
  same shape as the vm-cli I/O-code CSV.
- New `compiler/playground/build.rs` generating `host_codes.rs` with
  `pub const <SCREAMING_SNAKE>: &str = "H####";` constants (PascalCase→SNAKE, as
  vm-cli does). Included via a `host_codes` module in `lib.rs`.
- `lib.rs` populates `RunError.code` / `DiagnosticInfo.code` at each site using
  the generated constants.
- Docs: new `reference/playground/` section (`index`, `problems/index` with
  `.. problem-index:: H`, and one page per code). `ironplc_problemcode.py` learns
  the `playground` section + `H` prefix.
- Front end: `renderDiagnostics` gains an `H####` → `playground` section link and
  drops the empty-code special-case; `RunError.code` is now always present.

## File map

- create `compiler/playground/resources/problem-codes.csv`
- create `compiler/playground/build.rs`
- modify `compiler/playground/Cargo.toml` (`[build-dependencies] csv`)
- modify `compiler/playground/src/lib.rs`
- create `docs/reference/playground/index.rst`
- create `docs/reference/playground/problems/index.rst`
- create `docs/reference/playground/problems/H1001.rst` … `H9005.rst`
- modify `docs/reference/index.rst` (add Playground to toctree)
- modify `docs/extensions/ironplc_problemcode.py` (section + prefix)
- modify `playground/src/app.ts` (`renderDiagnostics`)

## Tasks

- [ ] Commit this plan
- [ ] Add CSV + build.rs + Cargo build-dep
- [ ] Wire generated constants into `lib.rs` at every error site
- [ ] Add lib.rs tests asserting each host error carries its code
- [ ] Create docs section, index, and per-code pages
- [ ] Update `ironplc_problemcode.py` and `reference/index.rst`
- [ ] Update `renderDiagnostics` (add H link, drop empty-code case)
- [ ] `cd compiler && just` + docs build green; commit and push

## Validation

- `cd compiler && just` (build, coverage ≥85%, clippy, fmt)
- `cd docs && just ci` (Sphinx `-W -n`, thin-page check)
