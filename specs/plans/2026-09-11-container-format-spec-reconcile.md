# Reconcile the Container Format Spec with the Writer

**Issue:** [#1578](https://github.com/ironplc/ironplc/issues/1578)

## Goal

Make `specs/design/bytecode-container-format.md` describe the bytes that
`ironplc_container::Container::write_to` actually emits, and pin every
corrected claim with a `#[spec_test]` so the two cannot drift apart silently
again.

## Audit findings

Code is king throughout: the spec moves to the code, never the reverse.

| # | Spec says | Code does | Where |
|---|-----------|-----------|-------|
| 1 | Sections: header, content sig, debug sig, type, task, const, code, debug | header, **task**, type (opt), const, code, debug (opt); no signature sections are ever written | `container.rs:30-80` |
| 2 | Section directory is "in file-layout order" | Directory lists type before task; the file has task before type | `header.rs` |
| 3 | Content signature "present when flags bit 0 is set"; debug signature "bit 0 and bit 1" | Bit 0 is `FLAG_HAS_SYSTEM_UPTIME`; no flag bit is assigned to either signature section | `header.rs:14` |
| 4 | `content_hash` / `debug_hash` / `layout_hash` are BLAKE3 digests | All three are written as zeros; nothing computes them (ADR-0007 Implementation Status) | `builder.rs`, `codegen` |
| 5 | "The PLC rejects bytecode without a content signature"; loading sequence steps 4–13 | The VM loads any container it can parse; only magic, version and the zero call-depth check exist | `header.rs`, `vm.rs` |
| 6 | Type section has a Variable Table and Function Signatures | Type section is three count-prefixed sub-tables: FB type descriptors, array descriptors, **user FB descriptors** (undocumented) | `type_section.rs` |
| 7 | `FuncEntry` is 14 bytes | 16 bytes: `num_params: u16` at offset 14 | `code_section.rs:9` |
| 8 | String constants carry a u16 length prefix inside `value` | `value` is the raw character bytes; `size` is the only length | `constant_pool.rs` |
| 9 | "Version 1 is defined by this spec" | `FORMAT_VERSION` is 3 | `header.rs:10` |
| 10 | `61131-task-support.md`: task table sits "between the type section and the constant pool" | Immediately after the header | `container.rs:30` |
| 11 | `project::disassemble` reports `hasContentSignature` from bit 0 | Bit 0 is system uptime, so the bytecode viewer labels uptime containers "Content Signature" | `disassemble.rs:65` |

Finding 11 is a code bug that copied the spec's error; it is fixed against the
container crate's constant.

## Architecture

No format change. Every fix is either spec text, a test, or a consumer that
read the wrong bit.

Requirements are added as `REQ-CF-container-NNN` (owned by `container`) except
the header-hash claim, which is a compiler-output claim and is owned by
`codegen` as `REQ-CF-codegen-NNN`; `codegen/build.rs` starts listing the doc.

Unimplemented material (signature sections, hashes, verifier-only type
sub-tables, loading steps) stays in the document, marked with a status note
that points at the ADR whose Implementation Status already records the gap.
Those paragraphs get no requirement IDs: a `#[ignore]`d marker would report
them as covered.

## Prefactoring

Introduce `FLAG_HAS_DEBUG_SECTION` and `FLAG_HAS_TYPE_SECTION` in
`header.rs` beside `FLAG_HAS_SYSTEM_UPTIME`, and use them in `container.rs`
and `project/src/disassemble.rs` in place of the `0x02` / `0x04` literals.
The disassembler bug (finding 11) is exactly the class of mistake a named
constant prevents, and the new spec tests should reference names, not
magic numbers. Behaviour-preserving; existing tests pass unchanged.

## Design doc reference

- `specs/design/bytecode-container-format.md`
- `specs/design/61131-task-support.md`
- `specs/design/cross-crate-spec-conformance.md` (ID grammar)

## File map

| File | Change |
|------|--------|
| `compiler/container/src/header.rs` | Add the two flag constants |
| `compiler/container/src/container.rs` | Use the constants; fix the struct doc comment |
| `compiler/container/src/lib.rs` | Re-export the constants |
| `compiler/container/src/spec_conformance.rs` | New `REQ-CF-container-010` … tests |
| `compiler/codegen/build.rs` | List `bytecode-container-format.md` |
| `compiler/codegen/src/spec_conformance.rs` | `REQ-CF-codegen-025` test |
| `compiler/project/src/disassemble.rs` | Report signature presence from the directory; add `hasSystemUptime` |
| `integrations/vscode/src/iplcRendering.ts` (+ tests) | Render the uptime flag |
| `docs/reference/editor/bytecode-viewer.rst` | Flag list |
| `specs/design/bytecode-container-format.md` | Findings 1–9 |
| `specs/design/61131-task-support.md` | Finding 10 |

## Tasks

- [ ] Prefactor: named flag constants
- [ ] Spec + tests: file layout, flags, signature and hash status (findings 1–5, 9)
- [ ] Spec + tests: type, code, constant-pool sections (findings 6–8)
- [ ] Spec + test: header hashes are zero (codegen-owned)
- [ ] Task-support doc (finding 10)
- [ ] Disassembler and viewer (finding 11)
- [ ] `cd compiler && just`, `cd specs && just`, extension unit tests
- [ ] Delete this plan
