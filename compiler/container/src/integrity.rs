//! The header hashes that make a container tamper-evident (ADR-0007).
//!
//! Two of the three hashes the header declares are computed here:
//!
//! - `content_hash` — BLAKE3 over `type_section || constant_pool ||
//!   code_section`, in file order. It covers everything that determines
//!   execution, and nothing else, so stripping the debug section leaves it
//!   valid.
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

use crate::ContainerError;

/// Length in bytes of every header hash.
pub const HASH_LEN: usize = 32;

/// The all-zero hash: "no hash was computed". A reader does not check it.
pub const NO_HASH: [u8; HASH_LEN] = [0; HASH_LEN];

/// Computes the content hash over the three sections it covers, in file
/// order. Pass an empty slice for an absent type section.
pub fn content_hash(
    type_section: &[u8],
    const_section: &[u8],
    code_section: &[u8],
) -> [u8; HASH_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(type_section);
    hasher.update(const_section);
    hasher.update(code_section);
    *hasher.finalize().as_bytes()
}

/// Computes the debug hash over the debug section bytes.
pub fn debug_hash(debug_section: &[u8]) -> [u8; HASH_LEN] {
    *blake3::hash(debug_section).as_bytes()
}

/// Checks a header's `content_hash` against the sections it covers.
///
/// Succeeds when the header carries [`NO_HASH`] or when the recomputed hash
/// matches; fails with [`ContainerError::ContentHashMismatch`] otherwise.
pub fn check_content_hash(
    expected: &[u8; HASH_LEN],
    type_section: &[u8],
    const_section: &[u8],
    code_section: &[u8],
) -> Result<(), ContainerError> {
    if *expected == NO_HASH {
        return Ok(());
    }
    // BLAKE3's Hash compares in constant time; an integrity check does not
    // need that, but there is no reason to give it up either.
    let actual = blake3::Hash::from_bytes(content_hash(type_section, const_section, code_section));
    if actual == blake3::Hash::from_bytes(*expected) {
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

    #[test]
    fn content_hash_when_sections_concatenated_then_equals_blake3_of_concatenation() {
        let expected = *blake3::hash(b"typeconstcode").as_bytes();
        assert_eq!(content_hash(b"type", b"const", b"code"), expected);
    }

    #[test]
    fn content_hash_when_type_section_absent_then_equals_blake3_of_const_and_code() {
        let expected = *blake3::hash(b"constcode").as_bytes();
        assert_eq!(content_hash(&[], b"const", b"code"), expected);
    }

    #[test]
    fn content_hash_when_section_boundary_moves_then_hash_unchanged() {
        // The hash is over the concatenation, so only the bytes matter: the
        // reader does not need to agree with the writer about section sizes
        // beyond what the directory says.
        assert_eq!(
            content_hash(b"ab", b"cd", b"ef"),
            content_hash(b"a", b"bcd", b"ef")
        );
    }

    #[test]
    fn debug_hash_when_known_input_then_matches_blake3() {
        assert_eq!(debug_hash(b"debug"), *blake3::hash(b"debug").as_bytes());
    }

    #[test]
    fn check_content_hash_when_matches_then_ok() {
        let hash = content_hash(b"t", b"c", b"k");
        assert!(check_content_hash(&hash, b"t", b"c", b"k").is_ok());
    }

    #[test]
    fn check_content_hash_when_no_hash_then_ok() {
        assert!(check_content_hash(&NO_HASH, b"t", b"c", b"k").is_ok());
    }

    #[test]
    fn check_content_hash_when_differs_then_mismatch() {
        let hash = content_hash(b"t", b"c", b"k");
        assert!(matches!(
            check_content_hash(&hash, b"t", b"c", b"K"),
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
