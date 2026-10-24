//! Tests of an initializer that is exactly one constant's name: a `VAR`
//! initializer (`x : UDINT := C;`), a string initializer
//! (`s : STRING[20] := CS;`) and a `TYPE` default (`TYPE MyInt : UDINT :=
//! C;`). The smallest constant expression, and the one the declared-type
//! check applies to.
//!
//! These run the type-resolution pipeline rather than `apply` alone: the
//! check reads the type environment (an alias on either side compares as the
//! type it names), and a user-typed variable only reaches the fold through
//! the type resolver (ADR-0050). The diagnostics tests run every rule too,
//! so that a second report of the same mistake would show up.
use super::tests::find_var_decl;
use super::*;
use crate::stages::analyze;
use crate::test_helpers::{codes, parse_and_resolve_types_with_options};
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_test::cast;
use rstest::rstest;

fn opts() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Rusty)
}

/// A program with `constant` declared in `VAR_GLOBAL CONSTANT` and
/// `variable` declared in a function block, with the aliases `MyInt` and
/// `MyUdint` available to both.
fn program(constant: &str, variable: &str) -> String {
    format!(
        "
TYPE
    MyInt : INT;
    MyUdint : UDINT;
END_TYPE
VAR_GLOBAL CONSTANT
    {constant}
END_VAR
FUNCTION_BLOCK fb
VAR
    {variable}
END_VAR
END_FUNCTION_BLOCK"
    )
}

/// The problem codes `analyze` reports for `source`, in order.
fn analyzed_codes(source: &str, options: &CompilerOptions) -> Vec<String> {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (_, context) = analyze(&[&library], options).unwrap();
    context
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect()
}

/// The magnitude of the integer literal `name` starts at after the fold.
fn integer_value(library: &Library, name: &str) -> u128 {
    let simple = cast!(
        &find_var_decl(library, name).initializer,
        InitialValueAssignmentKind::Simple
    );
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::IntegerLiteral
    );
    lit.value.value.value
}

#[test]
fn apply_when_initializer_names_global_constant_then_variable_starts_at_its_value() {
    let source = program("C : UDINT := 7;", "x : UDINT := C;");
    let (library, context) = parse_and_resolve_types_with_options(&source, &opts());

    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    assert_eq!(7, integer_value(&library, "x"));
}

#[test]
fn apply_when_initializer_names_local_constant_then_variable_starts_at_its_value() {
    let source = "
FUNCTION_BLOCK fb
VAR CONSTANT
    LIMIT : INT := 100;
END_VAR
VAR
    threshold : INT := LIMIT;
END_VAR
END_FUNCTION_BLOCK";
    let (library, context) = parse_and_resolve_types_with_options(source, &opts());

    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    assert_eq!(100, integer_value(&library, "threshold"));
}

/// The substituted literal is positioned at the name that was replaced, not
/// at the declaration of the constant, so a later diagnostic on the value
/// points at the initializer.
#[test]
fn apply_when_initializer_names_constant_then_literal_span_is_the_reference() {
    let source = program("C : UDINT := 7;", "x : UDINT := C;");
    let (library, _) = parse_and_resolve_types_with_options(&source, &opts());

    let simple = cast!(
        &find_var_decl(&library, "x").initializer,
        InitialValueAssignmentKind::Simple
    );
    let span = simple.initial_value.as_ref().unwrap().span();
    let reference = source.find("C;").unwrap();
    assert_eq!(reference, span.start);
    assert_eq!(reference + 1, span.end);
}

#[test]
fn apply_when_initializer_names_constant_and_flag_disabled_then_p4037_alone() {
    let options = CompilerOptions {
        allow_constant_initializer_expressions: false,
        ..opts()
    };
    let source = program("C : UDINT := 7;", "x : UDINT := C;");

    assert_eq!(
        codes(&[Problem::ConstantInitializerExpressionNotAllowed]),
        analyzed_codes(&source, &options)
    );
}

#[test]
fn apply_when_initializer_names_variable_that_is_not_constant_then_p4038() {
    let source = "
FUNCTION_BLOCK fb
VAR
    v : UDINT := 7;
    x : UDINT := v;
END_VAR
END_FUNCTION_BLOCK";

    assert_eq!(
        codes(&[Problem::InitializerNotConstantExpression]),
        analyzed_codes(source, &opts())
    );
}

/// A constant whose declared type an assignment would reject is rejected
/// here too, once, with the value left out so that no later check reports
/// the same declaration a second time.
#[rstest]
#[case::narrowing("C : UDINT := 1;", "x : INT := C;")]
#[case::signed_to_unsigned("C : INT := 1;", "x : UDINT := C;")]
#[case::real_to_integer("C : REAL := 1.5;", "x : INT := C;")]
#[case::lreal_to_real("C : LREAL := 1.5;", "x : REAL := C;")]
#[case::bool_to_integer("C : BOOL := TRUE;", "x : INT := C;")]
#[case::integer_to_bool("C : INT := 1;", "x : BOOL := C;")]
#[case::alias_of_variable_type("C : UDINT := 1;", "x : MyInt := C;")]
#[case::alias_of_constant_type("C : MyUdint := 1;", "x : INT := C;")]
fn apply_when_constant_type_incompatible_with_variable_type_then_p4022_alone(
    #[case] constant: &str,
    #[case] variable: &str,
) {
    let source = program(constant, variable);

    assert_eq!(
        codes(&[Problem::InitializerTypeMismatch]),
        analyzed_codes(&source, &opts())
    );
}

/// The relation is the one an assignment uses: the same type, an alias of
/// it on either side, and the implicit widenings the options allow.
#[rstest]
#[case::same_type("C : UDINT := 1;", "x : UDINT := C;")]
#[case::integer_widening("C : INT := 1;", "x : DINT := C;")]
#[case::real_widening("C : REAL := 1.5;", "x : LREAL := C;")]
#[case::temporal_widening("C : TIME := T#1s;", "x : LTIME := C;")]
#[case::alias_of_variable_type("C : INT := 1;", "x : MyInt := C;")]
#[case::alias_of_constant_type("C : MyInt := 1;", "x : INT := C;")]
#[case::alias_on_both_sides("C : MyUdint := 1;", "x : MyUdint := C;")]
#[case::cross_family_conversion("C : DWORD := 16#1;", "x : UDINT := C;")]
fn apply_when_constant_type_compatible_with_variable_type_then_ok(
    #[case] constant: &str,
    #[case] variable: &str,
) {
    let source = program(constant, variable);

    assert!(analyzed_codes(&source, &opts()).is_empty());
}

/// Only a bare name has a declared type to check. An expression over the
/// same constant has none, and is accepted on its folded value, as before.
#[test]
fn apply_when_initializer_is_expression_over_constant_then_checked_by_value_only() {
    let source = program("C : UDINT := 1;", "x : INT := C + 0;");

    assert!(analyzed_codes(&source, &opts()).is_empty());
}

/// The default of the simple type declaration named `name`.
fn type_default(library: &Library, name: &str) -> Option<ConstantKind> {
    library.elements.iter().find_map(|element| match element {
        LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Simple(simple))
            if simple.type_name == TypeName::from(name) =>
        {
            let init = cast!(&simple.spec_and_init, InitialValueAssignmentKind::Simple);
            init.initial_value.clone()
        }
        _ => None,
    })
}

#[test]
fn apply_when_type_default_names_constant_then_default_is_its_value() {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
TYPE
    MyInt : UDINT := C;
END_TYPE";
    let (library, context) = parse_and_resolve_types_with_options(source, &opts());

    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    let default = type_default(&library, "MyInt").unwrap();
    let lit = cast!(&default, ConstantKind::IntegerLiteral);
    assert_eq!(7, lit.value.value.value);
}

#[test]
fn apply_when_type_default_names_constant_and_flag_disabled_then_p4037_alone() {
    let options = CompilerOptions {
        allow_constant_initializer_expressions: false,
        ..opts()
    };
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 7;
END_VAR
TYPE
    MyInt : UDINT := C;
END_TYPE";

    assert_eq!(
        codes(&[Problem::ConstantInitializerExpressionNotAllowed]),
        analyzed_codes(source, &options)
    );
}

/// The folded string keeps the declaration's length and takes the
/// constant's literal.
#[test]
fn apply_when_string_initializer_names_constant_then_literal_and_length_kept() {
    let source = program("CS : STRING := 'abc';", "s : STRING[20] := CS;");
    let (library, context) = parse_and_resolve_types_with_options(&source, &opts());

    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    let string = cast!(
        &find_var_decl(&library, "s").initializer,
        InitialValueAssignmentKind::String
    );
    assert_eq!(
        Some(20),
        string
            .length
            .as_ref()
            .and_then(|length| length.as_integer())
            .map(|length| length.value)
    );
    let literal = string.initial_value.as_ref().unwrap();
    assert_eq!(vec!['a', 'b', 'c'], literal.value);
}

#[test]
fn apply_when_string_initializer_names_constant_and_flag_disabled_then_p4037_alone() {
    let options = CompilerOptions {
        allow_constant_initializer_expressions: false,
        ..opts()
    };
    let source = program("CS : STRING := 'abc';", "s : STRING := CS;");

    assert_eq!(
        codes(&[Problem::ConstantInitializerExpressionNotAllowed]),
        analyzed_codes(&source, &options)
    );
}

#[test]
fn apply_when_string_initializer_names_variable_that_is_not_constant_then_p4038() {
    let source = "
FUNCTION_BLOCK fb
VAR
    v : STRING := 'abc';
    s : STRING := v;
END_VAR
END_FUNCTION_BLOCK";

    assert_eq!(
        codes(&[Problem::InitializerNotConstantExpression]),
        analyzed_codes(source, &opts())
    );
}

/// A string and a value of another type, or of the other string encoding,
/// are as incompatible here as in an assignment; a longer expression that
/// folds to anything but a string is reported the same way.
#[rstest]
#[case::wstring_into_string("WS : WSTRING := \"abc\";", "s : STRING := WS;")]
#[case::string_into_wstring("CS : STRING := 'abc';", "w : WSTRING := CS;")]
#[case::integer_into_string("C : UDINT := 1;", "s : STRING := C;")]
#[case::string_into_integer("CS : STRING := 'abc';", "x : UDINT := CS;")]
#[case::integer_expression_into_string("C : UDINT := 1;", "s : STRING := C + 1;")]
fn apply_when_string_constant_type_incompatible_then_p4022_alone(
    #[case] constant: &str,
    #[case] variable: &str,
) {
    let source = program(constant, variable);

    assert_eq!(
        codes(&[Problem::InitializerTypeMismatch]),
        analyzed_codes(&source, &opts())
    );
}

/// The length is not part of the relation: a constant of either string
/// type initializes a variable of that type at any length.
#[rstest]
#[case::unsized_into_sized("CS : STRING := 'abc';", "s : STRING[20] := CS;")]
#[case::unsized_into_unsized("CS : STRING := 'abc';", "s : STRING := CS;")]
#[case::wstring("WS : WSTRING := \"abc\";", "w : WSTRING[8] := WS;")]
fn apply_when_string_constant_type_compatible_then_ok(
    #[case] constant: &str,
    #[case] variable: &str,
) {
    let source = program(constant, variable);

    assert!(analyzed_codes(&source, &opts()).is_empty());
}
