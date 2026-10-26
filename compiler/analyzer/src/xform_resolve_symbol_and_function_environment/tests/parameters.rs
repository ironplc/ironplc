//! The type of each parameter of a function's signature.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{FileId, Id};
use ironplc_parser::parse_program;

use crate::{
    function_environment::FunctionEnvironment,
    semantic_context::SemanticContext,
    semantic_type::{ParameterType, SemanticFunctionParameter},
    symbol_environment::SymbolEnvironment,
    test_helpers::{edition3_options, parse_and_resolve_types_with_options},
    xform_resolve_symbol_and_function_environment::apply_impl,
};

fn resolve(program: &str) -> SemanticContext {
    parse_and_resolve_types_with_options(program, &edition3_options()).1
}

/// The parameters of the function `name` in `context`.
fn parameters<'a>(context: &'a SemanticContext, name: &str) -> &'a [SemanticFunctionParameter] {
    &context.functions().get(&Id::from(name)).unwrap().parameters
}

#[test]
fn apply_when_parameter_names_type_then_type_id_of_name() {
    let context = resolve(
        "
TYPE Point : STRUCT x : INT; END_STRUCT; END_TYPE
FUNCTION f : INT
VAR_INPUT a : INT; END_VAR
VAR_IN_OUT p : Point; END_VAR
f := a;
END_FUNCTION",
    );
    let types = context.types();
    let parameters = parameters(&context, "f");

    assert_eq!(
        parameters[0].param_type,
        ParameterType::Type(types.id_of(&TypeName::from("INT")).unwrap())
    );
    assert_eq!(
        parameters[1].param_type,
        ParameterType::Type(types.id_of(&TypeName::from("Point")).unwrap())
    );
}

#[test]
fn apply_when_ref_to_parameter_then_type_id_of_reference() {
    let context = resolve(
        "
FUNCTION f : INT
VAR_INPUT r : REF_TO INT; END_VAR
f := r^;
END_FUNCTION",
    );
    let types = context.types();
    let parameter = &parameters(&context, "f")[0];

    assert!(parameter.is_reference);
    assert!(matches!(parameter.param_type, ParameterType::Type(_)));
    let ParameterType::Type(reference) = parameter.param_type else {
        return;
    };
    assert_eq!(
        types.referenced_type(reference),
        types.id_of(&TypeName::from("INT"))
    );
}

/// Without the type ids `xform_resolve_decl_types` records, no declared
/// type resolves; each parameter is still a parameter of the signature.
#[test]
fn apply_when_parameter_type_unresolved_then_name_as_written() {
    let library = parse_program(
        "
FUNCTION f : INT
VAR_INPUT a : Nope; r : REF_TO Nope; END_VAR
f := 0;
END_FUNCTION",
        &FileId::default(),
        &edition3_options(),
    )
    .unwrap();
    let mut function_env = FunctionEnvironment::new();
    apply_impl(&library, &mut SymbolEnvironment::new(), &mut function_env);
    let parameters = &function_env.get(&Id::from("f")).unwrap().parameters;

    assert_eq!(
        parameters
            .iter()
            .map(|parameter| parameter.param_type.clone())
            .collect::<Vec<_>>(),
        [
            ParameterType::Unresolved(TypeName::from("Nope")),
            ParameterType::Unresolved(TypeName::from("REF_TO Nope")),
        ]
    );
}
