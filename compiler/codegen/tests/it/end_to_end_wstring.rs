//! End-to-end integration tests for WSTRING (UTF-16LE) support.
//!
//! These compile and execute real `.st` programs through the full pipeline
//! (parse → analyze → codegen → VM) and read the results by name.
//! `compile_wstring.rs` checks how a WSTRING is stored (UTF-16LE per ADR-0016).

use ironplc_container::debug_section::iec_type_tag;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::common::{parse_and_compile, Snapshot};

#[test]
fn wstring_when_literal_initializer_then_reads_literal() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("ws"), "hi");
}

#[test]
fn wstring_when_non_ascii_bmp_literal_then_reads_its_characters() {
    // U+00E9 (é) and U+20AC (€) are BMP code points needing the high byte.
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"é€\";
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("ws"), "é€");
}

#[test]
fn wstring_when_assigned_from_wstring_var_then_value_copied() {
    let source = "
PROGRAM main
  VAR
    src : WSTRING[10] := \"abc\";
    dst : WSTRING[10];
  END_VAR
  dst := src;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("dst"), "abc");
}

#[test]
fn wstring_when_literal_assignment_statement_then_value_stored() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[10];
  END_VAR
  ws := \"world\";
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("ws"), "world");
}

// BOOL true = 1, false = 0.
e2e_i32!(
    wstring_when_compared_equal_then_eq_true_and_ne_false,
    "
PROGRAM main
  VAR
    a : WSTRING[10] := \"abc\";
    b : WSTRING[10] := \"abc\";
    eq : BOOL;
    ne : BOOL;
  END_VAR
  eq := a = b;
  ne := a <> b;
END_PROGRAM
",
    &[("eq", 1), ("ne", 0)],
);

e2e_i32!(
    wstring_when_compared_different_then_eq_false_and_ne_true,
    "
PROGRAM main
  VAR
    a : WSTRING[10] := \"abc\";
    b : WSTRING[10] := \"abd\";
    eq : BOOL;
    ne : BOOL;
  END_VAR
  eq := a = b;
  ne := a <> b;
END_PROGRAM
",
    &[("eq", 0), ("ne", 1)],
);

// LEN counts code units, not bytes.
e2e_i32!(
    wstring_when_len_then_returns_code_unit_count,
    "
PROGRAM main
  VAR
    ws : WSTRING[10] := \"hello\";
    n : DINT;
  END_VAR
  n := LEN(ws);
END_PROGRAM
",
    &[("n", 5)],
);

#[test]
fn wstring_when_concat_then_joins_code_units() {
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"foo\";
    b : WSTRING[10] := \"bar\";
    out : WSTRING[20];
  END_VAR
  out := CONCAT(a, b);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("out"), "foobar");
}

#[test]
fn wstring_when_concat_with_literal_then_keeps_wide_characters() {
    // The literal is encoded at the width the source spells, so it stores into
    // a wide destination without an encoding mismatch (ADR-0034).
    let source = "
PROGRAM main
  VAR
    a : WSTRING[10] := \"foo\";
    out : WSTRING[20];
  END_VAR
  out := CONCAT(a, \"€\");
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("out"), "foo€");
}

#[test]
fn wstring_when_concat_of_literals_only_then_keeps_wide_characters() {
    // No WSTRING variable participates in the expression, so the wide temp
    // buffer sizing has to come from the literals themselves.
    let source = "
PROGRAM main
  VAR
    out : WSTRING[20];
  END_VAR
  out := CONCAT(\"é\", \"€\");
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("out"), "é€");
}

#[test]
fn wstring_when_left_right_mid_then_index_by_code_unit() {
    let source = "
PROGRAM main
  VAR
    s : WSTRING[20] := \"abcdef\";
    l : WSTRING[20];
    r : WSTRING[20];
    m : WSTRING[20];
  END_VAR
  l := LEFT(s, 2);
  r := RIGHT(s, 3);
  m := MID(s, 3, 2);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    // LEFT(s,2)="ab"; RIGHT(s,3)="def"; MID(s,3,2)= 3 code units from pos 2 ="bcd".
    assert_eq!(snapshot.read("l"), "ab");
    assert_eq!(snapshot.read("r"), "def");
    assert_eq!(snapshot.read("m"), "bcd");
}

// FIND is 1-based by code unit.
e2e_i32!(
    wstring_when_find_substring_then_returns_code_unit_position,
    "
PROGRAM main
  VAR
    hay : WSTRING[20] := \"abcdef\";
    needle : WSTRING[20] := \"cd\";
    pos : DINT;
  END_VAR
  pos := FIND(hay, needle);
END_PROGRAM
",
    &[("pos", 3)],
);

#[test]
fn wstring_array_when_assigned_and_read_back_then_values_match() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF WSTRING[8];
    first : WSTRING[8];
    result : WSTRING[8];
  END_VAR
  arr[1] := \"one\";
  arr[2] := \"two\";
  first := arr[1];
  result := arr[2];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("first"), "one");
    assert_eq!(snapshot.read("result"), "two");
}

#[test]
fn wstring_array_when_initial_values_then_populated() {
    let source = "
PROGRAM main
  VAR
    days : ARRAY[1..3] OF WSTRING[8] := [\"Mon\", \"Tue\", \"Wed\"];
    r1 : WSTRING[8];
    r2 : WSTRING[8];
    r3 : WSTRING[8];
  END_VAR
  r1 := days[1];
  r2 := days[2];
  r3 := days[3];
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("r1"), "Mon");
    assert_eq!(snapshot.read("r2"), "Tue");
    assert_eq!(snapshot.read("r3"), "Wed");
}

#[test]
fn mixed_string_and_wstring_when_in_one_program_then_independent() {
    let source = "
PROGRAM main
  VAR
    narrow : STRING[10] := 'abc';
    wide : WSTRING[10] := \"abc\";
  END_VAR
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("narrow"), "abc");
    assert_eq!(snapshot.read("wide"), "abc");
}

#[test]
fn wstring_when_declared_then_debug_tag_is_wstring() {
    let source = "
PROGRAM main
  VAR
    x : WSTRING[10] := \"hi\";
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();
    let var = debug.var_names.iter().find(|v| v.name == "x").unwrap();
    assert_eq!(var.type_name, "WSTRING");
    assert_eq!(var.iec_type_tag, iec_type_tag::WSTRING);
}

#[test]
fn string_assigned_wstring_when_analyzed_then_compile_error() {
    // STRING := WSTRING is rejected at compile time (P4034); the runtime
    // encoding-mismatch trap is only defense-in-depth.
    use ironplc_dsl::core::FileId;
    use ironplc_parser::parse_program;
    let source = "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  s := w;
END_PROGRAM
";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (_lib, context) =
        ironplc_analyzer::stages::analyze(&[&library], &CompilerOptions::default()).unwrap();
    assert!(
        context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::StringEncodingMismatch.code()),
        "expected P4034 for STRING := WSTRING"
    );
}

#[test]
fn wstring_returning_call_when_used_as_a_string_operand_then_wide_encoding() {
    // The wide counterpart of the conversion case in end_to_end_conv_string:
    // a call whose result is a string operand of another string function. A
    // user function is the only call that can produce a WSTRING result today
    // -- the *_TO_STRING conversions are all Latin-1, and no *_TO_WSTRING
    // exists -- and its declared return type is what says so.
    let source = "
FUNCTION mk : WSTRING
  VAR_INPUT
    s : WSTRING[10];
  END_VAR
  mk := s;
END_FUNCTION

PROGRAM main
  VAR
    w : WSTRING[10] := \"ab\";
    out : WSTRING[20];
  END_VAR
  out := CONCAT(mk(w), \"cd\");
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("out"), "abcd");
}

// =========================================================================
// Operands of one string operation share an encoding
//
// A literal's delimiter is what types it -- 'abc' is a STRING and "abc" a
// WSTRING (IEC 61131-3 Table 5) -- so operands that disagree have no encoding
// in common, and analysis rejects them (`rule_string_encoding_compat`,
// P4034). These pin that the same operations compile and run when the
// operands do agree.
// =========================================================================

e2e_i32!(
    wstring_when_compared_to_wide_literal_then_eq_true,
    "
PROGRAM main
  VAR
    w : WSTRING[10] := \"abc\";
    eq : BOOL;
    ne : BOOL;
  END_VAR
  eq := w = \"abc\";
  ne := w <> \"abd\";
END_PROGRAM
",
    &[("eq", 1), ("ne", 1)],
);

e2e_i32!(
    wstring_when_find_wide_literal_needle_then_returns_position,
    "
PROGRAM main
  VAR
    hay : WSTRING[20] := \"abcdef\";
    pos : DINT;
  END_VAR
  pos := FIND(hay, \"cd\");
END_PROGRAM
",
    &[("pos", 3)],
);

#[test]
fn wstring_struct_field_when_assigned_literal_then_stored_wide() {
    // U+20AC (€) has no narrow encoding, so it only survives a wide field.
    let source = "
TYPE
  Rec : STRUCT
    label : WSTRING[10];
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : Rec;
    label : WSTRING[10];
  END_VAR
  r.label := \"hi€\";
  label := r.label;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    assert_eq!(snapshot.read("label"), "hi€");
}

e2e_i32!(
    wstring_array_element_when_compared_to_wide_literal_then_eq_true,
    "
PROGRAM main
  VAR
    arr : ARRAY[1..2] OF WSTRING[8];
    eq : BOOL;
  END_VAR
  arr[1] := \"one\";
  eq := arr[1] = \"one\";
END_PROGRAM
",
    &[("eq", 1)],
);

e2e_i32!(
    function_when_parameters_are_string_and_wstring_then_each_copied_at_its_own_width,
    "
FUNCTION same_len : BOOL
  VAR_INPUT
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  same_len := LEN(s) = LEN(w);
END_FUNCTION

PROGRAM main
  VAR
    s : STRING[10] := 'abc';
    w : WSTRING[10] := \"wx\";
    other : WSTRING[10] := \"z\";
    same : BOOL;
    differ : BOOL;
  END_VAR
  s := CONCAT(s, 'd');
  w := CONCAT(w, \"yz\");
  same := same_len(s, w);
  differ := same_len(s, other);
END_PROGRAM
",
    &[("same", 1), ("differ", 0)],
);

// A wide literal passed straight to a WSTRING parameter, with no wide variable
// in between. The analyzer used to type every character-string literal STRING
// and reject this call with P4026.
e2e_i32!(
    function_when_wstring_parameter_given_wide_literal_then_runs,
    "
FUNCTION wide_len : INT
  VAR_INPUT
    w : WSTRING[10];
  END_VAR
  wide_len := LEN(w);
END_FUNCTION

PROGRAM main
  VAR
    n : INT;
  END_VAR
  n := wide_len(\"abcd\");
END_PROGRAM
",
    &[("n", 4)],
);

/// A WSTRING operation in a loop reuses one temp buffer per iteration, the
/// same discipline `end_to_end_string_loop.rs` pins for narrow strings.
/// WSTRING is worth its own case because temp-buffer slots are sized in wide
/// bytes when any wide string is present.
#[test]
fn wstring_when_concat_in_loop_then_runs_every_iteration() {
    let source = "
PROGRAM main
  VAR
    ws : WSTRING[32];
    i : INT;
  END_VAR
  ws := \"\";
  FOR i := 1 TO 200 DO
    ws := CONCAT(ws, \"x\");
  END_FOR;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());

    // 200 iterations, truncated to the declared 32 code units.
    assert_eq!(snapshot.read("ws"), "x".repeat(32));
}

// Storing into a WSTRING element of an array field of a structure produces
// the value at the element's wide encoding. It used to be produced narrow,
// which trapped V9014 (encoding mismatch) on the store. The element is read
// back through LEN and a comparison: assigning it to a WSTRING variable is
// rejected until #2104 is fixed, because analysis types it as a STRING.
e2e_i32!(
    wstring_when_struct_wstring_array_field_written_then_reads_back,
    "
TYPE Holder : STRUCT
  names : ARRAY[1..2] OF WSTRING[10];
END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    r : DINT;
    same : DINT;
    h : Holder;
  END_VAR
  h.names[2] := \"wide\";
  r := LEN(h.names[2]);
  IF h.names[2] = \"wide\" THEN
    same := 1;
  END_IF;
END_PROGRAM
",
    &[("r", 4), ("same", 1)],
);
