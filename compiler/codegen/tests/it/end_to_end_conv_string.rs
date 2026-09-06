//! End-to-end tests for numeric ↔ STRING type conversions.

use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::parse_and_run;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

// =========================================================================
// <TYPE>_TO_STRING
//
// Integer decimal formatting stays a parametrized table (not a property test)
// so REAL_TO_STRING, whose decimal formatting has no clean Rust oracle, can
// share the identical scaffold. Each case is (declared type, initial value,
// expected decimal string).
// =========================================================================

#[rstest]
#[case::int_positive("INT", "42", "42")]
#[case::int_negative("INT", "-123", "-123")]
#[case::int_zero("INT", "0", "0")]
#[case::dint_large("DINT", "2147483647", "2147483647")]
#[case::dint_negative("DINT", "-100", "-100")]
#[case::sint_negative("SINT", "-7", "-7")]
#[case::usint("USINT", "255", "255")]
#[case::uint("UINT", "65535", "65535")]
#[case::udint_large("UDINT", "4294967295", "4294967295")]
#[case::dword("DWORD", "255", "255")]
#[case::word("WORD", "1000", "1000")]
#[case::byte("BYTE", "42", "42")]
// Rust formats 3.5_f32 as "3.5", -0.5 as "-0.5", and 100.0 as "100".
#[case::real_positive("REAL", "3.5", "3.5")]
#[case::real_negative("REAL", "-0.5", "-0.5")]
#[case::real_integer_value("REAL", "100.0", "100")]
fn num_to_string(#[case] ty: &str, #[case] value: &str, #[case] expected: &str) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {ty} := {value};
    s : STRING;
  END_VAR
  s := {ty}_TO_STRING(x);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), expected);
}

// =========================================================================
// STRING_TO_<INTEGER>
//
// Parses a STRING literal into an integer slot. Invalid input yields 0.
// =========================================================================

#[rstest]
#[case::int_valid("INT", "123", 123)]
#[case::int_negative("INT", "-456", -456)]
#[case::int_invalid("INT", "abc", 0)]
#[case::dint_large("DINT", "2147483647", 2147483647)]
fn string_to_int(#[case] tgt: &str, #[case] input: &str, #[case] expected: i32) {
    let source = format!(
        "
PROGRAM main
  VAR
    s : STRING := '{input}';
    x : {tgt};
  END_VAR
  x := STRING_TO_{tgt}(s);
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// =========================================================================
// STRING_TO_REAL
//
// Kept as distinct assertions: the valid case needs a tolerance compare,
// the invalid case an exact-zero compare.
// =========================================================================

e2e_f32_near!(
    string_to_real_when_valid_then_parsed,
    1e-5,
    "
PROGRAM main
  VAR
    s : STRING := '2.5';
    x : REAL;
  END_VAR
  x := STRING_TO_REAL(s);
END_PROGRAM
",
    &[(1, 2.5)],
);

e2e_f32!(
    string_to_real_when_invalid_then_zero,
    "
PROGRAM main
  VAR
    s : STRING := 'xyz';
    x : REAL;
  END_VAR
  x := STRING_TO_REAL(s);
END_PROGRAM
",
    &[(1, 0.0)],
);

#[test]
fn conversion_result_when_used_as_a_string_operand_then_narrow_encoding() {
    // A conversion builds a Latin-1 string, and nothing in the call spells
    // that out: it is neither a literal nor a declared variable, and neither
    // of the two encodings a string function preserves. The return type the
    // analyzer gave the call is what says so.
    let source = "
PROGRAM main
  VAR
    i : INT := 42;
    out : STRING[20];
  END_VAR
  out := CONCAT(INT_TO_STRING(i), 'x');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // `out` is the first string in the data region; CONCAT's temporaries
    // follow it.
    assert_eq!(read_string(&bufs.data_region, 0), "42x");
}
