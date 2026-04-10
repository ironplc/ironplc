//! Spec conformance tests for the bytecode container format and instruction set.
//!
//! Each test is annotated with `#[spec_test(REQ_XX_NNN)]` which:
//! 1. Adds `#[test]`
//! 2. References a build-script-generated constant — compilation fails if the
//!    requirement was removed from the spec markdown.
//!
//! The `all_spec_requirements_have_tests` meta-test ensures every requirement
//! in the spec has at least one test here.
//!
//! See `specs/design/spec-conformance-testing.md` for full design.

use std::vec::Vec;

use spec_test_macro::spec_test;

use crate::header::{FileHeader, FLAG_HAS_SYSTEM_UPTIME, FORMAT_VERSION, HEADER_SIZE, MAGIC};

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    let source = include_str!("spec_conformance.rs");
    let mut missing = Vec::new();
    for req in crate::spec_requirements::ALL {
        let ident = req.replace('-', "_");
        if !source.contains(&ident) {
            missing.push(*req);
        }
    }
    assert!(
        missing.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        missing
    );
}

// ---------------------------------------------------------------------------
// Container Format — File Header (REQ-CF-001 through REQ-CF-007)
// ---------------------------------------------------------------------------

/// REQ-CF-001: The file header is exactly 256 bytes.
#[spec_test(REQ_CF_001)]
fn container_spec_req_cf_001_header_size_is_256_bytes() {
    assert_eq!(core::mem::size_of::<FileHeader>(), HEADER_SIZE);
    assert_eq!(HEADER_SIZE, 256);
}

/// REQ-CF-002: Magic number is 0x49504C43 ("IPLC" in ASCII).
#[spec_test(REQ_CF_002)]
fn container_spec_req_cf_002_magic_is_iplc() {
    assert_eq!(MAGIC, 0x49504C43);
    // On disk (little-endian): bytes are [0x43, 0x4C, 0x50, 0x49].
    // The u32 value 0x49504C43 encodes "IPLC" MSB-first.
    let bytes = MAGIC.to_le_bytes();
    assert_eq!(bytes, [0x43, 0x4C, 0x50, 0x49]);
}

/// REQ-CF-003: Format version is 1.
#[spec_test(REQ_CF_003)]
fn container_spec_req_cf_003_format_version_is_1() {
    assert_eq!(FORMAT_VERSION, 1);
}

/// REQ-CF-004: All multi-byte values in the header are little-endian.
#[spec_test(REQ_CF_004)]
fn container_spec_req_cf_004_header_uses_little_endian() {
    let header = FileHeader::default();
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // Magic at offset 0: 0x49504C43 in LE is [0x43, 0x4C, 0x50, 0x49]
    assert_eq!(&buf[0..4], &0x49504C43u32.to_le_bytes());

    // Format version at offset 4: 1u16 in LE is [0x01, 0x00]
    assert_eq!(&buf[4..6], &1u16.to_le_bytes());
}

/// REQ-CF-005: Header field offsets match the spec table layout, totaling
/// 256 bytes with reserved at 218-255.
#[spec_test(REQ_CF_005)]
fn container_spec_req_cf_005_header_field_offsets() {
    // Write a header with distinctive values and verify byte offsets
    let header = FileHeader {
        num_variables: 0x1234,
        code_section_offset: 0xAABBCCDD,
        ..Default::default()
    };

    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();
    assert_eq!(buf.len(), 256);

    // num_variables at offset 196 (u16 LE)
    assert_eq!(u16::from_le_bytes([buf[196], buf[197]]), 0x1234);

    // code_section_offset at offset 176 (u32 LE)
    assert_eq!(
        u32::from_le_bytes([buf[176], buf[177], buf[178], buf[179]]),
        0xAABBCCDD
    );
}

/// REQ-CF-006: Reserved bytes are 38 bytes at offsets 218-255.
#[spec_test(REQ_CF_006)]
fn container_spec_req_cf_006_reserved_is_38_bytes_at_offset_218() {
    let header = FileHeader::default();
    assert_eq!(header.reserved.len(), 38);

    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // Bytes 218..256 should all be zero (reserved)
    assert_eq!(&buf[218..256], &[0u8; 38]);
    // And that's exactly the end of the header
    assert_eq!(buf.len(), 256);
}

/// REQ-CF-007: Flags bit 0 is FLAG_HAS_SYSTEM_UPTIME (0x01).
#[spec_test(REQ_CF_007)]
fn container_spec_req_cf_007_flags_bit0_is_system_uptime() {
    assert_eq!(FLAG_HAS_SYSTEM_UPTIME, 0x01);

    // Verify the flag is at byte offset 7
    let header = FileHeader {
        flags: FLAG_HAS_SYSTEM_UPTIME,
        ..Default::default()
    };
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();
    assert_eq!(buf[7], 0x01);
}
