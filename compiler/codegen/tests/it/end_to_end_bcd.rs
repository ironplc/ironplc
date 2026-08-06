//! End-to-end integration tests for the BCD_TO_INT and INT_TO_BCD functions.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// BCD_TO_INT decodes a packed-BCD bit string into its decimal value. Kept as
/// a parametrized table rather than a property test (no verified Rust BCD
/// oracle). Each case is (bit-string type, result type, hex BCD literal,
/// decoded decimal).
#[rstest]
#[case::byte("BYTE", "USINT", "42", 42)]
#[case::word("WORD", "UINT", "1234", 1234)]
#[case::byte_zero("BYTE", "USINT", "00", 0)]
fn bcd_to_int(
    #[case] bit_ty: &str,
    #[case] result_ty: &str,
    #[case] hex: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    bcd_val : {bit_ty};
    result : {result_ty};
  END_VAR
  bcd_val := {bit_ty}#16#{hex};
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

/// INT_TO_BCD encodes a decimal value into a packed-BCD bit string. Each case
/// is (input type, result bit-string type, decimal input, expected packed-BCD).
#[rstest]
#[case::usint("USINT", "BYTE", "42", 0x42)]
#[case::uint("UINT", "WORD", "1234", 0x1234)]
fn int_to_bcd(
    #[case] int_ty: &str,
    #[case] result_ty: &str,
    #[case] dec: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    int_val : {int_ty};
    result : {result_ty};
  END_VAR
  int_val := {int_ty}#{dec};
  result := INT_TO_BCD(int_val);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

#[test]
fn end_to_end_when_bcd_to_int_roundtrip_then_matches() {
    let source = "
PROGRAM main
  VAR
    original : USINT;
    bcd_val : BYTE;
    result : USINT;
  END_VAR
  original := USINT#73;
  bcd_val := INT_TO_BCD(original);
  result := BCD_TO_INT(bcd_val);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result = bufs.vars[2].as_i32() as u8;
    assert_eq!(result, 73, "expected 73, got {result}");
}
