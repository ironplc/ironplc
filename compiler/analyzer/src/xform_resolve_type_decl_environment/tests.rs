//! Unit tests for `xform_resolve_type_decl_environment`.

use crate::semantic_type::{ByteSized, SemanticType};
use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};

use super::apply;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::{common::*, core::FileId};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

#[test]
fn apply_when_ambiguous_enum_then_resolves_type() {
    let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
    ";
    let (result, _env) = parse_and_apply_with_elementary_types(program);
    let (result, _diagnostics) = result.unwrap();

    let expected = Library {
        elements: vec![
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                EnumerationDeclaration {
                    type_name: TypeName::from("LEVEL"),
                    spec_init: EnumeratedSpecificationInit::values_and_default(
                        vec!["CRITICAL"],
                        "CRITICAL",
                    ),
                },
            )),
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                EnumerationDeclaration {
                    type_name: TypeName::from("LEVEL_ALIAS"),
                    spec_init: EnumeratedSpecificationInit {
                        spec: SpecificationKind::Named(TypeName::from("LEVEL")),
                        default: None,
                        underlying_type: None,
                    },
                },
            )),
        ],
    };

    assert_eq!(result, expected)
}

/// A repeated type name is reported, and the pass still completes with
/// the first declaration resolved: the repeat does not revert the library.
#[test]
fn apply_when_has_duplicate_items_then_p2007_and_first_kept() {
    let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE
    ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut env = TypeEnvironment::new();
    let (library, diagnostics) = apply(input, &mut env).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Problem::TypeDeclNameDuplicated.code());
    assert_eq!(library.elements.len(), 2);
    assert!(env.get(&TypeName::from("LEVEL")).is_some());
}

/// The first declaration of a repeated alias is labeled at its own
/// name, not at the declaration of the type it aliases.
#[test]
fn apply_when_alias_repeated_then_p2007_labels_first_alias() {
    let program = "
TYPE
Base : STRUCT x : INT; END_STRUCT;
END_TYPE
TYPE
Again : Base;
END_TYPE
TYPE
Again : BOOL;
END_TYPE
    ";
    let (result, _env) = parse_and_apply_with_elementary_types(program);
    let (_, diagnostics) = result.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Problem::TypeDeclNameDuplicated.code());
    let first = program.find("Again").unwrap();
    let second = program.rfind("Again").unwrap();
    assert_eq!(diagnostics[0].primary.location.start, second);
    assert_eq!(diagnostics[0].secondary[0].location.start, first);
}

#[test]
fn apply_when_declares_stdlib_type_then_error() {
    let program = "
TYPE
LREAL : REAL;
END_TYPE
    ";
    let result =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap_err();
    // This doesn't actually fail due to this transform but something should
    // catch this.
    assert_eq!("P0002", result.code);
}

#[test]
fn apply_when_array_element_is_string_type_then_ok() {
    let program = "
TYPE
  STRING10 : STRING[10];
END_TYPE

TYPE
  ELEMENT_WEEKDAYS	: ARRAY [1..7] OF STRING10;
END_TYPE
    ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();

    let mut env = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap();
    let _library = apply(input, &mut env).unwrap();
}

#[test]
fn apply_when_simple_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_INT : INT := 0;
MY_BOOL : BOOL := FALSE;
END_TYPE
    ";
    let (_result, env) = parse_and_apply_with_elementary_types(program);

    let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
    assert!(matches!(
        &my_int_type.representation,
        SemanticType::Int {
            size: ByteSized::B16
        }
    ));

    let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
    assert!(matches!(&my_bool_type.representation, SemanticType::Bool));
}

#[test]
fn apply_when_simple_type_alias_missing_base_then_error() {
    let program = "
TYPE
MY_TYPE : UNKNOWN_TYPE := 0;
END_TYPE

    ";
    let (result, _env) = parse_and_apply_with_elementary_types(program);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(
        Problem::ParentTypeNotDeclared.code(),
        error.first().unwrap().code
    );
}

#[test]
fn apply_when_int_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_INT : INT := 42;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
    assert!(matches!(
        &my_int_type.representation,
        SemanticType::Int {
            size: ByteSized::B16
        }
    ));
}

#[test]
fn apply_when_invalid_base_type_then_error() {
    let program = "
TYPE
MY_TYPE : UNKNOWN_TYPE := 0;
END_TYPE
    ";
    let (result, _env) = parse_and_apply_with_empty_env(program);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(
        Problem::ParentTypeNotDeclared.code(),
        error.first().unwrap().code
    );
}

#[test]
fn apply_when_real_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_REAL : REAL := 3.14;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_real_type = env.get(&TypeName::from("MY_REAL")).unwrap();
    assert!(matches!(
        &my_real_type.representation,
        SemanticType::Real {
            size: ByteSized::B32
        }
    ));
}

#[test]
fn apply_when_bool_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_BOOL : BOOL := TRUE;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
    assert!(matches!(&my_bool_type.representation, SemanticType::Bool));
}

#[test]
fn apply_when_dint_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_DINT : DINT := 1000;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_dint_type = env.get(&TypeName::from("MY_DINT")).unwrap();
    assert!(matches!(
        &my_dint_type.representation,
        SemanticType::Int {
            size: ByteSized::B32
        }
    ));
}

#[test]
fn apply_when_time_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_TIME : TIME := T#5s;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_time_type = env.get(&TypeName::from("MY_TIME")).unwrap();
    assert!(matches!(
        &my_time_type.representation,
        SemanticType::Time { .. }
    ));
}

#[test]
fn apply_when_multiple_type_aliases_then_creates_all_aliases() {
    let program = "
TYPE
MY_INT : INT := 42;
MY_BOOL : BOOL := FALSE;
MY_REAL : REAL := 2.71;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify all aliases were created
    let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
    assert!(matches!(
        &my_int_type.representation,
        SemanticType::Int {
            size: ByteSized::B16
        }
    ));

    let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
    assert!(matches!(&my_bool_type.representation, SemanticType::Bool));

    let my_real_type = env.get(&TypeName::from("MY_REAL")).unwrap();
    assert!(matches!(
        &my_real_type.representation,
        SemanticType::Real {
            size: ByteSized::B32
        }
    ));
}

#[test]
fn apply_when_byte_type_alias_then_creates_alias() {
    let program = "
TYPE
MY_BYTE : BYTE := 16#FF;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the alias was created
    let my_byte_type = env.get(&TypeName::from("MY_BYTE")).unwrap();
    assert!(matches!(
        &my_byte_type.representation,
        SemanticType::Bytes {
            size: ByteSized::B8
        }
    ));
}

/// What `apply` returns: the resolved library with the repeats it met, or
/// the failure that reverted it.
type Applied = Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>;

/// Helper function to parse 61131-3 code and apply type resolution with elementary types
fn parse_and_apply_with_elementary_types(program: &str) -> (Applied, TypeEnvironment) {
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut env = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap();
    let result = apply(input, &mut env);
    (result, env)
}

/// Helper function to parse 61131-3 code and apply type resolution with empty environment
fn parse_and_apply_with_empty_env(program: &str) -> (Applied, TypeEnvironment) {
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut env = TypeEnvironment::new();
    let result = apply(input, &mut env);
    (result, env)
}

// Array type integration tests

#[test]
fn apply_when_array_declaration_then_integrates_with_type_environment() {
    let program = "
TYPE
MY_ARRAY : ARRAY [1..10] OF INT;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify the array type was added to the type environment
    let array_type = env.get(&TypeName::from("MY_ARRAY")).unwrap();
    assert!(matches!(
        &array_type.representation,
        SemanticType::Array { .. }
    ));
}

#[test]
fn apply_when_array_type_alias_then_creates_alias() {
    let program = "
TYPE
BASE_ARRAY : ARRAY [1..10] OF INT;
ALIAS_ARRAY : BASE_ARRAY;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Both should resolve to the same array type
    let base_type = env.get(&TypeName::from("BASE_ARRAY")).unwrap();
    let alias_type = env.get(&TypeName::from("ALIAS_ARRAY")).unwrap();
    assert_eq!(base_type.representation, alias_type.representation);
}

#[test]
fn apply_when_array_of_user_defined_type_then_resolves_correctly() {
    let program = "
TYPE
POINT : STRUCT
X : INT;
Y : INT;
END_STRUCT;
MY_ARRAY : ARRAY [1..3] OF POINT;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify array of structure type works
    let array_type = env.get(&TypeName::from("MY_ARRAY")).unwrap();
    assert!(matches!(
        &array_type.representation,
        SemanticType::Array {
            element_type,
            ..
        } if matches!(**element_type, SemanticType::Structure { .. })
    ));
}

#[test]
fn apply_when_nested_array_types_then_resolves_correctly() {
    let program = "
TYPE
INNER_ARRAY : ARRAY [1..5] OF INT;
OUTER_ARRAY : ARRAY [1..3] OF INNER_ARRAY;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Verify nested arrays work
    let outer_type = env.get(&TypeName::from("OUTER_ARRAY")).unwrap();
    assert!(matches!(
        &outer_type.representation,
        SemanticType::Array {
            element_type,
            ..
        } if matches!(**element_type, SemanticType::Array { .. })
    ));
}

// Subrange type integration tests

#[test]
fn apply_when_subrange_type_alias_then_creates_alias() {
    let program = "
TYPE
BASE_RANGE : INT (0..100);
ALIAS_RANGE : BASE_RANGE;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Both should resolve to the same subrange type
    let base_type = env.get(&TypeName::from("BASE_RANGE")).unwrap();
    let alias_type = env.get(&TypeName::from("ALIAS_RANGE")).unwrap();
    assert!(matches!(
        &base_type.representation,
        SemanticType::Subrange { .. }
    ));
    assert_eq!(base_type.representation, alias_type.representation);
}

#[test]
fn apply_when_nested_subrange_type_alias_then_creates_alias() {
    let program = "
TYPE
BASE_RANGE : INT (0..100);
MIDDLE_RANGE : BASE_RANGE;
TOP_RANGE : MIDDLE_RANGE;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // All three should resolve to the same subrange type
    let base_type = env.get(&TypeName::from("BASE_RANGE")).unwrap();
    let middle_type = env.get(&TypeName::from("MIDDLE_RANGE")).unwrap();
    let top_type = env.get(&TypeName::from("TOP_RANGE")).unwrap();
    assert!(matches!(
        &base_type.representation,
        SemanticType::Subrange { .. }
    ));
    assert_eq!(base_type.representation, middle_type.representation);
    assert_eq!(base_type.representation, top_type.representation);
}

#[test]
fn apply_when_simple_decl_without_type_spec_then_error() {
    // This test verifies that declarations without a type specification
    // return a proper error. This would happen if the parser produces
    // InitialValueAssignmentKind::None for a simple declaration.
    let input = Library {
        elements: vec![LibraryElementKind::DataTypeDeclaration(
            DataTypeDeclarationKind::Simple(SimpleDeclaration {
                type_name: TypeName::from("MY_TYPE"),
                spec_and_init: InitialValueAssignmentKind::None(SourceSpan::default()),
            }),
        )],
    };

    let mut env = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap();

    let result = apply(input, &mut env);

    // Should return an error
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "P2019"); // InvalidSimpleTypeDecl
}

// Structure type alias tests

#[test]
fn apply_when_structure_type_alias_then_creates_alias() {
    let program = "
TYPE
Point : STRUCT
    x : INT := 0;
    y : INT := 0;
END_STRUCT;
PointAlias : Point;
END_TYPE
    ";
    let (result, env) = parse_and_apply_with_elementary_types(program);
    assert!(result.is_ok());

    // Both should resolve to the same structure type
    let base_type = env.get(&TypeName::from("Point")).unwrap();
    let alias_type = env.get(&TypeName::from("PointAlias")).unwrap();
    assert!(base_type.representation.is_structure());
    assert_eq!(base_type.representation, alias_type.representation);
}

#[test]
fn apply_when_structure_type_alias_missing_base_then_error() {
    let program = "
TYPE
MyAlias : MissingStruct;
END_TYPE
    ";
    let (result, _env) = parse_and_apply_with_elementary_types(program);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(
        Problem::ParentTypeNotDeclared.code(),
        error.first().unwrap().code
    );
}

// ---------------------------------------------------------------------
// OOP extension: INTERFACE registers as a known type.
// See specs/design/beckhoff-twincat-dialect.md §1.3.
// ---------------------------------------------------------------------

#[test]
fn apply_when_interface_declared_then_registers_as_interface_type() {
    let program = "
INTERFACE I_Drivable
END_INTERFACE
    ";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let input = ironplc_parser::parse_program(program, &FileId::default(), &options).unwrap();
    let mut env = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap();
    let result = apply(input, &mut env);
    assert!(result.is_ok(), "{:?}", result.err());

    let interface_type = env.get(&TypeName::from("I_Drivable")).unwrap();
    assert!(interface_type.representation.is_interface());
}

#[test]
fn apply_when_var_declared_with_interface_type_then_resolves() {
    // Interface must be declared before use (declarations are sorted
    // upstream by xform_toposort_declarations, but this module alone
    // only resolves types in file order, so declare first here).
    let program = "
INTERFACE I_Drivable
END_INTERFACE

FUNCTION_BLOCK FB_Example
VAR
pDrv : I_Drivable;
END_VAR
END_FUNCTION_BLOCK
    ";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let input = ironplc_parser::parse_program(program, &FileId::default(), &options).unwrap();
    let mut env = TypeEnvironmentBuilder::new()
        .with_elementary_types()
        .build()
        .unwrap();
    let result = apply(input, &mut env);
    assert!(result.is_ok(), "{:?}", result.err());
}
