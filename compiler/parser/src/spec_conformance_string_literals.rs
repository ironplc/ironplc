//! Spec conformance tests for character string literals (parser-owned
//! requirements): the characters a `$` escape denotes, and the diagnostic for
//! an escape the standard does not define.
//!
//! Each test is annotated with `#[spec_test(REQ_SL_parser_NNN)]`, which adds
//! `#[test]` and references a build-script-generated constant so the test
//! fails to compile if the requirement is removed from the spec. The
//! `all_spec_requirements_have_tests` meta-test in `spec_conformance` asserts
//! every parser-owned requirement has a test.
//!
//! See `specs/design/string-literals.md`.

use std::convert::Infallible;

use dsl::common::CharacterStringLiteral;
use dsl::core::FileId;
use dsl::visitor::Visitor;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::options::CompilerOptions;
use crate::parse_program;

/// Collects the value of every character string literal in a library.
#[derive(Default)]
struct Literals(Vec<Vec<char>>);

impl Visitor<Infallible> for Literals {
    type Value = ();
    fn visit_character_string_literal(
        &mut self,
        node: &CharacterStringLiteral,
    ) -> Result<(), Infallible> {
        self.0.push(node.value.clone());
        Ok(())
    }
}

/// The characters `literal` denotes, parsed as the value assigned in a
/// program.
fn value_of(literal: &str) -> String {
    let source = format!("PROGRAM main VAR s : WSTRING; END_VAR s := {literal}; END_PROGRAM");
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let mut literals = Literals::default();
    let Ok(()) = literals.walk(&library);
    assert_eq!(1, literals.0.len());
    literals.0[0].iter().collect()
}

/// REQ-SL-parser-001: `$$` is `$` in both widths.
#[spec_test(REQ_SL_parser_001)]
fn parser_spec_req_sl_001_dollar() {
    assert_eq!("a$b", value_of("'a$$b'"));
    assert_eq!("a$b", value_of("\"a$$b\""));
}

/// REQ-SL-parser-002: `$L` and `$l` are a line feed.
#[spec_test(REQ_SL_parser_002)]
fn parser_spec_req_sl_002_line_feed() {
    assert_eq!("\n\n", value_of("'$L$l'"));
    assert_eq!("\n\n", value_of("\"$L$l\""));
}

/// REQ-SL-parser-003: `$N` and `$n` are a line feed.
#[spec_test(REQ_SL_parser_003)]
fn parser_spec_req_sl_003_newline() {
    assert_eq!("\n\n", value_of("'$N$n'"));
    assert_eq!("\n\n", value_of("\"$N$n\""));
}

/// REQ-SL-parser-004: `$P` and `$p` are a form feed.
#[spec_test(REQ_SL_parser_004)]
fn parser_spec_req_sl_004_form_feed() {
    assert_eq!("\u{0C}\u{0C}", value_of("'$P$p'"));
    assert_eq!("\u{0C}\u{0C}", value_of("\"$P$p\""));
}

/// REQ-SL-parser-005: `$R` and `$r` are a carriage return.
#[spec_test(REQ_SL_parser_005)]
fn parser_spec_req_sl_005_carriage_return() {
    assert_eq!("\r\r", value_of("'$R$r'"));
    assert_eq!("\r\r", value_of("\"$R$r\""));
}

/// REQ-SL-parser-006: `$T` and `$t` are a tab.
#[spec_test(REQ_SL_parser_006)]
fn parser_spec_req_sl_006_tab() {
    assert_eq!("\t\t", value_of("'$T$t'"));
    assert_eq!("\t\t", value_of("\"$T$t\""));
}

/// REQ-SL-parser-007: `$'` is `'` in both widths.
#[spec_test(REQ_SL_parser_007)]
fn parser_spec_req_sl_007_single_quote() {
    assert_eq!("it's", value_of("'it$'s'"));
    assert_eq!("it's", value_of("\"it$'s\""));
}

/// REQ-SL-parser-008: `$"` is `"` in both widths.
#[spec_test(REQ_SL_parser_008)]
fn parser_spec_req_sl_008_double_quote() {
    assert_eq!("say \"hi\"", value_of("'say $\"hi$\"'"));
    assert_eq!("say \"hi\"", value_of("\"say $\"hi$\"\""));
}

/// REQ-SL-parser-009: `$` and two hex digits in a `STRING`.
#[spec_test(REQ_SL_parser_009)]
fn parser_spec_req_sl_009_two_hex_digits() {
    assert_eq!("Aé", value_of("'$41$e9'"));
}

/// REQ-SL-parser-010: `$` and four hex digits in a `WSTRING`.
#[spec_test(REQ_SL_parser_010)]
fn parser_spec_req_sl_010_four_hex_digits() {
    assert_eq!("A€", value_of("\"$0041$20AC\""));
}

/// REQ-SL-parser-020: Any other `$` sequence is P0012.
#[rstest]
#[case::unknown_letter("'$q'")]
#[case::one_hex_digit("'$4'")]
#[case::two_hex_digits_in_wide("\"$41\"")]
#[case::surrogate("\"$D800\"")]
#[spec_test(REQ_SL_parser_020)]
fn parser_spec_req_sl_020_undefined_escape_is_p0012(#[case] literal: &str) {
    let source = format!("PROGRAM main VAR s : WSTRING; END_VAR s := {literal}; END_PROGRAM");
    let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert_eq!("P0012", result.unwrap_err().code);
}
