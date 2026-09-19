# Compute and Verify the Container Content and Debug Hashes

Addresses step 1 and step 2 of
[issue #1583](https://github.com/ironplc/ironplc/issues/1583): the header's
`content_hash` and `debug_hash` are declared by the format and printed by the
disassembler, but nothing computes them and nothing checks them. Signatures
(step 3) are out of scope; ADR-0007 stays `proposed`.

## Goal

1. Every container the compiler writes carries a BLAKE3 `content_hash` over
   `type_section || constant_pool || code_section` and, when a debug section
   is present, a BLAKE3 `debug_hash` over it.
2. Every reader (`Container::read_from` for the host VM and tools,
   `ContainerRef::from_slice` for `no_std`) recomputes the content hash and
   rejects a container whose header hash disagrees.
3. An all-zero `content_hash` means "no hash" and is accepted without a check,
   so hand-built fixtures and the frozen `steel_thread.iplc` golden keep
   loading. The compiler never writes the all-zero form.
4. A `debug_hash` mismatch discards the debug section and is not fatal, so
   stripping or corrupting debug info leaves execution unaffected — the
   separability ADR-0007 claims, and now tests.

## Architecture

- A new `no_std` module `container/src/integrity.rs` owns the hash
  definitions: `content_hash(type, const, code)`, `debug_hash(debug)`, the
  all-zero `NO_HASH` sentinel and `check_content_hash`. `blake3` becomes a
  non-optional dependency of `ironplc-container` with
  `default-features = false`, so the `no_std` reader can use it.
- `Container::write_to` computes both hashes from the section bytes it is
  about to write, then writes the header.
- `Container::read_from` checks the content hash before parsing sections and
  drops the debug section on a debug hash mismatch. `ContainerRef::from_slice`
  checks the content hash after bounds-checking the sections.
- `ContainerError` gains `ContentHashMismatch`. `ironplcvm` surfaces it under
  the existing `V6002` container-read code.

## Prefactoring

`Container::write_to` streams each section straight to the writer, deriving
offsets from `section_size()` estimates. The hash needs the bytes before the
header is written, so the prefactor serializes each section to a buffer first
and derives the header from those buffers. The result is the header being
described by what was actually written rather than by a parallel size
computation. Existing tests pass unchanged.

The `with_tampered_header` helper in `container_ref.rs` tests and the inline
copies in `container.rs` tests move to `test_support`, since the new tests
tamper the header in the same way.

## Design doc reference

- `specs/design/bytecode-container-format.md` — header table, Content Hash
  Scope, Loading Sequence, REQ-CF-codegen-025
- `specs/adrs/0007-dual-signature-integrity-model.md` — Implementation Status

## File map

- `compiler/container/Cargo.toml` — blake3 non-optional, no_std
- `compiler/container/src/integrity.rs` — new
- `compiler/container/src/lib.rs` — export
- `compiler/container/src/error.rs` — `ContentHashMismatch`
- `compiler/container/src/container.rs` — compute on write, check on read
- `compiler/container/src/container_ref.rs` — check on parse
- `compiler/container/src/test_support.rs` — `with_tampered_header`
- `compiler/container/src/spec_conformance.rs` — new REQ tests
- `compiler/codegen/src/spec_conformance_container_format.rs` — REQ-CF-codegen-025/026
- `compiler/vm-cli/tests/cli.rs` — end-to-end rejection under V6002
- `specs/design/bytecode-container-format.md`, `specs/adrs/0007-*.md`,
  `docs/reference/runtime/problems/V6002.rst`

## Tasks

- [ ] Prefactor `Container::write_to` to serialize sections to buffers first
- [ ] Move `with_tampered_header` to `test_support`
- [ ] Add `integrity` module and `ContentHashMismatch`
- [ ] Compute hashes in `write_to`; verify in `read_from` and `from_slice`
- [ ] Spec requirements and conformance tests (container, codegen)
- [ ] End-to-end `ironplcvm` rejection test
- [ ] Update design doc, ADR-0007 status, V6002 docs
- [ ] Delete this plan; run `cd compiler && just`
