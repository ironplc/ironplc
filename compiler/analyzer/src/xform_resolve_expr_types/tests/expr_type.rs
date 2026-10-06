//! Tests for `Expr::expr_type`, the type of an expression's value by
//! identity.

use crate::semantic_context::SemanticContext;
use crate::semantic_type::SemanticType;
use crate::test_helpers::parse_and_resolve_types_with_options;
use ironplc_dsl::common::{GenericTypeName, Library, TypeName};
use ironplc_dsl::textual::{Assignment, ExprType};
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::{CompilerOptions, Dialect};
use std::convert::Infallible;

/// Resolves `body` inside a program declaring one variable of each kind the
/// tests read, under edition 3 (which has `REF`).
fn resolve(body: &str) -> (Library, SemanticContext) {
    let program = format!(
        "
TYPE
  MY_BYTE : BYTE := 0;
END_TYPE
PROGRAM main
VAR
  n : DINT;
  b : MY_BYTE;
  a : ARRAY[1..2] OF DINT;
  e : (RED, GREEN);
  r : REF_TO DINT;
END_VAR
  {body}
END_PROGRAM
"
    );
    parse_and_resolve_types_with_options(
        &program,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
}

/// The `expr_type` of the value of the first assignment in `library`.
fn first_assigned_type(library: &Library) -> Option<ExprType> {
    struct First(Option<Option<ExprType>>);
    impl Visitor<Infallible> for First {
        type Value = ();
        fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
            if self.0.is_none() {
                self.0 = Some(node.value.expr_type.clone());
            }
            Ok(())
        }
    }
    let mut first = First(None);
    let _ = first.walk(library);
    first.0.flatten()
}

fn concrete(expr_type: Option<ExprType>) -> ironplc_dsl::type_id::TypeId {
    match expr_type {
        Some(ExprType::Concrete(id)) => id,
        other => panic!("expected a concrete type, got {other:?}"),
    }
}

#[test]
fn apply_when_elementary_variable_then_concrete_elementary() {
    let (library, context) = resolve("n := n;");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("DINT"))
    );
}

#[test]
fn apply_when_alias_variable_then_alias_type() {
    let (library, context) = resolve("n := b;");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("MY_BYTE"))
    );
}

#[test]
fn apply_when_whole_anonymous_array_then_anonymous_array_type() {
    let (library, context) = resolve("n := a;");

    let id = concrete(first_assigned_type(&library));

    assert_eq!(context.types().name_of(id), None);
    assert!(matches!(
        context.types().get_by_id(id).unwrap().representation,
        SemanticType::Array { .. }
    ));
}

#[test]
fn apply_when_parenthesized_then_inner_type() {
    let (plain, _) = resolve("n := a;");
    let (grouped, _) = resolve("n := (a);");

    assert_eq!(first_assigned_type(&plain), first_assigned_type(&grouped));
}

#[test]
fn apply_when_inline_enumeration_then_anonymous_enumeration() {
    let (library, context) = resolve("n := e;");

    let id = concrete(first_assigned_type(&library));

    assert!(context
        .types()
        .get_by_id(id)
        .unwrap()
        .representation
        .is_enumeration());
}

#[test]
fn apply_when_arithmetic_then_concrete_operand_type() {
    let (library, context) = resolve("n := n + 1;");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("DINT"))
    );
}

#[test]
fn apply_when_untyped_integer_literal_then_literal_any_int() {
    let (library, _) = resolve("n := 5;");

    assert_eq!(
        first_assigned_type(&library),
        Some(ExprType::Literal(GenericTypeName::AnyInt))
    );
}

#[test]
fn apply_when_ref_then_same_reference_type_as_declared_reference() {
    let (library, context) = resolve("r := REF(n);");

    let id = concrete(first_assigned_type(&library));
    let types = context.types();

    let dint = types.id_of(&TypeName::from("DINT")).unwrap();
    assert_eq!(types.referenced_type(id), Some(dint));
    assert!(matches!(
        types.get_by_id(id).unwrap().representation,
        SemanticType::Reference { .. }
    ));
}

#[test]
fn apply_when_ref_and_reference_variable_then_one_type() {
    let (by_ref, _) = resolve("r := REF(n);");
    let (by_variable, _) = resolve("r := r;");

    assert_eq!(
        first_assigned_type(&by_ref),
        first_assigned_type(&by_variable)
    );
}

#[test]
fn apply_when_null_then_null() {
    let (library, _) = resolve("r := NULL;");

    assert_eq!(first_assigned_type(&library), Some(ExprType::Null));
}

#[test]
fn apply_when_deref_then_referenced_type() {
    let (library, context) = resolve("n := r^;");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("DINT"))
    );
}

/// Resolves `body` inside a program over a structure, an array of it and a
/// reference to that array.
fn resolve_points(body: &str) -> (Library, SemanticContext) {
    let program = format!(
        "
TYPE
  POINT : STRUCT x : INT; y : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
  n : INT;
  p : POINT;
  pts : ARRAY[1..2] OF POINT;
  rp : REF_TO ARRAY[1..2] OF POINT;
END_VAR
  {body}
END_PROGRAM
"
    );
    parse_and_resolve_types_with_options(
        &program,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
}

#[test]
fn apply_when_array_element_then_element_type() {
    let (library, context) = resolve_points("p := pts[1];");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("POINT"))
    );
}

#[test]
fn apply_when_dereferenced_array_element_then_element_type() {
    let (library, context) = resolve_points("p := rp^[1];");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("POINT"))
    );
}

#[test]
fn apply_when_field_of_array_element_then_field_type() {
    let (library, context) = resolve_points("n := pts[1].x;");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("INT"))
    );
}

/// Resolves `unit` after an enumeration `E_State` that has a value `Error`.
fn resolve_with_enumeration(unit: &str) -> (Library, SemanticContext) {
    let program = format!(
        "
TYPE E_State : (Idle, Error); END_TYPE
{unit}
"
    );
    parse_and_resolve_types_with_options(
        &program,
        &CompilerOptions {
            allow_fb_inheritance: true,
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        },
    )
}

/// A variable in scope hides an enumerated value of the same name. Late-bound
/// resolution sees only the variables of the enclosing unit, so these are
/// the names it takes for the enumerated value until the symbol environment
/// restores them.
#[rstest::rstest]
#[case::method_input(
    "FUNCTION_BLOCK FB_A VAR b : BOOL; END_VAR METHOD M VAR_INPUT Error : BOOL; END_VAR b := Error; END_METHOD END_FUNCTION_BLOCK"
)]
#[case::method_local(
    "FUNCTION_BLOCK FB_A VAR b : BOOL; END_VAR METHOD M VAR Error : BOOL; END_VAR b := Error; END_METHOD END_FUNCTION_BLOCK"
)]
#[case::global(
    "VAR_GLOBAL Error : BOOL; END_VAR PROGRAM main VAR b : BOOL; END_VAR b := Error; END_PROGRAM"
)]
#[case::inherited_field(
    "FUNCTION_BLOCK FB_Base VAR Error : BOOL; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK FB_A EXTENDS FB_Base VAR b : BOOL; END_VAR METHOD M b := Error; END_METHOD END_FUNCTION_BLOCK"
)]
#[case::property_set_input(
    "FUNCTION_BLOCK FB_A VAR b : BOOL; END_VAR PROPERTY Error : BOOL SET b := Error; END_SET END_PROPERTY END_FUNCTION_BLOCK"
)]
fn apply_when_variable_named_like_enumerated_value_in_scope_then_variable_type(#[case] unit: &str) {
    let (library, context) = resolve_with_enumeration(unit);

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("BOOL"))
    );
}

#[test]
fn apply_when_no_variable_named_like_enumerated_value_then_enumeration_type() {
    let (library, context) =
        resolve_with_enumeration("PROGRAM main VAR s : E_State; END_VAR s := Error; END_PROGRAM");

    assert_eq!(
        Some(concrete(first_assigned_type(&library))),
        context.types().id_of(&TypeName::from("E_State"))
    );
}
