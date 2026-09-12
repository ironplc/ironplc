//! Spec conformance tests for what the compiler writes into the container
//! header (`REQ-CF-codegen-*`).
//!
//! The container crate owns the rest of `bytecode-container-format.md`; the
//! requirements here are claims about compiler output rather than about the
//! format, so they are verified where the output is produced.
//!
//! See `specs/design/spec-conformance-testing.md` for the mechanism.

use std::io::Cursor;

use ironplc_container::{FileHeader, HEADER_SIZE};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

/// Compiles `source` and parses the header back out of the serialized bytes,
/// so the assertion is about what reaches the file.
fn compiled_header(source: &str) -> FileHeader {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
    let container = crate::compile(
        &analyzed,
        &ctx,
        &crate::CodegenOptions::from(&options),
        &crate::EmptyLookup,
    )
    .unwrap();
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    FileHeader::read_from(&mut Cursor::new(&buf[..HEADER_SIZE])).unwrap()
}

/// REQ-CF-codegen-025: content_hash, debug_hash and layout_hash are written
/// as zeros — nothing computes them yet.
#[spec_test(REQ_CF_codegen_025)]
fn container_spec_req_cf_025_header_hashes_are_written_as_zeros() {
    let header = compiled_header(
        "PROGRAM main
         VAR
             x : DINT;
         END_VAR
             x := 1;
         END_PROGRAM",
    );
    assert_eq!(header.content_hash, [0u8; 32]);
    assert_eq!(header.debug_hash, [0u8; 32]);
    assert_eq!(header.layout_hash, [0u8; 32]);
}
