//! Standard library type detection for IEC 61131-3.
//!
//! This module provides functions to detect whether a type name refers to
//! a standard library type, and whether that type is supported or not.

use ironplc_dsl::common::TypeName;
use phf::{phf_set, Set};

/// Standard library types that are NOT YET implemented.
///
/// These are types from IEC 61131-5 or other standard extensions.
///
/// When a user tries to use one of these, they get an "unsupported stdlib type" error.
static UNSUPPORTED_STANDARD_LIBRARY_TYPES: Set<&'static str> = phf_set! {
    // Add IEC 61131-5 types here as needed
};

/// Standard library types that ARE implemented and available.
///
/// These types are registered in the type environment and can be used directly.
/// This list is kept in sync with intermediates/stdlib_function_block.rs.
static SUPPORTED_STANDARD_LIBRARY_TYPES: Set<&'static str> = phf_set! {
    // Bistable function blocks (IEC 61131-3 2.5.2.3.1)
    "sr",
    "rs",
    // Edge detection (IEC 61131-3 2.5.2.3.2)
    "r_trig",
    "f_trig",
    // Counters (IEC 61131-3 2.5.2.3.3) - all integer type variants
    "ctu",
    "ctu_dint",
    "ctu_lint",
    "ctu_udint",
    "ctu_ulint",
    "ctd",
    "ctd_dint",
    "ctd_lint",
    "ctd_udint",
    "ctd_ulint",
    "ctud",
    "ctud_dint",
    "ctud_lint",
    "ctud_udint",
    "ctud_ulint",
    // Timers (IEC 61131-3 2.5.2.3.4)
    "ton",
    "tof",
    "tp",
};

/// Returns true if the type is a standard library type that is NOT supported.
///
/// This is used by rule_unsupported_stdlib_type to generate an error when
/// a user tries to use an unsupported standard library type.
pub(crate) fn is_unsupported_standard_type(ty: &TypeName) -> bool {
    UNSUPPORTED_STANDARD_LIBRARY_TYPES.contains(ty.name.lower_case().as_str())
}

/// Returns true if the type is a standard library type that IS supported.
///
/// Supported types are available in the type environment and don't require
/// user definition.
#[allow(dead_code)]
pub(crate) fn is_supported_standard_type(ty: &TypeName) -> bool {
    SUPPORTED_STANDARD_LIBRARY_TYPES.contains(ty.name.lower_case().as_str())
}

/// Returns true if the type is any standard library type (supported or not).
#[allow(dead_code)]
pub(crate) fn is_standard_library_type(ty: &TypeName) -> bool {
    is_supported_standard_type(ty) || is_unsupported_standard_type(ty)
}
