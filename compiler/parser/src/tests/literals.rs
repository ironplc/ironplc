//! Numeric, real, duration and long-date/time literal parsing.

use super::common::*;

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
