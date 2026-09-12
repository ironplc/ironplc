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

use std::io::Cursor;
use std::vec;
use std::vec::Vec;

use spec_test_macro::spec_test;

use crate::builder::ContainerBuilder;
use crate::code_section::{CodeSection, FuncEntry};
use crate::constant_pool::{ConstEntry, ConstantPool};
use crate::debug_section::FuncNameEntry;
use crate::header::{
    FileHeader, FLAG_HAS_DEBUG_SECTION, FLAG_HAS_SYSTEM_UPTIME, FLAG_HAS_TYPE_SECTION,
    FORMAT_VERSION, HEADER_SIZE, MAGIC,
};
use crate::id_types::{FbTypeId, FunctionId};
use crate::type_section::{
    ArrayDescriptor, FbTypeDescriptor, FieldEntry, FieldType, TypeSection, UserFbDescriptor,
};
use crate::{opcode, ConstType, ContainerError};

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    // UNTESTED is computed by build.rs by scanning all .rs files under src/
    // for #[spec_test(REQ_...)] attributes.
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

// ---------------------------------------------------------------------------
// Container Format — File Header (REQ-CF-container-001 through REQ-CF-container-007)
// ---------------------------------------------------------------------------

/// REQ-CF-container-001: The file header is exactly 256 bytes.
#[spec_test(REQ_CF_container_001)]
fn container_spec_req_cf_001_header_size_is_256_bytes() {
    assert_eq!(core::mem::size_of::<FileHeader>(), HEADER_SIZE);
    assert_eq!(HEADER_SIZE, 256);
}

/// REQ-CF-container-002: Magic number is 0x49504C43 ("IPLC" in ASCII).
#[spec_test(REQ_CF_container_002)]
fn container_spec_req_cf_002_magic_is_iplc() {
    assert_eq!(MAGIC, 0x49504C43);
    // On disk (little-endian): bytes are [0x43, 0x4C, 0x50, 0x49].
    // The u32 value 0x49504C43 encodes "IPLC" MSB-first.
    let bytes = MAGIC.to_le_bytes();
    assert_eq!(bytes, [0x43, 0x4C, 0x50, 0x49]);
}

/// REQ-CF-container-003: Format version is 3.
#[spec_test(REQ_CF_container_003)]
fn container_spec_req_cf_003_format_version_is_3() {
    assert_eq!(FORMAT_VERSION, 3);
}

/// REQ-CF-container-004: All multi-byte values in the header are little-endian.
#[spec_test(REQ_CF_container_004)]
fn container_spec_req_cf_004_header_uses_little_endian() {
    let header = FileHeader::default();
    let mut buf = Vec::new();
    header.write_to(&mut buf).unwrap();

    // Magic at offset 0: 0x49504C43 in LE is [0x43, 0x4C, 0x50, 0x49]
    assert_eq!(&buf[0..4], &0x49504C43u32.to_le_bytes());

    // Format version at offset 4: 3u16 in LE is [0x03, 0x00]
    assert_eq!(&buf[4..6], &3u16.to_le_bytes());
}

/// REQ-CF-container-005: Header field offsets match the spec table layout, totaling
/// 256 bytes with reserved at 218-255.
#[spec_test(REQ_CF_container_005)]
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

    // Bytes 40..72 are the reserved hash slot (formerly source_hash);
    // a default header must zero them.
    assert_eq!(&buf[40..72], &[0u8; 32]);
}

/// REQ-CF-container-006: Reserved bytes are 38 bytes at offsets 218-255.
#[spec_test(REQ_CF_container_006)]
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

/// REQ-CF-container-007: Flags bit 0 is FLAG_HAS_SYSTEM_UPTIME (0x01).
#[spec_test(REQ_CF_container_007)]
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

// ---------------------------------------------------------------------------
// Container Format — File Layout (REQ-CF-container-010 through REQ-CF-container-016)
// ---------------------------------------------------------------------------

/// Serializes `container` and parses the header back out of the bytes, so
/// every assertion below is about what is on disk, not what the builder
/// held in memory.
fn write_and_read_header(container: &crate::Container) -> (Vec<u8>, FileHeader) {
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    let header = FileHeader::read_from(&mut Cursor::new(&buf[..HEADER_SIZE])).unwrap();
    (buf, header)
}

/// A container carrying every section, both optional ones included.
fn full_container_bytes() -> (Vec<u8>, FileHeader) {
    let mut builder = ContainerBuilder::new();
    builder.add_array_descriptor(0, 4, 0);
    let container = builder
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(FunctionId::INIT, &[opcode::RET_VOID], 1, 1, 0)
        .add_func_name(FuncNameEntry {
            function_id: FunctionId::INIT,
            name: "MAIN".into(),
        })
        .build();
    write_and_read_header(&container)
}

/// A container carrying only the mandatory sections.
fn minimal_container_bytes() -> (Vec<u8>, FileHeader) {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(FunctionId::INIT, &[opcode::RET_VOID], 1, 1, 0)
        .build();
    write_and_read_header(&container)
}

/// REQ-CF-container-010: Sections appear in the order header, task table,
/// type section, constant pool, code section, debug section. The signature
/// sections that precede the task table are planned and not yet emitted.
#[spec_test(REQ_CF_container_010)]
fn container_spec_req_cf_010_sections_appear_in_fixed_order() {
    let (_, h) = full_container_bytes();
    assert_eq!(h.task_section_offset, HEADER_SIZE as u32);
    assert!(h.task_section_offset < h.type_section_offset);
    assert!(h.type_section_offset < h.const_section_offset);
    assert!(h.const_section_offset < h.code_section_offset);
    assert!(h.code_section_offset < h.debug_section_offset);
}

/// REQ-CF-container-011: Every section starts where the previous present one
/// ends, with no padding; with no signature sections emitted the task table
/// therefore starts at 256.
#[spec_test(REQ_CF_container_011)]
fn container_spec_req_cf_011_sections_are_contiguous() {
    let (buf, h) = full_container_bytes();
    assert_eq!(h.task_section_offset, 256);
    assert_eq!(
        h.type_section_offset,
        h.task_section_offset + h.task_section_size
    );
    assert_eq!(
        h.const_section_offset,
        h.type_section_offset + h.type_section_size
    );
    assert_eq!(
        h.code_section_offset,
        h.const_section_offset + h.const_section_size
    );
    assert_eq!(
        h.debug_section_offset,
        h.code_section_offset + h.code_section_size
    );
    assert_eq!(
        buf.len() as u32,
        h.debug_section_offset + h.debug_section_size
    );

    // With the optional sections absent the constant pool follows the task
    // table directly and the file ends with the code section.
    let (buf, h) = minimal_container_bytes();
    assert_eq!(
        h.const_section_offset,
        h.task_section_offset + h.task_section_size
    );
    assert_eq!(
        buf.len() as u32,
        h.code_section_offset + h.code_section_size
    );
}

/// REQ-CF-container-012: An absent optional section has offset 0 and size 0.
#[spec_test(REQ_CF_container_012)]
fn container_spec_req_cf_012_absent_sections_have_zero_offset_and_size() {
    let (_, h) = minimal_container_bytes();
    assert_eq!(h.type_section_offset, 0);
    assert_eq!(h.type_section_size, 0);
    assert_eq!(h.debug_section_offset, 0);
    assert_eq!(h.debug_section_size, 0);
}

/// REQ-CF-container-013: Flag bit 1 (0x02) is set iff a debug section is present.
#[spec_test(REQ_CF_container_013)]
fn container_spec_req_cf_013_flag_bit1_iff_debug_section() {
    assert_eq!(FLAG_HAS_DEBUG_SECTION, 0x02);
    let (buf, h) = full_container_bytes();
    assert_ne!(h.flags & FLAG_HAS_DEBUG_SECTION, 0);
    assert_ne!(buf[7] & 0x02, 0);
    let (_, h) = minimal_container_bytes();
    assert_eq!(h.flags & FLAG_HAS_DEBUG_SECTION, 0);
}

/// REQ-CF-container-014: Flag bit 2 (0x04) is set iff a type section is present.
#[spec_test(REQ_CF_container_014)]
fn container_spec_req_cf_014_flag_bit2_iff_type_section() {
    assert_eq!(FLAG_HAS_TYPE_SECTION, 0x04);
    let (buf, h) = full_container_bytes();
    assert_ne!(h.flags & FLAG_HAS_TYPE_SECTION, 0);
    assert_ne!(buf[7] & 0x04, 0);
    let (_, h) = minimal_container_bytes();
    assert_eq!(h.flags & FLAG_HAS_TYPE_SECTION, 0);
}

/// REQ-CF-container-015: Bits 3–7 are written as zero and no bit marks a
/// signature section: a container with every section sets exactly the debug
/// and type bits.
#[spec_test(REQ_CF_container_015)]
fn container_spec_req_cf_015_no_other_flag_bits_are_written() {
    let (_, h) = full_container_bytes();
    assert_eq!(h.flags, FLAG_HAS_DEBUG_SECTION | FLAG_HAS_TYPE_SECTION);
    assert_eq!(h.flags & 0xF8, 0);
    let (_, h) = minimal_container_bytes();
    assert_eq!(h.flags, 0);
}

/// REQ-CF-container-016: Without signature sections all four signature
/// directory entries are zero.
#[spec_test(REQ_CF_container_016)]
fn container_spec_req_cf_016_signature_directory_entries_are_zero() {
    let (_, h) = full_container_bytes();
    assert_eq!(h.sig_section_offset, 0);
    assert_eq!(h.sig_section_size, 0);
    assert_eq!(h.debug_sig_offset, 0);
    assert_eq!(h.debug_sig_size, 0);
}

// ---------------------------------------------------------------------------
// Container Format — Type Section (REQ-CF-container-008 through REQ-CF-container-009,
// REQ-CF-container-018 through REQ-CF-container-021)
// ---------------------------------------------------------------------------

fn write_type_section(section: &TypeSection) -> Vec<u8> {
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();
    buf
}

/// REQ-CF-container-018: The type section is FB type descriptors, then array
/// descriptors, then user FB descriptors, each behind a u16 count.
#[spec_test(REQ_CF_container_018)]
fn container_spec_req_cf_018_type_section_sub_table_order() {
    let section = TypeSection {
        fb_types: vec![FbTypeDescriptor {
            type_id: FbTypeId::new(0x0A),
            fields: vec![FieldEntry {
                field_type: FieldType::I32,
                field_extra: 0,
            }],
        }],
        array_descriptors: vec![ArrayDescriptor {
            element_type: FieldType::F64 as u8,
            total_elements: 3,
            element_extra: 0,
        }],
        user_fb_types: vec![UserFbDescriptor {
            type_id: FbTypeId::new(0x0B),
            function_id: FunctionId::new(2),
            var_offset: 5,
            num_fields: 1,
        }],
    };
    let buf = write_type_section(&section);
    // fb count(2) + fb header(4) + one field(4) = 10, then array count(2) +
    // descriptor(8) = 20, then user count(2) + descriptor(8) = 30.
    assert_eq!(buf.len(), 30);
    assert_eq!(&buf[0..2], &1u16.to_le_bytes());
    assert_eq!(&buf[2..4], &0x0Au16.to_le_bytes());
    assert_eq!(&buf[10..12], &1u16.to_le_bytes());
    assert_eq!(buf[12], FieldType::F64 as u8);
    assert_eq!(&buf[20..22], &1u16.to_le_bytes());
    assert_eq!(&buf[22..24], &0x0Bu16.to_le_bytes());
}

/// REQ-CF-container-019: An ArrayDescriptor is element_type u8, reserved u8,
/// total_elements u32, element_extra u16 — 8 bytes.
#[spec_test(REQ_CF_container_019)]
fn container_spec_req_cf_019_array_descriptor_is_8_bytes() {
    let section = TypeSection {
        array_descriptors: vec![ArrayDescriptor {
            element_type: FieldType::String as u8,
            total_elements: 0x0102_0304,
            element_extra: 0x0506,
        }],
        ..Default::default()
    };
    let buf = write_type_section(&section);
    // fb count(2) + array count(2) + descriptor(8) + user count(2)
    assert_eq!(buf.len(), 14);
    assert_eq!(
        &buf[4..12],
        &[
            FieldType::String as u8,
            0,
            0x04,
            0x03,
            0x02,
            0x01,
            0x06,
            0x05
        ]
    );
}

/// REQ-CF-container-020: A UserFbDescriptor is type_id u16, function_id u16,
/// var_offset u16, num_fields u8, reserved u8 — 8 bytes.
#[spec_test(REQ_CF_container_020)]
fn container_spec_req_cf_020_user_fb_descriptor_is_8_bytes() {
    let section = TypeSection {
        user_fb_types: vec![UserFbDescriptor {
            type_id: FbTypeId::new(0x0102),
            function_id: FunctionId::new(0x0304),
            var_offset: 0x0506,
            num_fields: 7,
        }],
        ..Default::default()
    };
    let buf = write_type_section(&section);
    // fb count(2) + array count(2) + user count(2) + descriptor(8)
    assert_eq!(buf.len(), 14);
    assert_eq!(&buf[6..14], &[0x02, 0x01, 0x04, 0x03, 0x06, 0x05, 7, 0]);
}

/// REQ-CF-container-021: An FB type descriptor is a 4-byte header (type_id
/// u16, num_fields u8, reserved u8) followed by its FieldEntry records.
#[spec_test(REQ_CF_container_021)]
fn container_spec_req_cf_021_fb_type_descriptor_header_is_4_bytes() {
    let section = TypeSection {
        fb_types: vec![FbTypeDescriptor {
            type_id: FbTypeId::new(0x0102),
            fields: vec![
                FieldEntry {
                    field_type: FieldType::I32,
                    field_extra: 0,
                },
                FieldEntry {
                    field_type: FieldType::String,
                    field_extra: 0x0708,
                },
            ],
        }],
        ..Default::default()
    };
    let buf = write_type_section(&section);
    // fb count(2) + header(4) + 2 fields(8) + array count(2) + user count(2)
    assert_eq!(buf.len(), 18);
    assert_eq!(&buf[2..6], &[0x02, 0x01, 2, 0]);
    assert_eq!(&buf[6..10], &[FieldType::I32 as u8, 0, 0, 0]);
    assert_eq!(&buf[10..14], &[FieldType::String as u8, 0, 0x08, 0x07]);
}

// ---------------------------------------------------------------------------
// Container Format — Constant Pool and Code Section (REQ-CF-container-022
// through REQ-CF-container-024)
// ---------------------------------------------------------------------------

fn write_code_section(section: &CodeSection) -> Vec<u8> {
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();
    buf
}

/// REQ-CF-container-022: A FuncEntry is 16 bytes with num_params at offset 14.
#[spec_test(REQ_CF_container_022)]
fn container_spec_req_cf_022_func_entry_is_16_bytes() {
    let section = CodeSection {
        functions: vec![FuncEntry {
            function_id: FunctionId::new(0x0102),
            code_offset: 0x0304_0506,
            code_length: 1,
            max_stack_depth: 0x0708,
            num_locals: 0x090A,
            num_params: 0x0B0C,
        }],
        bytecode: vec![opcode::RET_VOID],
    };
    assert_eq!(section.section_size(), 17);
    let buf = write_code_section(&section);
    assert_eq!(buf.len(), 17);
    assert_eq!(&buf[0..2], &[0x02, 0x01]);
    assert_eq!(&buf[2..6], &[0x06, 0x05, 0x04, 0x03]);
    assert_eq!(&buf[6..10], &[1, 0, 0, 0]);
    assert_eq!(&buf[10..12], &[0x08, 0x07]);
    assert_eq!(&buf[12..14], &[0x0A, 0x09]);
    assert_eq!(&buf[14..16], &[0x0C, 0x0B]);
}

/// REQ-CF-container-023: Bodies follow the directory immediately; a body is
/// at 16 × num_functions + code_offset within the section.
#[spec_test(REQ_CF_container_023)]
fn container_spec_req_cf_023_bodies_follow_directory() {
    let section = CodeSection {
        functions: vec![
            FuncEntry {
                function_id: FunctionId::new(0),
                code_offset: 0,
                code_length: 1,
                max_stack_depth: 0,
                num_locals: 0,
                num_params: 0,
            },
            FuncEntry {
                function_id: FunctionId::new(1),
                code_offset: 1,
                code_length: 2,
                max_stack_depth: 0,
                num_locals: 0,
                num_params: 0,
            },
        ],
        bytecode: vec![0xAA, 0xBB, 0xCC],
    };
    let buf = write_code_section(&section);
    assert_eq!(buf.len(), 2 * 16 + 3);
    assert_eq!(&buf[32..35], &[0xAA, 0xBB, 0xCC]);
    assert_eq!(&buf[32 + 1..32 + 3], &[0xBB, 0xCC]);
}

/// REQ-CF-container-024: A ConstEntry is const_type u8, char_width u8,
/// size u16, then exactly `size` value bytes; strings carry no length prefix.
#[spec_test(REQ_CF_container_024)]
fn container_spec_req_cf_024_const_entry_header_and_value() {
    let mut pool = ConstantPool::default();
    pool.push(ConstEntry::primitive_le(
        ConstType::I32,
        &0x0102_0304i32.to_le_bytes(),
    ));
    pool.push(ConstEntry::string(b"hi".to_vec()));
    let mut buf = Vec::new();
    pool.write_to(&mut buf).unwrap();
    assert_eq!(
        buf,
        vec![
            2,
            0, // count
            ConstType::I32 as u8,
            0,
            4,
            0,
            0x04,
            0x03,
            0x02,
            0x01, // i32
            ConstType::Str as u8,
            1,
            2,
            0,
            b'h',
            b'i', // STRING "hi"
        ]
    );
}

// ---------------------------------------------------------------------------
// Container Format — Loading Sequence (REQ-CF-container-026 through
// REQ-CF-container-027)
// ---------------------------------------------------------------------------

/// REQ-CF-container-026: A wrong magic is rejected with InvalidMagic.
#[spec_test(REQ_CF_container_026)]
fn container_spec_req_cf_026_wrong_magic_is_rejected() {
    let mut bytes = [0u8; HEADER_SIZE];
    bytes[0..4].copy_from_slice(&(MAGIC + 1).to_le_bytes());
    bytes[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    assert!(matches!(
        FileHeader::from_bytes(&bytes),
        Err(ContainerError::InvalidMagic)
    ));
}

/// REQ-CF-container-027: A format_version other than FORMAT_VERSION is
/// rejected with UnsupportedVersion.
#[spec_test(REQ_CF_container_027)]
fn container_spec_req_cf_027_unsupported_version_is_rejected() {
    let mut bytes = [0u8; HEADER_SIZE];
    bytes[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    bytes[4..6].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    assert!(matches!(
        FileHeader::from_bytes(&bytes),
        Err(ContainerError::UnsupportedVersion)
    ));
}

// ---------------------------------------------------------------------------
// Container Format — Type Section (REQ-CF-container-008 through REQ-CF-container-009)
// ---------------------------------------------------------------------------

/// REQ-CF-container-008: Each FieldEntry is 4 bytes.
#[spec_test(REQ_CF_container_008)]
fn container_spec_req_cf_008_field_entry_is_4_bytes() {
    let section = TypeSection {
        fb_types: vec![FbTypeDescriptor {
            type_id: FbTypeId::new(0),
            fields: vec![FieldEntry {
                field_type: FieldType::I32,
                field_extra: 0,
            }],
        }],
        ..Default::default()
    };
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();
    // fb_count(2) + type_id(2) + num_fields(1) + reserved(1) + field(4)
    //   + array_count(2) + user_fb_count(2) = 14
    // The single field entry occupies exactly 4 bytes (bytes 6..10).
    assert_eq!(buf.len(), 14);
}

/// REQ-CF-container-009: FieldType/var_type encoding values 0 through 10.
#[spec_test(REQ_CF_container_009)]
fn container_spec_req_cf_009_field_type_encoding_values() {
    assert_eq!(FieldType::I32 as u8, 0);
    assert_eq!(FieldType::U32 as u8, 1);
    assert_eq!(FieldType::I64 as u8, 2);
    assert_eq!(FieldType::U64 as u8, 3);
    assert_eq!(FieldType::F32 as u8, 4);
    assert_eq!(FieldType::F64 as u8, 5);
    assert_eq!(FieldType::String as u8, 6);
    assert_eq!(FieldType::WString as u8, 7);
    assert_eq!(FieldType::FbInstance as u8, 8);
    assert_eq!(FieldType::Time as u8, 9);
    assert_eq!(FieldType::Slot as u8, 10);
    // Values 0-10 are valid; 11 is invalid
    for tag in 0..=10u8 {
        assert!(FieldType::from_u8(tag).is_ok());
    }
    assert!(FieldType::from_u8(11).is_err());
}

// ---------------------------------------------------------------------------
// Enumeration debug section — ENUM_DEF payload (cross-crate: this requirement
// is defined in enumeration-codegen.md but owned here because the on-disk
// ENUM_DEF format is a container concern. codegen owns the rest of that doc.)
// ---------------------------------------------------------------------------

/// REQ-EN-container-061: ENUM_DEF sub-table roundtrips through write/read.
#[spec_test(REQ_EN_container_061)]
fn container_spec_req_en_061_enum_def_payload_roundtrips() {
    use crate::debug_section::{DebugSection, EnumDefEntry};
    use std::io::Cursor;

    let section = DebugSection {
        var_names: vec![],
        func_names: vec![],
        line_map: vec![],
        string_layouts: vec![],
        source_files: vec![],
        enum_defs: vec![EnumDefEntry {
            type_name: "COLOR".into(),
            values: vec!["RED".into(), "GREEN".into(), "BLUE".into()],
        }],
    };
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();

    let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(decoded.enum_defs.len(), 1);
    assert_eq!(decoded.enum_defs[0].type_name, "COLOR");
    assert_eq!(decoded.enum_defs[0].values, vec!["RED", "GREEN", "BLUE"]);
}
