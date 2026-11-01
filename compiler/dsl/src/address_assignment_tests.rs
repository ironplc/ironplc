//! Tests that [`AddressAssignment::try_from`] reads a direct address the way
//! IEC 61131-3 (B.1.4.1) writes one: each field is an `integer`
//! (`digit {['_'] digit}`) of any length, and the letters are not
//! case-sensitive.

use rstest::rstest;

use crate::common::{AddressAssignment, LocationPrefix, SizePrefix};

#[rstest]
#[case("%MW10", LocationPrefix::M, SizePrefix::W, &[10])]
#[case("%IX12.7", LocationPrefix::I, SizePrefix::X, &[12, 7])]
#[case("%QD100", LocationPrefix::Q, SizePrefix::D, &[100])]
#[case("%IX1.2.3.4", LocationPrefix::I, SizePrefix::X, &[1, 2, 3, 4])]
#[case("%QL4294967295", LocationPrefix::Q, SizePrefix::L, &[u32::MAX])]
#[case("%Q10.2", LocationPrefix::Q, SizePrefix::Nil, &[10, 2])]
#[case("%mb3", LocationPrefix::M, SizePrefix::B, &[3])]
#[case("%MW1_000", LocationPrefix::M, SizePrefix::W, &[1000])]
#[case("%m*", LocationPrefix::M, SizePrefix::Unspecified, &[])]
fn try_from_when_valid_address_then_fields(
    #[case] text: &str,
    #[case] location: LocationPrefix,
    #[case] size: SizePrefix,
    #[case] fields: &[u32],
) {
    let address = AddressAssignment::try_from(text).unwrap();

    assert_eq!(address.location, location);
    assert_eq!(address.size, size);
    assert_eq!(address.address, fields);
}

#[rstest]
#[case::no_field("%MW")]
#[case::empty_field("%IX1..2")]
#[case::trailing_period("%IX1.")]
#[case::leading_underscore("%MW_1")]
#[case::trailing_underscore("%MW1_")]
#[case::double_underscore("%MW1__0")]
#[case::overflow("%MW4294967296")]
#[case::size_before_star("%IX*")]
fn try_from_when_not_a_direct_address_then_err(#[case] text: &str) {
    assert!(AddressAssignment::try_from(text).is_err());
}
