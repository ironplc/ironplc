//! End-to-end integration tests for the BCD_TO_INT and INT_TO_BCD functions.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{bit_string_options, compile_by_route, parse, Snapshot};

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
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i32>("result"), expected);
}

/// An integer literal given to BCD_TO_INT, where the dialect lets one stand
/// for a bit string, is the DWORD it spells (REQ-IC-analyzer-118): `18` is
/// 16#12. Both routes decode it at 32 bits, to the same code.
#[rstest]
#[case::hexadecimal("16#1234", 1234)]
#[case::decimal("18", 12)]
fn run_when_bcd_to_int_of_integer_literal_then_decoded_as_dword(
    #[case] literal: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "PROGRAM main VAR result : DINT; END_VAR result := BCD_TO_INT({literal}); END_PROGRAM"
    );
    let options = bit_string_options();
    let snapshot = Snapshot::run(&source, &options);
    assert_eq!(snapshot.read_as::<i32>("result"), expected);
    let (library, context) = parse(&source, &options);
    let code = |route| {
        let container = compile_by_route(&library, &context, &options, route).expect("compiles");
        format!("{:?} {:?}", container.code, container.constant_pool)
    };
    assert_eq!(code(Route::RequireLowered), code(Route::Ast));
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
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i32>("result"), expected);
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
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    let result = snapshot.read_as::<u8>("result");
    assert_eq!(result, 73, "expected 73, got {result}");
}
