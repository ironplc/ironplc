//! Type names, generic/ANY return types and STRING-length forms.

use super::common::*;

#[test]
fn parse_when_enumerated_value_is_reserved_keyword_then_parses() {
    // enumerated_value() used to require a bare identifier(), which
    // rejects ON/STEP/R_EDGE/F_EDGE (reserved tokens) even though
    // variable_identifier() already carves out exactly this set for
    // VAR declarations (see #300, "Feature/reserved variables") --
    // real-world enums commonly use these as ordinary member names
    // (e.g. `(off, on)` for a blink state).
    let source = "TYPE E_Test : (on, off, step, r_edge, f_edge, normal_value); END_TYPE";
    let library = parse_text(source);
    let decl = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let enumeration = cast!(decl, DataTypeDeclarationKind::Enumeration);
    let inline = cast!(&enumeration.spec_init.spec, SpecificationKind::Inline);
    let values: Vec<&String> = inline.values.iter().map(|v| v.value.original()).collect();
    assert_eq!(
        values,
        vec!["on", "off", "step", "r_edge", "f_edge", "normal_value"]
    );
}

#[test]
fn parse_when_function_with_any_num_return_type_then_parses() {
    let lib = parse_text(
        "FUNCTION ABS : ANY_NUM
            VAR_INPUT
                IN : ANY_NUM;
            END_VAR
            ABS := IN;
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    assert_eq!(
        func.return_type,
        FunctionReturnType::Named(TypeName::from("ANY_NUM"))
    );
}

#[test]
fn parse_when_variable_with_any_int_type_then_parses() {
    let lib = parse_text(
        "FUNCTION TEST : INT
            VAR_INPUT
                val : ANY_INT;
            END_VAR
            TEST := 0;
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let type_ref = func.variables[0].type_name();
    assert_eq!(type_ref, TypeReference::Named(TypeName::from("ANY_INT")));
}

#[test]
fn parse_when_all_generic_type_names_then_parses() {
    // Test all generic type names can be used as type references
    let generic_types = [
        "ANY",
        "ANY_DERIVED",
        "ANY_ELEMENTARY",
        "ANY_MAGNITUDE",
        "ANY_NUM",
        "ANY_REAL",
        "ANY_INT",
        "ANY_BIT",
        "ANY_STRING",
        "ANY_DATE",
    ];

    for generic_type in generic_types {
        let source = format!(
            "FUNCTION TEST : {}
                VAR_INPUT
                    val : {};
                END_VAR
                TEST := val;
                END_FUNCTION",
            generic_type, generic_type
        );

        let result = parse_program(&source, &FileId::default(), &CompilerOptions::default());
        assert!(
            result.is_ok(),
            "Failed to parse generic type: {}",
            generic_type
        );

        let lib = result.unwrap();
        let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
        assert_eq!(
            func.return_type,
            FunctionReturnType::Named(TypeName::from(generic_type)),
            "Return type mismatch for {}",
            generic_type
        );
    }
}

#[test]
fn parse_when_function_with_string_length_return_type_then_parses() {
    let lib = parse_text(
        "FUNCTION my_func : STRING[255]
            VAR_INPUT
                x : INT;
            END_VAR
            my_func := 'hello';
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let spec = cast!(&func.return_type, FunctionReturnType::String);
    assert_eq!(spec.width, dsl::common::StringType::String);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_function_with_wstring_length_return_type_then_parses() {
    let lib = parse_text(
        "FUNCTION my_func : WSTRING[100]
            VAR_INPUT
                x : INT;
            END_VAR
            my_func := 'hello';
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let spec = cast!(&func.return_type, FunctionReturnType::WString);
    assert_eq!(spec.width, dsl::common::StringType::WString);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_function_with_string_paren_length_return_type_then_parses() {
    // CODESYS/TwinCAT accept STRING(n) with parentheses as an alternate
    // delimiter to the standard STRING[n] brackets. This is a vendor
    // extension, not standard IEC 61131-3, so it requires the
    // `allow_paren_string_length` flag; the strict default dialect rejects
    // it (see parse_when_var_with_string_paren_length_and_strict_dialect_
    // then_rejected).
    let lib = parse_text_paren_string_length(
        "FUNCTION my_func : STRING(255)
            VAR_INPUT
                x : INT;
            END_VAR
            my_func := 'hello';
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let spec = cast!(&func.return_type, FunctionReturnType::String);
    assert_eq!(spec.width, dsl::common::StringType::String);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_function_with_wstring_paren_length_return_type_then_parses() {
    let lib = parse_text_paren_string_length(
        "FUNCTION my_func : WSTRING(100)
            VAR_INPUT
                x : INT;
            END_VAR
            my_func := 'hello';
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let spec = cast!(&func.return_type, FunctionReturnType::WString);
    assert_eq!(spec.width, dsl::common::StringType::WString);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_var_with_string_paren_length_then_parses() {
    let lib = parse_text_paren_string_length(
        "PROGRAM main
VAR
    hostName : STRING(255);
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let spec = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::String
    );
    assert_eq!(spec.width, dsl::common::StringType::String);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_var_with_wstring_paren_length_then_parses() {
    let lib = parse_text_paren_string_length(
        "PROGRAM main
VAR
    wideName : WSTRING(100);
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let spec = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::String
    );
    assert_eq!(spec.width, dsl::common::StringType::WString);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_var_with_string_bracket_length_then_parses() {
    // Regression: the standard bracket form must still parse unchanged.
    let lib = parse_text(
        "PROGRAM main
VAR
    hostName : STRING[255];
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let spec = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::String
    );
    assert_eq!(spec.width, dsl::common::StringType::String);
    assert!(spec.length.is_some());
}

#[test]
fn parse_when_var_with_string_paren_length_and_strict_dialect_then_rejected() {
    // The STRING(n) parenthesis delimiter is a vendor extension, not
    // standard IEC 61131-3. Under the strict default dialect (no
    // allow_paren_string_length flag) it must be rejected (P4042).
    let result = parse_program(
        "PROGRAM main
VAR
    hostName : STRING(255);
END_VAR
END_PROGRAM",
        &FileId::default(),
        &CompilerOptions::default(),
    );
    assert!(result.is_err());
}

#[test]
fn parse_when_var_with_string_mixed_bracket_paren_delimiters_then_rejected() {
    // A mismatched delimiter pair (`[` ... `)`) is not a valid length spec
    // under either form. Uses the flag ON so the failure is attributable to
    // the delimiter mismatch, not the vendor-extension gate.
    let options = CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(
        "PROGRAM main
VAR
    hostName : STRING[20);
END_VAR
END_PROGRAM",
        &FileId::default(),
        &options,
    );
    assert!(result.is_err());
}

#[test]
fn parse_when_var_with_string_mixed_paren_bracket_delimiters_then_rejected() {
    // The other mismatched pair (`(` ... `]`). Flag ON, same reasoning.
    let options = CompilerOptions {
        allow_paren_string_length: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(
        "PROGRAM main
VAR
    hostName : STRING(20];
END_VAR
END_PROGRAM",
        &FileId::default(),
        &options,
    );
    assert!(result.is_err());
}

#[test]
fn parse_when_function_with_bare_string_return_type_then_parses() {
    let lib = parse_text(
        "FUNCTION my_func : STRING
            VAR_INPUT
                x : INT;
            END_VAR
            my_func := 'hello';
            END_FUNCTION",
    );

    assert_eq!(lib.elements.len(), 1);
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let spec = cast!(&func.return_type, FunctionReturnType::String);
    assert_eq!(spec.width, dsl::common::StringType::String);
    assert!(spec.length.is_none());
}

#[test]
fn parse_when_function_var_with_string_length_then_parses() {
    let _lib = parse_text(
        "FUNCTION MY_FUNC : INT
            VAR_INPUT
                x : INT;
            END_VAR
            VAR
                buf : STRING[10];
            END_VAR
            MY_FUNC := 0;
            END_FUNCTION",
    );
}

#[test]
fn parse_when_function_var_constant_with_string_length_then_parses() {
    let _lib = parse_text(
        "FUNCTION MY_FUNC : INT
            VAR CONSTANT
                FILL : STRING[1] := '0';
            END_VAR
            MY_FUNC := 0;
            END_FUNCTION",
    );
}

#[test]
fn parse_when_function_var_in_out_with_string_length_then_parses() {
    let _lib = parse_text(
        "FUNCTION MY_FUNC : INT
            VAR_IN_OUT
                buf : STRING[255];
            END_VAR
            MY_FUNC := 0;
            END_FUNCTION",
    );
}

#[test]
fn parse_when_struct_member_with_string_length_then_parses() {
    let _lib = parse_text(
        "TYPE MY_STRUCT :
            STRUCT
                name : STRING[10];
            END_STRUCT;
            END_TYPE
            FUNCTION MY_FUNC : INT
            VAR_INPUT
                x : INT;
            END_VAR
            MY_FUNC := 0;
            END_FUNCTION",
    );
}
