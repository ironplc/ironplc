//! End-to-end tests for bit access and partial access on each shape of base:
//! a variable, an array element, a structure field, an element of an array in
//! a structure, and a field of a structure in an array. Each access is read
//! and written on each base, at 32 and at 64 bits.

use crate::common::Snapshot;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

/// Declares a base of each shape at each width, runs `body`, and returns the
/// variables. `i` is 1, so a subscript is computed at run time.
fn run_on_bases(body: &str) -> Snapshot {
    let source = format!(
        "
TYPE ELEMENT : STRUCT d : DWORD; l : LWORD; END_STRUCT; END_TYPE
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
    es : ARRAY[0..1] OF ELEMENT;
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
    #[values("d", "ds[i]", "s.d", "s.ds[i]", "es[i].d")] base: &str,
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
    #[values("l", "ls[i]", "s.l", "s.ls[i]", "es[i].l")] base: &str,
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
    #[values("d", "ds[i]", "s.d", "s.ds[i]", "es[i].d")] base: &str,
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
    #[values("l", "ls[i]", "s.l", "s.ls[i]", "es[i].l")] base: &str,
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

/// A `VAR_IN_OUT` base is read and written through the caller's variable.
#[spec_test(REQ_PAB_codegen_132)]
#[test]
fn partial_access_when_base_is_in_out_then_caller_variable_changes() {
    let source = "
FUNCTION SET_BITS : BOOL
  VAR_IN_OUT d : DWORD; l : LWORD; END_VAR
  d.0 := TRUE;
  d.%B2 := BYTE#16#FF;
  l.40 := FALSE;
  l.%D1 := DWORD#16#CAFEF00D;
  SET_BITS := d.3;
END_FUNCTION
PROGRAM main
  VAR d : DWORD; l : LWORD; r : BOOL; END_VAR
  d := DWORD#16#12345678;
  l := LWORD#16#0123456789ABCDEF;
  r := SET_BITS(d := d, l := l);
END_PROGRAM
";
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);
    assert_eq!(snapshot.read_as::<u32>("d"), 0x12FF_5679);
    assert_eq!(snapshot.read_as::<u64>("l"), 0xCAFE_F00D_89AB_CDEF);
    assert_eq!(snapshot.read_as::<u64>("r"), 1);
}

/// An element of an array reached through a `REF_TO ARRAY` is written
/// through the reference, in the caller's array.
#[spec_test(REQ_PAB_codegen_132)]
#[test]
fn partial_access_when_base_is_element_through_reference_then_referenced_array_changes() {
    let source = "
FUNCTION SET_BITS : BOOL
  VAR_INPUT p : REF_TO ARRAY[0..1] OF DWORD; END_VAR
  p^[1].0 := TRUE;
  p^[1].%B2 := BYTE#16#FF;
  SET_BITS := p^[1].3;
END_FUNCTION
PROGRAM main
  VAR arr : ARRAY[0..1] OF DWORD; r_dword : DWORD; r : BOOL; END_VAR
  arr[1] := DWORD#16#12345678;
  r := SET_BITS(p := REF(arr));
  r_dword := arr[1];
END_PROGRAM
";
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);
    assert_eq!(snapshot.read_as::<u32>("r_dword"), 0x12FF_5679);
    assert_eq!(snapshot.read_as::<u64>("r"), 1);
}

/// An element of an array inside each element of an array of structures is
/// read and written at its own width.
#[spec_test(REQ_PAB_codegen_132)]
#[test]
fn partial_access_when_base_is_array_in_array_of_structures_then_element_width() {
    let source = "
TYPE ELEMENT : STRUCT vals : ARRAY[0..1] OF LWORD; END_STRUCT; END_TYPE
PROGRAM main
  VAR es : ARRAY[0..1] OF ELEMENT; i : DINT := 1; r_lword : LWORD; r : BOOL; END_VAR
  es[i].vals[i] := LWORD#16#0123456789ABCDEF;
  es[i].vals[i].40 := FALSE;
  es[i].vals[i].%B7 := BYTE#16#FF;
  r := es[i].vals[i].42;
  r_lword := es[i].vals[i];
END_PROGRAM
";
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    let snapshot = Snapshot::run(source, &options);
    assert_eq!(snapshot.read_as::<u64>("r_lword"), 0xFF23_4467_89AB_CDEF);
    assert_eq!(snapshot.read_as::<u64>("r"), 1);
}

/// A base that is not a single-slot place is reported as not implemented
/// rather than compiled to write somewhere else.
#[rstest]
#[case::bits_of_bits("d : DWORD;", "d.%B1.3 := TRUE;")]
#[case::through_reference("x : BYTE; p : REF_TO BYTE;", "p := REF(x); p^.3 := TRUE;")]
fn partial_access_when_base_is_not_a_place_then_not_implemented(
    #[case] decls: &str,
    #[case] body: &str,
) {
    let source = format!("PROGRAM main VAR {decls} END_VAR {body} END_PROGRAM");
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let result = crate::common::try_parse_and_compile(&source, &options);
    assert_eq!(result.unwrap_err().code, "P9999");
}
