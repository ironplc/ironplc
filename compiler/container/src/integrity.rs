//! The header hashes that make a container tamper-evident (ADR-0007).
//!
//! Two of the three hashes the header declares are computed here:
//!
//! - `content_hash` — BLAKE3 over everything that determines how the
//!   program executes: the header (masked, see [`masked_header`]), the task
//!   table, the type section, the constant pool and the code section, in
//!   file order. Stripping the debug section leaves it valid.
//! - `debug_hash` — BLAKE3 over the debug section alone, so the debug
//!   section can be verified, replaced or discarded without touching the
//!   content hash.
//!
//! `layout_hash` is still not computed; it is written as zeros.
//!
//! An all-zero hash is [`NO_HASH`]: the writer did not compute one. A reader
//! accepts it without checking, which keeps hand-built fixtures and
//! containers that predate hashing loadable. The compiler never writes it.
//!
//! This module is `no_std` so the same definitions serve
//! [`crate::ContainerRef`] on constrained targets. `blake3` is built without
//! its `std` feature for that reason.

use crate::header::{
    CONTENT_HASH_RANGE, DEBUG_HASH_RANGE, FLAGS_OFFSET, FLAG_HAS_DEBUG_SECTION, HEADER_SIZE,
    SECTION_DIRECTORY_RANGE,
};
use crate::ContainerError;

/// Length in bytes of every header hash.
pub const HASH_LEN: usize = 32;

/// The all-zero hash: "no hash was computed". A reader does not check it.
pub const NO_HASH: [u8; HASH_LEN] = [0; HASH_LEN];

/// The header bytes as the content hash covers them.
///
/// The header carries the hash, so it cannot be hashed verbatim. Rather than
/// leave it out — which would let a changed `format_version`, `profile` or
/// runtime parameter go unnoticed — the hash covers a copy with these
/// fields zeroed:
///
/// - `content_hash` itself, and `debug_hash`, which the writer fills after
///   the content is fixed and a debug strip zeroes;
/// - the section directory (bytes 136–191). Every section's bytes are
///   hashed directly, so the directory only says *where* they are, and
///   excluding it lets a signature section be inserted, or the debug
///   section removed, without shifting the hash;
/// - the `FLAG_HAS_DEBUG_SECTION` bit, for the same reason as the debug
///   directory entry.
///
/// Everything else — magic, version, profile, the other flags,
/// `layout_hash`, the reserved slots and the runtime parameters — is
/// covered.
pub fn masked_header(header: &[u8; HEADER_SIZE]) -> [u8; HEADER_SIZE] {
    let mut image = *header;
    image[FLAGS_OFFSET] &= !FLAG_HAS_DEBUG_SECTION;
    image[CONTENT_HASH_RANGE].fill(0);
    image[DEBUG_HASH_RANGE].fill(0);
    image[SECTION_DIRECTORY_RANGE].fill(0);
    image
}

/// The parts of a container the content hash covers, in file order.
///
/// Pass an empty slice for an absent type section.
pub struct Content<'a> {
    pub header: &'a [u8; HEADER_SIZE],
    pub task_table: &'a [u8],
    pub type_section: &'a [u8],
    pub const_section: &'a [u8],
    pub code_section: &'a [u8],
}

/// Computes the content hash.
pub fn content_hash(content: &Content<'_>) -> [u8; HASH_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&masked_header(content.header));
    hasher.update(content.task_table);
    hasher.update(content.type_section);
    hasher.update(content.const_section);
    hasher.update(content.code_section);
    *hasher.finalize().as_bytes()
}

/// Computes the debug hash over the debug section bytes.
pub fn debug_hash(debug_section: &[u8]) -> [u8; HASH_LEN] {
    *blake3::hash(debug_section).as_bytes()
}

/// Checks a header's `content_hash` against the content it covers.
///
/// Succeeds when the header carries [`NO_HASH`] or when the recomputed hash
/// matches; fails with [`ContainerError::ContentHashMismatch`] otherwise.
pub fn check_content_hash(
    expected: &[u8; HASH_LEN],
    content: &Content<'_>,
) -> Result<(), ContainerError> {
    if *expected == NO_HASH {
        return Ok(());
    }
    // BLAKE3's Hash compares in constant time; an integrity check does not
    // need that, but there is no reason to give it up either.
    if blake3::Hash::from_bytes(content_hash(content)) == blake3::Hash::from_bytes(*expected) {
        Ok(())
    } else {
        Err(ContainerError::ContentHashMismatch)
    }
}

/// Reports whether a header's `debug_hash` matches the debug section bytes.
///
/// [`NO_HASH`] matches anything: it means no hash was computed, not that
/// the section is empty.
pub fn debug_hash_matches(expected: &[u8; HASH_LEN], debug_section: &[u8]) -> bool {
    *expected == NO_HASH
        || blake3::Hash::from_bytes(debug_hash(debug_section))
            == blake3::Hash::from_bytes(*expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::FileHeader;
    use std::vec::Vec;

    fn header_bytes(header: &FileHeader) -> [u8; HEADER_SIZE] {
        let mut buf = Vec::new();
        header.write_to(&mut buf).unwrap();
        buf.try_into().unwrap()
    }

    fn content<'a>(header: &'a [u8; HEADER_SIZE], sections: [&'a [u8]; 4]) -> Content<'a> {
        Content {
            header,
            task_table: sections[0],
            type_section: sections[1],
            const_section: sections[2],
            code_section: sections[3],
        }
    }

    #[test]
    fn content_hash_when_sections_concatenated_then_equals_blake3_of_masked_header_and_sections() {
        let header = header_bytes(&FileHeader::default());
        let mut concatenated = masked_header(&header).to_vec();
        concatenated.extend_from_slice(b"tasktypeconstcode");
        let expected = *blake3::hash(&concatenated).as_bytes();
        assert_eq!(
            content_hash(&content(&header, [b"task", b"type", b"const", b"code"])),
            expected
        );
    }

    #[test]
    fn content_hash_when_section_boundary_moves_then_hash_unchanged() {
        // The hash is over the concatenation, so only the bytes matter: the
        // reader does not need to agree with the writer about section sizes
        // beyond what the directory says.
        let header = header_bytes(&FileHeader::default());
        assert_eq!(
            content_hash(&content(&header, [b"ab", b"cd", b"ef", b"gh"])),
            content_hash(&content(&header, [b"a", b"bcd", b"ef", b"gh"]))
        );
    }

    #[test]
    fn masked_header_when_excluded_fields_differ_then_images_equal() {
        let plain = header_bytes(&FileHeader::default());
        let decorated = header_bytes(&FileHeader {
            flags: FLAG_HAS_DEBUG_SECTION,
            content_hash: [1; HASH_LEN],
            debug_hash: [2; HASH_LEN],
            sig_section_offset: 3,
            sig_section_size: 4,
            debug_sig_offset: 5,
            debug_sig_size: 6,
            type_section_offset: 7,
            task_section_offset: 8,
            const_section_offset: 9,
            code_section_offset: 10,
            debug_section_offset: 11,
            debug_section_size: 12,
            ..FileHeader::default()
        });
        assert_eq!(masked_header(&plain), masked_header(&decorated));
    }

    #[test]
    fn masked_header_when_covered_fields_differ_then_images_differ() {
        let plain = header_bytes(&FileHeader::default());
        let profile = header_bytes(&FileHeader {
            profile: 1,
            ..FileHeader::default()
        });
        let uptime = header_bytes(&FileHeader {
            flags: crate::header::FLAG_HAS_SYSTEM_UPTIME,
            ..FileHeader::default()
        });
        let layout = header_bytes(&FileHeader {
            layout_hash: [9; HASH_LEN],
            ..FileHeader::default()
        });
        let params = header_bytes(&FileHeader {
            num_functions: 2,
            ..FileHeader::default()
        });
        assert_ne!(masked_header(&plain), masked_header(&profile));
        assert_ne!(masked_header(&plain), masked_header(&uptime));
        assert_ne!(masked_header(&plain), masked_header(&layout));
        assert_ne!(masked_header(&plain), masked_header(&params));
    }

    #[test]
    fn debug_hash_when_known_input_then_matches_blake3() {
        assert_eq!(debug_hash(b"debug"), *blake3::hash(b"debug").as_bytes());
    }

    #[test]
    fn check_content_hash_when_matches_then_ok() {
        let header = header_bytes(&FileHeader::default());
        let c = content(&header, [b"a", b"t", b"c", b"k"]);
        let hash = content_hash(&c);
        assert!(check_content_hash(&hash, &c).is_ok());
    }

    #[test]
    fn check_content_hash_when_no_hash_then_ok() {
        let header = header_bytes(&FileHeader::default());
        let c = content(&header, [b"a", b"t", b"c", b"k"]);
        assert!(check_content_hash(&NO_HASH, &c).is_ok());
    }

    #[test]
    fn check_content_hash_when_differs_then_mismatch() {
        let header = header_bytes(&FileHeader::default());
        let hash = content_hash(&content(&header, [b"a", b"t", b"c", b"k"]));
        assert!(matches!(
            check_content_hash(&hash, &content(&header, [b"a", b"t", b"c", b"K"])),
            Err(ContainerError::ContentHashMismatch)
        ));
    }

    #[test]
    fn debug_hash_matches_when_no_hash_then_true() {
        assert!(debug_hash_matches(&NO_HASH, b"anything"));
    }

    #[test]
    fn debug_hash_matches_when_differs_then_false() {
        let hash = debug_hash(b"debug");
        assert!(!debug_hash_matches(&hash, b"Debug"));
    }

    #[test]
    fn debug_hash_matches_when_same_then_true() {
        let hash = debug_hash(b"debug");
        assert!(debug_hash_matches(&hash, b"debug"));
    }
}
