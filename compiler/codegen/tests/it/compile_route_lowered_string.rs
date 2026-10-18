//! Tests of string assignments, of strings that are elements of arrays or
//! fields of structures, of the standard string functions, of conversions to
//! and from a `STRING` and of comparisons of strings, compiled from the
//! lowered program, with every statement required to lower.

use rstest::rstest;

use crate::compile_route_lowered::{assert_same_code, run_lowered};

/// A literal and the string at a place are stored at a string place. A
/// value longer than the place keeps its first code units
/// (REQ-LOW-codegen-056), and a `WSTRING` keeps a character a `STRING`
/// cannot hold.
#[test]
fn run_when_strings_assigned_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING[5]; t : STRING := 'hello world'; u : STRING; v : STRING;
               w : WSTRING; END_VAR
           s := t;
           u := 'abc';
           v := s;
           w := \"π ≈ 3.14\";
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("s"), "hello");
    assert_eq!(snapshot.read("u"), "abc");
    assert_eq!(snapshot.read("v"), "hello");
    assert_eq!(snapshot.read("w"), "π ≈ 3.14");
}

/// A string assignment compiles to the same instructions and constants by
/// both routes.
#[rstest]
#[case::literal("s := 'abc';")]
#[case::empty_literal("s := '';")]
#[case::variable("s := t;")]
#[case::to_shorter("n := t;")]
#[case::wide_literal("w := \"xyz\";")]
#[case::wide_variable("w := x;")]
fn compile_when_string_assignment_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING; n : STRING[3]; w : WSTRING; x : WSTRING; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// The types the tests of string elements and fields use: a structure with a
/// string field and a string array field, and one with an array of those.
const STRING_PLACES: &str = "
    TYPE Item : STRUCT label : STRING[6]; tags : ARRAY[1..2] OF STRING[4]; END_STRUCT; END_TYPE
    TYPE Holder : STRUCT items : ARRAY[1..2] OF Item; END_STRUCT; END_TYPE";

/// A string is read and written as an element of an array, a field of a
/// structure, an element of a structure's array field and a field of an
/// element of an array of structures, each keeping the code units its
/// capacity holds (REQ-LOW-codegen-056), and is passed from there to a string
/// function.
#[test]
fn run_when_string_elements_and_fields_then_values() {
    let snapshot = run_lowered(&format!(
        "{STRING_PLACES}
         PROGRAM main
           VAR names : ARRAY[1..3] OF STRING[8]; p : Item; h : Holder; i : DINT := 2;
               r1, r2, r3, r4, r5 : STRING; n : DINT; END_VAR
           names[i] := 'abcdefghij';
           p.label := 'label';
           p.tags[i] := 'tag';
           h.items[i].label := names[i];
           r1 := names[2];
           r2 := p.label;
           r3 := p.tags[2];
           r4 := h.items[2].label;
           r5 := CONCAT(p.label, names[i]);
           n := LEN(names[i]);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read("r1"), "abcdefgh");
    assert_eq!(snapshot.read("r2"), "label");
    assert_eq!(snapshot.read("r3"), "tag");
    assert_eq!(snapshot.read("r4"), "abcdef");
    assert_eq!(snapshot.read("r5"), "labelabcdefgh");
    assert_eq!(snapshot.read_as::<i32>("n"), 8);
}

/// A string element or field is stored, loaded and passed to a string
/// function with the same instructions and constants by both routes.
#[rstest]
#[case::element_store("names[i] := s;")]
#[case::element_load("s := names[2];")]
#[case::field_store("p.label := 'x';")]
#[case::field_load("s := p.label;")]
#[case::element_of_field("p.tags[i] := p.label;")]
#[case::field_of_element("h.items[i].label := s;")]
#[case::len_of_element("n := LEN(names[i]);")]
#[case::concat_of_field_and_element("s := CONCAT(p.label, names[1]);")]
fn compile_when_string_element_or_field_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "{STRING_PLACES}
         PROGRAM main
           VAR names : ARRAY[1..3] OF STRING[8]; p : Item; h : Holder; i : DINT; s : STRING;
               n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// `LEN` counts the code units of its string, and `FIND` gives the 1-based
/// position of the first occurrence of its second string in its first, or 0
/// when there is none or the string it looks for is empty
/// (REQ-LOW-codegen-089). A `WSTRING` counts 16-bit code units.
#[test]
fn run_when_len_or_find_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING[8] := 'hello'; w : WSTRING := \"π ≈ 3.14\";
               r1, r2, r3, r4, r5, r6, r7 : DINT; END_VAR
           r1 := LEN(s);
           r2 := LEN('');
           r3 := FIND(s, 'll');
           r4 := FIND(s, 'xyz');
           r5 := FIND(s, '');
           r6 := LEN(w);
           r7 := FIND(w, \"3.14\");
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("r1"), 5);
    assert_eq!(snapshot.read_as::<i32>("r2"), 0);
    assert_eq!(snapshot.read_as::<i32>("r3"), 3);
    assert_eq!(snapshot.read_as::<i32>("r4"), 0);
    assert_eq!(snapshot.read_as::<i32>("r5"), 0);
    assert_eq!(snapshot.read_as::<i32>("r6"), 8);
    assert_eq!(snapshot.read_as::<i32>("r7"), 5);
}

/// A call of `LEN` or `FIND` compiles to the same instructions and constants
/// by both routes: the string at a variable is passed as the variable's own
/// slot, and any other string is first stored in a slot of its capacity.
#[rstest]
#[case::len_of_variable("n := LEN(s);")]
#[case::len_of_literal("n := LEN('abc');")]
#[case::find_of_variables("n := FIND(s, t);")]
#[case::find_of_literal("n := FIND(s, 'b');")]
#[case::wide_len("n := LEN(w);")]
#[case::wide_find("n := FIND(w, \"y\");")]
fn compile_when_len_or_find_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING[3]; w : WSTRING; n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// Each string function that yields a string gives what the standard says.
/// A length is cut at the end of its string and a negative one counts as 0;
/// a position below 1 counts as 1, and one past the end as the end
/// (REQ-LOW-codegen-089). A result longer than its place keeps its first
/// code units (REQ-LOW-codegen-056).
#[test]
fn run_when_string_functions_yield_strings_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := 'hello'; w : WSTRING := \"π ≈ 3.14\"; short : STRING[4];
               r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11 : STRING;
               w1, w2 : WSTRING; n : DINT; END_VAR
           r1 := CONCAT(s, ' world');
           r2 := LEFT(s, 2);
           r3 := RIGHT(s, 3);
           r4 := MID(s, 3, 2);
           r5 := INSERT(s, 'XY', 2);
           r6 := DELETE(s, 2, 3);
           r7 := REPLACE(s, 'J', 1, 1);
           r8 := LEFT(s, 9);
           r9 := LEFT(s, -1);
           r10 := MID(s, 2, 0);
           r11 := INSERT(s, '!', 9);
           short := CONCAT(s, s);
           n := LEN(CONCAT(LEFT(s, 2), RIGHT(s, 2)));
           w1 := CONCAT(w, \"!\");
           w2 := MID(w, 1, 3);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("r1"), "hello world");
    assert_eq!(snapshot.read("r2"), "he");
    assert_eq!(snapshot.read("r3"), "llo");
    assert_eq!(snapshot.read("r4"), "ell");
    assert_eq!(snapshot.read("r5"), "heXYllo");
    assert_eq!(snapshot.read("r6"), "heo");
    assert_eq!(snapshot.read("r7"), "Jello");
    assert_eq!(snapshot.read("r8"), "hello");
    assert_eq!(snapshot.read("r9"), "");
    assert_eq!(snapshot.read("r10"), "he");
    assert_eq!(snapshot.read("r11"), "hello!");
    assert_eq!(snapshot.read("short"), "hell");
    assert_eq!(snapshot.read_as::<i32>("n"), 4);
    assert_eq!(snapshot.read("w1"), "π ≈ 3.14!");
    assert_eq!(snapshot.read("w2"), "≈");
}

/// A call of a string function that yields a string compiles to the same
/// instructions and constants by both routes, its operands passed as `LEN`'s
/// are, and its result stored, or passed to another call in a slot of its
/// capacity.
#[rstest]
#[case::concat("s := CONCAT(s, t);")]
#[case::concat_literal("s := CONCAT(t, 'ab');")]
#[case::insert("s := INSERT(s, t, 2);")]
#[case::replace("s := REPLACE(s, t, 1, 2);")]
#[case::delete("s := DELETE(s, 1, 2);")]
#[case::left("s := LEFT(t, 2);")]
#[case::right("s := RIGHT(t, 2);")]
#[case::mid("s := MID(t, 2, 1);")]
#[case::nested("s := CONCAT(LEFT(s, 1), RIGHT(t, 1));")]
#[case::len_of_call("n := LEN(CONCAT(s, t));")]
#[case::wide_concat("w := CONCAT(w, \"x\");")]
#[case::wide_mid("w := MID(w, 1, 1);")]
fn compile_when_string_function_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING[3]; w : WSTRING; n : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// A conversion from a `STRING` reads a number of its target type, the
/// whole string less surrounding whitespace under the default policy; one to
/// a `STRING` writes its number's decimal text (REQ-LOW-codegen-089).
#[test]
fn run_when_string_conversion_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := ' 42 '; f : STRING := '1.5'; n : DINT; u : UDINT; i : SINT;
               r : REAL; l : LREAL; q : LINT; neg : INT := -7; big : UDINT := 4000000000;
               x : REAL := 2.5; t1, t2, t3 : STRING; END_VAR
           n := STRING_TO_DINT(s);
           u := STRING_TO_UDINT('4000000000');
           i := STRING_TO_SINT('-12');
           r := STRING_TO_REAL(f);
           l := STRING_TO_LREAL('2.25');
           q := STRING_TO_LINT('-9000000000');
           t1 := INT_TO_STRING(neg);
           t2 := UDINT_TO_STRING(big);
           t3 := REAL_TO_STRING(x);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("n"), 42);
    assert_eq!(snapshot.read_as::<u64>("u"), 4_000_000_000);
    assert_eq!(snapshot.read_as::<i32>("i"), -12);
    assert_eq!(snapshot.read_as::<f32>("r"), 1.5);
    assert_eq!(snapshot.read_as::<f64>("l"), 2.25);
    assert_eq!(snapshot.read_as::<i64>("q"), -9_000_000_000);
    assert_eq!(snapshot.read("t1"), "-7");
    assert_eq!(snapshot.read("t2"), "4000000000");
    assert_eq!(snapshot.read("t3"), "2.5");
}

/// A conversion to or from a `STRING` compiles to the same instructions and
/// constants by both routes: a string read as a number is passed by the
/// offset of its slot, and a number written as a string is a temporary
/// buffer.
#[rstest]
#[case::to_dint("n := STRING_TO_DINT(s);")]
#[case::to_udint_of_literal("u := STRING_TO_UDINT('7');")]
#[case::to_lint("q := STRING_TO_LINT(s);")]
#[case::to_real("r := STRING_TO_REAL(s);")]
#[case::to_lreal("l := STRING_TO_LREAL(s);")]
#[case::to_dint_of_call("n := STRING_TO_DINT(LEFT(s, 2));")]
#[case::from_dint("s := DINT_TO_STRING(n);")]
#[case::from_udint("s := UDINT_TO_STRING(u);")]
#[case::from_real("s := REAL_TO_STRING(r);")]
#[case::from_dint_in_call("s := CONCAT(DINT_TO_STRING(n), 'x');")]
fn compile_when_string_conversion_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; n : DINT; u : UDINT; q : LINT; r : REAL; l : LREAL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// Of two strings, the first code unit that differs decides their order, and
/// a string that is a prefix of the other is less (REQ-LOW-codegen-089).
#[test]
fn run_when_strings_compared_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := 'abc'; t : STRING := 'abd'; w : WSTRING := \"xy\";
               eq, ne, lt, le, gt, ge, prefix, wide, cond : BOOL; END_VAR
           eq := s = 'abc';
           ne := s <> t;
           lt := s < t;
           le := LE(t, s);
           gt := CONCAT(s, 'x') > s;
           ge := s >= 'abc';
           prefix := 'ab' < s;
           wide := w > \"x\";
           IF s < t THEN cond := TRUE; END_IF;
         END_PROGRAM",
    );
    let read = |name| snapshot.read_as::<u64>(name);
    assert_eq!(read("eq"), 1);
    assert_eq!(read("ne"), 1);
    assert_eq!(read("lt"), 1);
    assert_eq!(read("le"), 0);
    assert_eq!(read("gt"), 1);
    assert_eq!(read("ge"), 1);
    assert_eq!(read("prefix"), 1);
    assert_eq!(read("wide"), 1);
    assert_eq!(read("cond"), 1);
}

/// A comparison of strings compiles to the same instructions and constants
/// by both routes: its strings are passed as `LEN`'s are, and the order the
/// builtin gives them is compared with zero.
#[rstest]
#[case::eq_literal("x := s = 'abc';")]
#[case::lt_variables("x := s < t;")]
#[case::function_form("x := GE(s, t);")]
#[case::literal_first("x := 'abc' <> s;")]
#[case::call_operand("x := CONCAT(s, t) > s;")]
#[case::wide("x := w <= w;")]
#[case::if_condition("IF s = t THEN x := TRUE; END_IF;")]
fn compile_when_string_comparison_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR s : STRING; t : STRING; w : WSTRING; x : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// A string declared through a type alias of `STRING` or `WSTRING`, as a
/// function's or method's input, local or result, is held as a string of
/// the alias's capacity, as one spelled out is: it is copied in, starts
/// empty or at its initial value on every call, and keeps no more than its
/// capacity. The route that reads the AST kept such a variable as the index
/// of a temporary buffer, which trapped when read before it was written.
#[test]
fn run_when_string_declared_through_alias_in_function_then_string_values() {
    let snapshot = run_lowered(
        "TYPE Name : STRING[20]; END_TYPE
         TYPE Short : STRING[3]; END_TYPE
         TYPE WName : WSTRING[10]; END_TYPE
         FUNCTION FIN : DINT VAR_INPUT n : Name; END_VAR FIN := LEN(n); END_FUNCTION
         FUNCTION FCOPY : DINT VAR_INPUT n : STRING[20]; END_VAR VAR l : Name; END_VAR
           l := n;
           FCOPY := LEN(l);
         END_FUNCTION
         FUNCTION FEMPTY : DINT VAR_INPUT x : DINT; END_VAR VAR l : Name; END_VAR
           FEMPTY := LEN(l);
         END_FUNCTION
         FUNCTION FINIT : DINT VAR_INPUT x : DINT; END_VAR VAR l : Name := 'ab'; END_VAR
           FINIT := LEN(l);
         END_FUNCTION
         FUNCTION FRES : Short VAR_INPUT x : DINT; END_VAR FRES := 'abcdef'; END_FUNCTION
         FUNCTION FWIDE : DINT VAR_INPUT n : WName; END_VAR VAR l : WName; END_VAR
           l := n;
           FWIDE := LEN(l);
         END_FUNCTION
         FUNCTION_BLOCK B
           METHOD M : DINT VAR_INPUT n : Name; END_VAR VAR l : Name; END_VAR
             l := n;
             M := LEN(l);
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR s : Name := 'abcd'; ws : WName := \"abcdef\"; b : B; result : STRING[20];
               input : DINT; copied : DINT; empty : DINT; init : DINT; result_len : DINT;
               wide : DINT; in_method : DINT; END_VAR
           input := FIN(n := s);
           copied := FCOPY(n := s);
           empty := FEMPTY(x := 1);
           init := FINIT(x := 1);
           result := FRES(x := 1);
           result_len := LEN(FRES(x := 1));
           wide := FWIDE(n := ws);
           in_method := b.M(n := s);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("input"), 4);
    assert_eq!(snapshot.read_as::<i32>("copied"), 4);
    assert_eq!(snapshot.read_as::<i32>("empty"), 0);
    assert_eq!(snapshot.read_as::<i32>("init"), 2);
    assert_eq!(snapshot.read("result"), "abc");
    assert_eq!(snapshot.read_as::<i32>("result_len"), 3);
    assert_eq!(snapshot.read_as::<i32>("wide"), 6);
    assert_eq!(snapshot.read_as::<i32>("in_method"), 4);
}
