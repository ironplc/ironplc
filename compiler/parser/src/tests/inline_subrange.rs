//! A variable declared with a subrange spelled in place (`x : INT(0..15)`),
//! with and without an initial value, in each block whose grammar allows it.

use super::common::*;
use dsl::common::{ElementaryTypeName, SubrangeInitialValueAssignment};

/// The base type, bounds and initial value of an inline subrange
/// initializer, with each value written as the source spells it.
fn inline_subrange(
    init: &InitialValueAssignmentKind,
) -> (ElementaryTypeName, String, String, Option<String>) {
    let subrange = cast!(init, InitialValueAssignmentKind::Subrange);
    let SubrangeInitialValueAssignment {
        spec,
        initial_value,
    } = subrange;
    let spec = cast!(spec, SpecificationKind::Inline);
    let bound =
        |bound: &dsl::common::SignedIntegerRef| bound.as_signed_integer().unwrap().to_string();
    (
        spec.type_name.clone(),
        bound(&spec.subrange.start),
        bound(&spec.subrange.end),
        initial_value.as_ref().map(|value| value.to_string()),
    )
}

/// The variables of the function block in `source`.
fn block_variables(source: &str) -> Vec<VarDecl> {
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let block = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    block.variables.clone()
}

#[rstest]
#[case::var("VAR", VariableType::Var)]
#[case::input("VAR_INPUT", VariableType::Input)]
#[case::output("VAR_OUTPUT", VariableType::Output)]
#[case::temp("VAR_TEMP", VariableType::VarTemp)]
#[case::retain("VAR RETAIN", VariableType::Var)]
fn parse_when_inline_subrange_without_initial_value_then_subrange(
    #[case] block: &str,
    #[case] var_type: VariableType,
) {
    let variables = block_variables(&format!(
        "FUNCTION_BLOCK FB {block} x : INT(0..15); END_VAR END_FUNCTION_BLOCK"
    ));

    assert_eq!(variables[0].var_type, var_type);
    assert_eq!(
        inline_subrange(&variables[0].initializer),
        (ElementaryTypeName::INT, "0".into(), "15".into(), None)
    );
}

#[rstest]
#[case::var("VAR")]
#[case::input("VAR_INPUT")]
#[case::output("VAR_OUTPUT")]
#[case::temp("VAR_TEMP")]
#[case::constant("VAR CONSTANT")]
fn parse_when_inline_subrange_with_initial_value_then_subrange_keeps_value(#[case] block: &str) {
    let variables = block_variables(&format!(
        "FUNCTION_BLOCK FB {block} x : DINT(-10..10) := -3; END_VAR END_FUNCTION_BLOCK"
    ));

    assert_eq!(
        inline_subrange(&variables[0].initializer),
        (
            ElementaryTypeName::DINT,
            "-10".into(),
            "10".into(),
            Some("-3".into())
        )
    );
}

#[test]
fn parse_when_inline_subrange_for_several_names_then_each_has_subrange() {
    let variables = block_variables(
        "FUNCTION_BLOCK FB VAR x, y : UINT(2..15) := 4; END_VAR END_FUNCTION_BLOCK",
    );

    assert_eq!(variables.len(), 2);
    for variable in &variables {
        assert_eq!(
            inline_subrange(&variable.initializer),
            (
                ElementaryTypeName::UINT,
                "2".into(),
                "15".into(),
                Some("4".into())
            )
        );
    }
}

#[test]
fn parse_when_located_inline_subrange_then_subrange() {
    let variables = block_variables(
        "FUNCTION_BLOCK FB VAR x AT %MW0 : INT(5..15) := 9; END_VAR END_FUNCTION_BLOCK",
    );

    assert!(matches!(
        variables[0].identifier,
        VariableIdentifier::Direct(_)
    ));
    assert_eq!(
        inline_subrange(&variables[0].initializer),
        (
            ElementaryTypeName::INT,
            "5".into(),
            "15".into(),
            Some("9".into())
        )
    );
}

#[test]
fn parse_when_external_inline_subrange_then_subrange_without_value() {
    let variables = block_variables(
        "FUNCTION_BLOCK FB VAR_EXTERNAL g : INT(5..15); END_VAR END_FUNCTION_BLOCK",
    );

    assert_eq!(variables[0].var_type, VariableType::External);
    assert_eq!(
        inline_subrange(&variables[0].initializer),
        (ElementaryTypeName::INT, "5".into(), "15".into(), None)
    );
}

#[test]
fn parse_when_external_inline_subrange_with_initial_value_then_error() {
    let result = parse_program(
        "FUNCTION_BLOCK FB VAR_EXTERNAL g : INT(5..15) := 9; END_VAR END_FUNCTION_BLOCK",
        &FileId::default(),
        &CompilerOptions::default(),
    );

    assert!(result.is_err());
}

#[test]
fn parse_when_global_inline_subrange_then_subrange() {
    let library = parse_program(
        "CONFIGURATION config
VAR_GLOBAL
    g : INT(5..15);
    h : LINT(-5000000000..5000000000) := 7;
END_VAR
RESOURCE res ON PLC PROGRAM p : main; END_RESOURCE
END_CONFIGURATION",
        &FileId::default(),
        &CompilerOptions::default(),
    )
    .unwrap();
    let configuration = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );

    assert_eq!(
        inline_subrange(&configuration.global_var[0].initializer),
        (ElementaryTypeName::INT, "5".into(), "15".into(), None)
    );
    assert_eq!(
        inline_subrange(&configuration.global_var[1].initializer),
        (
            ElementaryTypeName::LINT,
            "-5000000000".into(),
            "5000000000".into(),
            Some("7".into())
        )
    );
}

#[test]
fn parse_when_var_config_inline_subrange_then_subrange() {
    let library = parse_program(
        "CONFIGURATION config
RESOURCE res ON PLC PROGRAM p : main; END_RESOURCE
VAR_CONFIG
    res.p.x AT %MW0 : INT(0..15) := 3;
END_VAR
END_CONFIGURATION",
        &FileId::default(),
        &CompilerOptions::default(),
    )
    .unwrap();
    let configuration = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );

    assert_eq!(
        inline_subrange(&configuration.located_var_inits[0].initializer),
        (
            ElementaryTypeName::INT,
            "0".into(),
            "15".into(),
            Some("3".into())
        )
    );
}
