//! Numeric, real, duration and long-date/time literal parsing.

use super::common::*;
use dsl::core::Located;

#[test]
fn parse_program_when_complex_bit_string_then_ok() {
    let program = "
FUNCTION fun:DWORD

VAR_IN_OUT
    VAR1: INT;
END_VAR

VAR1 := DWORD#16#0000FFFF;

END_FUNCTION";

    parse_text(program);
}

#[test]
fn parse_program_when_real_then_ok() {
    let program = "
FUNCTION fun:DWORD

VAR
    InputsNumber : REAL := -5.0E-1;
END_VAR

fun := InputsNumber;

END_FUNCTION";
    let res = parse_text(program);

    let expected = new_library(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name: Id::from("fun"),
            return_type: FunctionReturnType::Named(TypeName::from("DWORD")),
            variables: vec![VarDecl {
                identifier: VariableIdentifier::new_symbol("InputsNumber"),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("REAL"),
                    initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                        value: -0.5,
                        data_type: None,
                        span: SourceSpan::default(),
                    })),
                }),
                block: next_block_id(),
            }],
            edge_variables: vec![],
            body: vec![StmtKind::simple_assignment("fun", "InputsNumber")],
        },
    ));
    assert_eq!(res, expected);
}

#[test]
fn parse_program_when_real_scientific_no_decimal_then_ok() {
    let program = "
FUNCTION fun:DWORD

VAR
    InputsNumber : REAL := 2E-3;
END_VAR

fun := InputsNumber;

END_FUNCTION";
    let res = parse_text(program);

    let expected = new_library(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name: Id::from("fun"),
            return_type: FunctionReturnType::Named(TypeName::from("DWORD")),
            variables: vec![VarDecl {
                identifier: VariableIdentifier::new_symbol("InputsNumber"),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("REAL"),
                    initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                        value: 0.002,
                        data_type: None,
                        span: SourceSpan::default(),
                    })),
                }),
                block: next_block_id(),
            }],
            edge_variables: vec![],
            body: vec![StmtKind::simple_assignment("fun", "InputsNumber")],
        },
    ));
    assert_eq!(res, expected);
}

#[test]
fn parse_program_when_real_scientific_positive_exponent_then_ok() {
    let program = "
FUNCTION fun:DWORD

VAR
    InputsNumber : REAL := 1.5E+2;
END_VAR

fun := InputsNumber;

END_FUNCTION";
    let res = parse_text(program);

    let expected = new_library(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name: Id::from("fun"),
            return_type: FunctionReturnType::Named(TypeName::from("DWORD")),
            variables: vec![VarDecl {
                identifier: VariableIdentifier::new_symbol("InputsNumber"),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("REAL"),
                    initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                        value: 150.0,
                        data_type: None,
                        span: SourceSpan::default(),
                    })),
                }),
                block: next_block_id(),
            }],
            edge_variables: vec![],
            body: vec![StmtKind::simple_assignment("fun", "InputsNumber")],
        },
    ));
    assert_eq!(res, expected);
}

#[test]
fn parse_program_when_fixed_point_duration_then_ok() {
    let program = "
FUNCTION fun:TIME

VAR
    tv : TIME := t#1.2s;
END_VAR

fun := tv;

END_FUNCTION";
    let actual = parse_text(program);

    let expected = new_library(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name: Id::from("fun"),
            return_type: FunctionReturnType::Named(TypeName::from("TIME")),
            variables: vec![VarDecl {
                identifier: VariableIdentifier::new_symbol("tv"),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("TIME"),
                    initial_value: Some(ConstantKind::Duration(DurationLiteral {
                        interval: Duration::milliseconds(1200),
                        span: SourceSpan::default(),
                    })),
                }),
                block: next_block_id(),
            }],
            edge_variables: vec![],
            body: vec![StmtKind::simple_assignment("fun", "tv")],
        },
    ));
    assert_eq!(actual, expected);
}

#[test]
fn parse_when_ldate_literal_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    d : LDATE;
END_VAR
    d := LDATE#2024-01-20;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
}

#[test]
fn parse_when_ltod_literal_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    t : LTOD;
END_VAR
    t := LTOD#14:30:20;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
}

#[test]
fn parse_when_ldt_literal_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    my_dt : LDT;
END_VAR
    my_dt := LDT#2024-01-20-15:30:22;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
}

#[test]
fn parse_when_ltime_of_day_long_form_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    t : LTIME_OF_DAY;
END_VAR
    t := LTOD#10:00:00;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
}

#[test]
fn parse_when_ldate_and_time_long_form_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    my_dt : LDATE_AND_TIME;
END_VAR
    my_dt := LDT#2024-01-20-15:30:22;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
}

/// Parses a function block whose single statement assigns `literal` to a
/// variable, and returns the character string literal that was parsed.
fn parse_assigned_character_string(source: &'static str) -> CharacterStringLiteral {
    let library = parse_text(source);
    let value = extract_assignment_value(&library);
    let constant = cast!(&value.kind, ExprKind::Const);
    cast!(constant, ConstantKind::CharacterString).clone()
}

#[test]
fn parse_program_when_single_quoted_literal_then_narrow_width() {
    let literal = parse_assigned_character_string(
        "
FUNCTION_BLOCK fb
VAR
    s : STRING[10];
END_VAR
s := 'abc';
END_FUNCTION_BLOCK",
    );

    assert_eq!(literal.value, vec!['a', 'b', 'c']);
    assert_eq!(literal.width, StringType::String);
}

#[test]
fn parse_program_when_double_quoted_literal_then_wide_width() {
    let literal = parse_assigned_character_string(
        "
FUNCTION_BLOCK fb
VAR
    w : WSTRING[10];
END_VAR
w := \"abc\";
END_FUNCTION_BLOCK",
    );

    assert_eq!(literal.value, vec!['a', 'b', 'c']);
    assert_eq!(literal.width, StringType::WString);
}

#[test]
fn parse_program_when_typed_string_prefix_then_width_from_delimiter() {
    let narrow = parse_assigned_character_string(
        "
FUNCTION_BLOCK fb
VAR
    s : STRING[10];
END_VAR
s := STRING#'abc';
END_FUNCTION_BLOCK",
    );
    assert_eq!(narrow.width, StringType::String);

    let wide = parse_assigned_character_string(
        "
FUNCTION_BLOCK fb
VAR
    w : WSTRING[10];
END_VAR
w := WSTRING#\"abc\";
END_FUNCTION_BLOCK",
    );
    assert_eq!(wide.width, StringType::WString);
}

/// Every literal kind records where it was written.
///
/// `Located for ExprKind` builds a compound expression's span by joining its
/// operands' spans, so one span-less literal kind makes every expression
/// containing it report byte offset 0 — which is what a diagnostic then
/// points at. Covering all nine kinds is what keeps that from regressing one
/// literal at a time.
///
/// A leading `-` is not part of any case here: outside a declaration
/// initializer it parses as a unary operator applied to the literal, and
/// `Located for ExprKind` reports a unary expression as its operand's span —
/// a separate truncation, unrelated to whether the literal has a span at all.
#[rstest]
#[case::integer("42")]
#[case::typed_integer("INT#42")]
#[case::hex_integer("16#2A")]
#[case::real("1.5")]
#[case::typed_real("REAL#1.5")]
#[case::boolean("TRUE")]
#[case::typed_boolean("BOOL#TRUE")]
#[case::character_string("'text'")]
#[case::duration("T#100ms")]
#[case::time_of_day("TOD#14:30:00")]
#[case::date("DATE#2025-03-15")]
#[case::date_and_time("DT#2025-03-15-14:30:00")]
#[case::bit_string("WORD#16#FF")]
fn parse_when_literal_then_constant_span_covers_the_literal(#[case] literal: &str) {
    let source = format!(
        "FUNCTION_BLOCK fb
VAR
    x : INT;
END_VAR
x := {literal};
END_FUNCTION_BLOCK"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap();

    let value = extract_assignment_value(&library);
    let constant = cast!(&value.kind, ExprKind::Const);
    let span = constant.span();
    assert_eq!(
        literal,
        &source[span.start..span.end],
        "span of {constant:?} should cover the literal as written"
    );
}
