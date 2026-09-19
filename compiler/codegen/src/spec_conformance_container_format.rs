//! Spec conformance tests for what the compiler writes into the container
//! header (`REQ-CF-codegen-*`).
//!
//! The container crate owns the rest of `bytecode-container-format.md`; the
//! requirements here are claims about compiler output rather than about the
//! format, so they are verified where the output is produced.
//!
//! See `specs/design/spec-conformance-testing.md` for the mechanism.

use std::io::Cursor;

use ironplc_container::{integrity, FileHeader, HEADER_SIZE};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

/// Compiles `source` and returns the serialized bytes with the header parsed
/// back out of them, so every assertion is about what reaches the file.
fn compiled(source: &str) -> (Vec<u8>, FileHeader) {
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
    let header = FileHeader::read_from(&mut Cursor::new(&buf[..HEADER_SIZE])).unwrap();
    (buf, header)
}

const ASSIGNMENT_PROGRAM: &str = "PROGRAM main
         VAR
             x : DINT;
         END_VAR
             x := 1;
         END_PROGRAM";

/// The bytes of one section, as the header's directory locates it.
fn section(buf: &[u8], offset: u32, size: u32) -> &[u8] {
    &buf[offset as usize..(offset + size) as usize]
}

/// REQ-CF-codegen-025: layout_hash is written as zeros — nothing computes
/// it yet.
#[spec_test(REQ_CF_codegen_025)]
fn container_spec_req_cf_025_layout_hash_is_written_as_zeros() {
    let (_, header) = compiled(ASSIGNMENT_PROGRAM);
    assert_eq!(header.layout_hash, integrity::NO_HASH);
}

/// REQ-CF-codegen-026: the compiler writes content_hash and debug_hash,
/// each reproducible from the section bytes it wrote.
#[spec_test(REQ_CF_codegen_026)]
fn container_spec_req_cf_026_content_and_debug_hashes_match_written_sections() {
    let (buf, header) = compiled(ASSIGNMENT_PROGRAM);

    let expected_content = integrity::content_hash(
        section(&buf, header.type_section_offset, header.type_section_size),
        section(&buf, header.const_section_offset, header.const_section_size),
        section(&buf, header.code_section_offset, header.code_section_size),
    );
    assert_ne!(header.content_hash, integrity::NO_HASH);
    assert_eq!(header.content_hash, expected_content);

    let debug = section(&buf, header.debug_section_offset, header.debug_section_size);
    assert_ne!(debug.len(), 0);
    assert_ne!(header.debug_hash, integrity::NO_HASH);
    assert_eq!(header.debug_hash, integrity::debug_hash(debug));
}
