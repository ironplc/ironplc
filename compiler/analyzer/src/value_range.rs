//! The values a type can hold.
//!
//! A type's range is what decides whether a constant can be stored in it.
//! `USINT` holds 0 through 255, so `300` is not a `USINT` however the compiler
//! chooses to operate on one. The range comes from the type's own record, so
//! the analyzer checking a value and the backend storing it work from one
//! statement of what the type is.

use crate::intermediate_type::IntermediateType;

/// The inclusive `(minimum, maximum)` of a two's-complement integer.
///
/// `bits` is a value width rather than a footprint: what the type can hold,
/// not what it occupies. The two coincide for every elementary integer type.
///
/// The widest case, an unsigned 64-bit type, has a maximum of `u64::MAX`,
/// which is why these are `i128` rather than `i64`.
pub fn for_integer(bits: u32, signed: bool) -> (i128, i128) {
    debug_assert!(bits > 0 && bits <= 64, "unsupported integer width");
    if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    }
}

/// The inclusive range of values `representation` can hold, or `None` when it
/// does not hold integers as numbers.
///
/// A bit string (`BYTE`, `WORD`, `DWORD`, `LWORD`) answers `None` even though
/// it holds an integer: it is a pattern rather than a magnitude, and wrapping
/// one is a legitimate thing for a program to want.
pub fn of(representation: &IntermediateType) -> Option<(i128, i128)> {
    match representation {
        IntermediateType::Int { size } => Some(for_integer(u32::from(size.as_bytes()) * 8, true)),
        IntermediateType::UInt { size } => Some(for_integer(u32::from(size.as_bytes()) * 8, false)),
        // A subrange states its own bounds, which are narrower than the base
        // type's by construction (`rule_decl_subrange_limits` rejects the
        // rest).
        IntermediateType::Subrange {
            min_value,
            max_value,
            ..
        } => Some((*min_value, *max_value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::ByteSized;
    use rstest::rstest;

    #[rstest]
    #[case::sint(8, true, -128, 127)]
    #[case::int(16, true, -32_768, 32_767)]
    #[case::dint(32, true, -2_147_483_648, 2_147_483_647)]
    #[case::lint(64, true, i64::MIN as i128, i64::MAX as i128)]
    #[case::usint(8, false, 0, 255)]
    #[case::uint(16, false, 0, 65_535)]
    #[case::udint(32, false, 0, 4_294_967_295)]
    #[case::ulint(64, false, 0, u64::MAX as i128)]
    fn for_integer_when_width_then_two_complement_bounds(
        #[case] bits: u32,
        #[case] signed: bool,
        #[case] minimum: i128,
        #[case] maximum: i128,
    ) {
        assert_eq!(for_integer(bits, signed), (minimum, maximum));
    }

    #[test]
    fn of_when_signed_integer_then_signed_range() {
        let representation = IntermediateType::Int {
            size: ByteSized::B8,
        };

        assert_eq!(of(&representation), Some((-128, 127)));
    }

    #[test]
    fn of_when_unsigned_integer_then_unsigned_range() {
        let representation = IntermediateType::UInt {
            size: ByteSized::B8,
        };

        assert_eq!(of(&representation), Some((0, 255)));
    }

    #[test]
    fn of_when_subrange_then_declared_bounds() {
        let representation = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: -10,
            max_value: 10,
        };

        assert_eq!(of(&representation), Some((-10, 10)));
    }

    /// A bit string holds an integer but is a pattern rather than a
    /// magnitude, so it has no range for this purpose.
    #[test]
    fn of_when_bit_string_then_none() {
        let representation = IntermediateType::Bytes {
            size: ByteSized::B8,
        };

        assert_eq!(of(&representation), None);
    }

    #[test]
    fn of_when_not_an_integer_then_none() {
        assert_eq!(of(&IntermediateType::Bool), None);
        assert_eq!(
            of(&IntermediateType::Real {
                size: ByteSized::B32
            }),
            None
        );
    }
}
