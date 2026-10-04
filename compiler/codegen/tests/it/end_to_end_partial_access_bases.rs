//! End-to-end tests for bit access and partial access on each shape of base:
//! a variable, an array element, a structure field, and an element of an
//! array in a structure. Each access is read and written on each base, at 32
//! and at 64 bits.

use crate::common::Snapshot;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

/// Declares a base of each shape at each width, runs `body`, and returns the
/// variables. `i` is 1, so a subscript is computed at run time.
fn run_on_bases(body: &str) -> Snapshot {
    let source = format!(
        "
TYPE HOLDER : STRUCT
    d : DWORD;
    l : LWORD;
    ds : ARRAY[0..1] OF DWORD;
    ls : ARRAY[0..1] OF LWORD;
END_STRUCT; END_TYPE
PROGRAM main
  VAR
    i : DINT := 1;
    d : DWORD;
    l : LWORD;
    ds : ARRAY[0..1] OF DWORD;
    ls : ARRAY[0..1] OF LWORD;
    s : HOLDER;
    r_bit : BOOL;
    r_byte : BYTE;
    r_word : WORD;
    r_dword : DWORD;
    r_lword : LWORD;
  END_VAR
  {body}
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    Snapshot::run(&source, &options)
}

/// Writing a bit or a slice of a 32-bit base changes only the selected bits.
#[spec_test(REQ_PAB_codegen_132)]
#[rstest]
fn partial_access_when_write_32_bit_base_then_only_selected_bits_change(
    #[values("d", "ds[i]", "s.d", "s.ds[i]")] base: &str,
    #[values(
        (".0", "TRUE", 0x1234_5679),
        (".3", "FALSE", 0x1234_5670),
        (".%X31", "TRUE", 0x9234_5678),
        (".%B2", "BYTE#16#FF", 0x12FF_5678),
        (".%W1", "WORD#16#BEEF", 0xBEEF_5678),
        (".%D0", "DWORD#16#CAFEF00D", 0xCAFE_F00D)
    )]
    access: (&str, &str, u32),
) {
    let (selector, value, expected) = access;
    let snapshot = run_on_bases(&format!(
        "{base} := DWORD#16#12345678; {base}{selector} := {value}; r_dword := {base};"
    ));
    assert_eq!(snapshot.read_as::<u32>("r_dword"), expected);
}

/// Writing a bit or a slice of a 64-bit base changes only the selected bits.
#[spec_test(REQ_PAB_codegen_132)]
#[rstest]
fn partial_access_when_write_64_bit_base_then_only_selected_bits_change(
    #[values("l", "ls[i]", "s.l", "s.ls[i]")] base: &str,
    #[values(
        (".0", "FALSE", 0x0123_4567_89AB_CDEE),
        (".40", "FALSE", 0x0123_4467_89AB_CDEF),
        (".%X63", "TRUE", 0x8123_4567_89AB_CDEF),
        (".%B7", "BYTE#16#FF", 0xFF23_4567_89AB_CDEF),
        (".%W2", "WORD#16#BEEF", 0x0123_BEEF_89AB_CDEF),
        (".%D1", "DWORD#16#CAFEF00D", 0xCAFE_F00D_89AB_CDEF),
        (".%L0", "LWORD#16#FEDCBA9876543210", 0xFEDC_BA98_7654_3210)
    )]
    access: (&str, &str, u64),
) {
    let (selector, value, expected) = access;
    let snapshot = run_on_bases(&format!(
        "{base} := LWORD#16#0123456789ABCDEF; {base}{selector} := {value}; r_lword := {base};"
    ));
    assert_eq!(snapshot.read_as::<u64>("r_lword"), expected);
}

/// Reading a bit or a slice of a 32-bit base yields the selected bits.
#[spec_test(REQ_PAB_codegen_132)]
#[rstest]
fn partial_access_when_read_32_bit_base_then_selected_bits(
    #[values("d", "ds[i]", "s.d", "s.ds[i]")] base: &str,
    #[values(
        (".3", "r_bit", 1),
        (".0", "r_bit", 0),
        (".%X28", "r_bit", 1),
        (".%B2", "r_byte", 0x34),
        (".%W1", "r_word", 0x1234),
        (".%D0", "r_dword", 0x1234_5678)
    )]
    access: (&str, &str, u64),
) {
    let (selector, result, expected) = access;
    let snapshot = run_on_bases(&format!(
        "{base} := DWORD#16#12345678; {result} := {base}{selector};"
    ));
    assert_eq!(snapshot.read_as::<u64>(result), expected);
}

/// Reading a bit or a slice of a 64-bit base yields the selected bits.
#[spec_test(REQ_PAB_codegen_132)]
#[rstest]
fn partial_access_when_read_64_bit_base_then_selected_bits(
    #[values("l", "ls[i]", "s.l", "s.ls[i]")] base: &str,
    #[values(
        (".0", "r_bit", 1),
        (".41", "r_bit", 0),
        (".%X56", "r_bit", 1),
        (".%B6", "r_byte", 0x23),
        (".%W2", "r_word", 0x4567),
        (".%D1", "r_dword", 0x0123_4567),
        (".%L0", "r_lword", 0x0123_4567_89AB_CDEF)
    )]
    access: (&str, &str, u64),
) {
    let (selector, result, expected) = access;
    let snapshot = run_on_bases(&format!(
        "{base} := LWORD#16#0123456789ABCDEF; {result} := {base}{selector};"
    ));
    assert_eq!(snapshot.read_as::<u64>(result), expected);
}
