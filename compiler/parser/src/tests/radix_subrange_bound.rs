//! Subrange bounds written in hex, binary or octal (`16#01..16#0F`).
//!
//! The bound parses to the same value a decimal bound would; the gate that
//! keeps it out of the strict dialect is `rule_token_radix_subrange_bound`.

use super::common::*;

fn parse_text_twincat(source: &str) -> Library {
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    result.unwrap()
}

/// The `(start, end)` values of a subrange whose bounds are both literals.
fn bounds(subrange: &dsl::common::Subrange) -> (u128, u128) {
    let start = subrange.start.as_signed_integer().unwrap();
    let end = subrange.end.as_signed_integer().unwrap();
    assert!(!start.is_neg && !end.is_neg);
    (start.value.value, end.value.value)
}

#[rstest]
#[case::hex("16#01..16#0F", (1, 15))]
#[case::binary("2#1..2#1111", (1, 15))]
#[case::octal("8#1..8#17", (1, 15))]
#[case::decimal_then_hex("1..16#0F", (1, 15))]
#[case::hex_then_decimal("16#01..15", (1, 15))]
#[case::spaced("16#01 .. 16#0F", (1, 15))]
fn parse_when_case_label_subrange_has_radix_bounds_then_subrange_values(
    #[case] label: &str,
    #[case] expected: (u128, u128),
) {
    let source = format!(
        "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    {label}: y := 1;
END_CASE;
END_FUNCTION_BLOCK"
    );
    let library = parse_text_twincat(&source);
    let case = extract_case(&library);
    let subrange = cast!(
        &case.statement_groups[0].selectors[0],
        CaseSelectionKind::Subrange
    );
    assert_eq!(bounds(subrange), expected);
}

#[test]
fn parse_when_case_label_radix_bound_then_span_covers_literal() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    1..16#0F: y := 1;
END_CASE;
END_FUNCTION_BLOCK";
    let library = parse_text_twincat(source);
    let case = extract_case(&library);
    let subrange = cast!(
        &case.statement_groups[0].selectors[0],
        CaseSelectionKind::Subrange
    );
    let end = subrange.end.as_signed_integer().unwrap();

    let start = source.find("16#0F").unwrap();
    assert_eq!(end.value.span.start, start);
    assert_eq!(end.value.span.end, start + "16#0F".len());
}

#[test]
fn parse_when_subrange_type_has_radix_bounds_then_subrange_values() {
    let library = parse_text_twincat("TYPE R : INT (16#00..16#FF); END_TYPE");
    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Subrange);
    let spec = cast!(&decl.spec, SpecificationKind::Inline);
    assert_eq!(bounds(&spec.subrange), (0, 255));
}

#[test]
fn parse_when_array_dimension_has_radix_bounds_then_subrange_values() {
    let library = parse_text_twincat(
        "PROGRAM main
VAR
    a : ARRAY[16#0..16#F, 8#1..8#10] OF INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(bounds(&subranges.ranges[0]), (0, 15));
    assert_eq!(bounds(&subranges.ranges[1]), (1, 8));
}

/// A radix literal is unsigned in the grammar, so a sign before one is not
/// a bound, as it is not a CASE label.
#[test]
fn parse_when_radix_bound_has_sign_then_err() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    -16#10..0: y := 1;
END_CASE;
END_FUNCTION_BLOCK";
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    assert!(parse_program(source, &FileId::default(), &options).is_err());
}

const CASE_SOURCE: &str = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#01..16#0F: y := 1;
END_CASE;
END_FUNCTION_BLOCK";

const ARRAY_SOURCE: &str = "
PROGRAM main
VAR
    a : ARRAY[0..16#F] OF INT;
END_VAR
END_PROGRAM";

const TYPE_SOURCE: &str = "TYPE R : INT (2#0..2#1111); END_TYPE";

/// The problem codes `parse_program` reports for `source` under `options`.
fn problem_codes(source: &str, options: &CompilerOptions) -> Vec<String> {
    match parse_program(source, &FileId::default(), options) {
        Ok(_) => vec![],
        Err(diagnostic) => vec![diagnostic.code],
    }
}

/// In a CASE label, a radix bound is gated by the flag that gates a radix
/// label, and reported with the same problem.
#[test]
fn parse_when_case_label_radix_bound_and_flag_off_then_p4041() {
    let options = CompilerOptions {
        allow_radix_subrange_bounds: true,
        ..CompilerOptions::default()
    };
    assert_eq!(problem_codes(CASE_SOURCE, &options), vec!["P4041"]);
}

#[test]
fn parse_when_case_label_radix_bound_and_flag_on_then_ok() {
    let options = CompilerOptions {
        allow_bit_string_case_labels: true,
        ..CompilerOptions::default()
    };
    assert!(problem_codes(CASE_SOURCE, &options).is_empty());
}

#[rstest]
#[case::array(ARRAY_SOURCE)]
#[case::subrange_type(TYPE_SOURCE)]
fn parse_when_declaration_radix_bound_and_flag_off_then_p4073(#[case] source: &str) {
    let options = CompilerOptions {
        allow_bit_string_case_labels: true,
        ..CompilerOptions::default()
    };
    assert_eq!(problem_codes(source, &options), vec!["P4073"]);
}

#[rstest]
#[case::array(ARRAY_SOURCE)]
#[case::subrange_type(TYPE_SOURCE)]
fn parse_when_declaration_radix_bound_and_flag_on_then_ok(#[case] source: &str) {
    let options = CompilerOptions {
        allow_radix_subrange_bounds: true,
        ..CompilerOptions::default()
    };
    assert!(problem_codes(source, &options).is_empty());
}
