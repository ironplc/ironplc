
use crate::type_environment::TypeEnvironment;

use super::apply;
use ironplc_dsl::{
    common::*,
    core::{FileId, Id, SourceSpan},
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

#[test]
fn apply_when_has_function_block_type_then_resolves_type() {
    let program = "
FUNCTION_BLOCK called

END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR
    fb_var : called;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let result = apply(input, &mut type_environment).unwrap().0;

    let expected = Library {
        elements: vec![
            LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                name: TypeName::from("called"),
                variables: vec![],
                edge_variables: vec![],
                body: FunctionBlockBodyKind::empty(),
                span: SourceSpan::default(),
                oop: None,
                methods: vec![],
                properties: vec![],
            }),
            LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                name: TypeName::from("caller"),
                variables: vec![VarDecl::function_block("fb_var", "called")],
                edge_variables: vec![],
                body: FunctionBlockBodyKind::empty(),
                span: SourceSpan::default(),
                oop: None,
                methods: vec![],
                properties: vec![],
            }),
        ],
    };

    assert_eq!(result, expected)
}

#[test]
fn apply_when_has_struct_type_then_resolves_type() {
    let program = "
TYPE
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT;  
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : the_struct;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let result = apply(input, &mut type_environment).unwrap().0;

    let expected = Library {
        elements: vec![
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
                StructureDeclaration {
                    type_name: TypeName::from("the_struct"),
                    elements: vec![StructureElementDeclaration {
                        name: Id::from("member"),
                        init: InitialValueAssignmentKind::simple_uninitialized(TypeName::from(
                            "BOOL",
                        )),
                    }],
                },
            )),
            LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                name: TypeName::from("caller"),
                variables: vec![VarDecl::structure("the_var", "the_struct")],
                edge_variables: vec![],
                body: FunctionBlockBodyKind::empty(),
                span: SourceSpan::default(),
                oop: None,
                methods: vec![],
                properties: vec![],
            }),
        ],
    };

    assert_eq!(result, expected)
}

#[test]
fn apply_when_has_enum_type_then_resolves_type() {
    let program = "
TYPE
    values : (val1, val2, val3);  
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : values;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let result = apply(input, &mut type_environment).unwrap().0;

    let expected = Library {
        elements: vec![
            LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                EnumerationDeclaration {
                    type_name: TypeName::from("values"),
                    spec_init: EnumeratedSpecificationInit {
                        spec: EnumeratedSpecificationKind::from_values(vec![
                            "val1", "val2", "val3",
                        ]),
                        default: None,
                        underlying_type: None,
                    },
                },
            )),
            LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                name: TypeName::from("caller"),
                variables: vec![VarDecl::uninitialized_enumerated("the_var", "values")],
                edge_variables: vec![],
                body: FunctionBlockBodyKind::empty(),
                span: SourceSpan::default(),
                oop: None,
                methods: vec![],
                properties: vec![],
            }),
        ],
    };

    assert_eq!(result, expected)
}

#[test]
fn apply_when_has_subrange_type_then_resolves_type() {
    let program = "
TYPE
    my_range : INT (1..100);
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : my_range;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let result = apply(input, &mut type_environment).unwrap().0;

    // Find the caller function block and check the variable initializer
    let caller_fb = result.elements.iter().find(|e| {
            matches!(e, LibraryElementKind::FunctionBlockDeclaration(fb) if fb.name == TypeName::from("caller"))
        });
    assert!(caller_fb.is_some());

    if let LibraryElementKind::FunctionBlockDeclaration(fb) = caller_fb.unwrap() {
        assert_eq!(fb.variables.len(), 1);
        assert!(matches!(
            &fb.variables[0].initializer,
            InitialValueAssignmentKind::Subrange(SpecificationKind::Named(tn))
            if *tn == TypeName::from("my_range")
        ));
    }
}

#[test]
fn apply_when_has_array_type_then_array_keeps_type_name() {
    let program = "
TYPE
    my_array : ARRAY[1..2] OF INT;
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : my_array;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let result = apply(input, &mut type_environment).unwrap().0;

    let caller_fb = result.elements.iter().find_map(|e| match e {
        LibraryElementKind::FunctionBlockDeclaration(fb) if fb.name == TypeName::from("caller") => {
            Some(fb)
        }
        _ => None,
    });
    assert!(matches!(
        &caller_fb.unwrap().variables[0].initializer,
        InitialValueAssignmentKind::Array(ArrayInitialValueAssignment {
            spec: SpecificationKind::Named(tn),
            ..
        }) if *tn == TypeName::from("my_array")
    ));
}

/// A repeated type name is the symbol environment's to report; this
/// pass keeps the first declaration and resolves against it.
#[test]
fn apply_when_duplicated_type_then_first_kept_without_error() {
    let program = "
TYPE
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT;  
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT; 
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : the_struct;
    END_VAR

END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (_library, diagnostics) = apply(input, &mut type_environment).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn apply_when_unrelated_pou_has_undeclared_type_then_other_pou_still_resolves() {
    // FB_A has a genuinely broken reference to an undeclared type.
    // FB_B is entirely unrelated and valid. Resolving FB_A's error
    // must not discard the successful resolution of FB_B's variable.
    let program = "
FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR
    inst : FB_Callee;
END_VAR
END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (result, diagnostics) = apply(input, &mut type_environment).unwrap();

    assert_eq!(1, diagnostics.len());
    assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);

    let fb_b = result
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb)
                if fb.name == TypeName::from("FB_B") =>
            {
                Some(fb)
            }
            _ => None,
        })
        .unwrap();

    assert!(matches!(
        &fb_b.variables[0].initializer,
        InitialValueAssignmentKind::FunctionBlock(fb_init)
        if fb_init.type_name == TypeName::from("FB_Callee")
    ));
}

#[test]
fn apply_when_same_pou_has_undeclared_type_then_other_variable_still_resolves() {
    // Both the broken and the valid variable declaration live in the
    // same POU, matching the shape found in a real corpus.
    let program = "
FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;
    inst : FB_Callee;
END_VAR
END_FUNCTION_BLOCK
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (result, diagnostics) = apply(input, &mut type_environment).unwrap();

    assert_eq!(1, diagnostics.len());
    assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);

    let fb_a = result
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb)
                if fb.name == TypeName::from("FB_A") =>
            {
                Some(fb)
            }
            _ => None,
        })
        .unwrap();

    assert!(matches!(
        &fb_a.variables[1].initializer,
        InitialValueAssignmentKind::FunctionBlock(fb_init)
        if fb_init.type_name == TypeName::from("FB_Callee")
    ));
}

#[rstest::rstest]
#[case::declared_block("FB_Counter", "(limit := 20)")]
#[case::stdlib_block("TON", "(PT := T#1s)")]
fn apply_when_member_initializer_on_function_block_then_function_block_initializer(
    #[case] type_name: &str,
    #[case] initializer: &str,
) {
    let program = format!(
        "
FUNCTION_BLOCK FB_Counter
VAR
    limit : INT := 10;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : {type_name} := {initializer};
END_VAR
END_PROGRAM"
    );
    let library = crate::test_helpers::parse_and_resolve_types(&program);

    let decl = library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::ProgramDeclaration(program) => program.variables.first(),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        &decl.initializer,
        InitialValueAssignmentKind::FunctionBlock(init)
            if init.type_name == TypeName::from(type_name) && init.init.len() == 1
    ));
}

// -----------------------------------------------------------------
// A user-typed initializer is classified by the type (ADR-0050).
// -----------------------------------------------------------------

/// The first variable of the first program organization unit after the
/// pass, with the diagnostics it raised.
fn resolve_first_var(program: &str) -> (VarDecl, Vec<ironplc_dsl::diagnostic::Diagnostic>) {
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
            .unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (result, diagnostics) = apply(input, &mut type_environment).unwrap();
    let decl = result
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::ProgramDeclaration(program) => program.variables.first(),
            _ => None,
        })
        .unwrap()
        .clone();
    (decl, diagnostics)
}

#[test]
fn apply_when_members_on_structure_type_then_structure_initializer() {
    let (decl, diagnostics) = resolve_first_var(
        "
TYPE
    Point : STRUCT a : INT; b : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    p : Point := (a := 1);
END_VAR
END_PROGRAM",
    );
    assert!(diagnostics.is_empty());
    assert!(matches!(
        &decl.initializer,
        InitialValueAssignmentKind::Structure(init)
            if init.type_name == TypeName::from("Point") && init.elements_init.len() == 1
    ));
}

#[test]
fn apply_when_value_on_enumeration_type_then_enumerated_initializer() {
    let (decl, diagnostics) = resolve_first_var(
        "
TYPE
    Color : (Red, Green);
END_TYPE
PROGRAM main
VAR
    c : Color := Green;
END_VAR
END_PROGRAM",
    );
    assert!(diagnostics.is_empty());
    assert!(matches!(
        &decl.initializer,
        InitialValueAssignmentKind::EnumeratedType(init)
            if init.type_name == TypeName::from("Color")
                && init.initial_value.as_ref().map(|v| v.value.clone()) == Some(Id::from("Green"))
    ));
}

#[test]
fn apply_when_value_on_alias_type_then_constant_expression_initializer() {
    // A bare identifier on a non-enumeration type is a named constant,
    // for the initializer-expression fold to evaluate.
    let (decl, diagnostics) = resolve_first_var(
        "
TYPE
    Count : INT := 0;
END_TYPE
PROGRAM main
VAR
    n : Count := LIMIT;
END_VAR
END_PROGRAM",
    );
    assert!(diagnostics.is_empty());
    assert!(matches!(
        &decl.initializer,
        InitialValueAssignmentKind::SimpleExpr(init)
            if init.type_name == TypeName::from("Count")
                && matches!(&init.initial_value.kind, ironplc_dsl::textual::ExprKind::Variable(_))
    ));
}

#[test]
fn apply_when_members_on_undeclared_type_then_diagnosed_and_placeholder_kept() {
    let (decl, diagnostics) = resolve_first_var(
        "
PROGRAM main
VAR
    p : Missing := (a := 1);
END_VAR
END_PROGRAM",
    );
    assert_eq!(1, diagnostics.len());
    assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);
    assert!(matches!(
        &decl.initializer,
        InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
            initial_value: Some(LateResolvedInitialValue::Members(_)),
            ..
        })
    ));
}

fn interface_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

fn only_prototype_variable(library: &Library) -> &VarDecl {
    let itf = library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::InterfaceDeclaration(itf) => Some(itf),
            _ => None,
        })
        .unwrap();
    &itf.methods[0].variables[0]
}

#[test]
fn apply_when_method_prototype_parameter_has_enum_type_then_resolves_type() {
    let program = "
TYPE
    E_Mode : (idle, busy);
END_TYPE

INTERFACE I_Axis
METHOD Move : BOOL
VAR_INPUT
    mode : E_Mode;
END_VAR
END_METHOD
END_INTERFACE
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &interface_options()).unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (result, diagnostics) = apply(input, &mut type_environment).unwrap();

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(matches!(
        only_prototype_variable(&result).initializer,
        InitialValueAssignmentKind::EnumeratedType(_)
    ));
}

#[test]
fn apply_when_method_prototype_parameter_has_undeclared_type_then_p2008() {
    let program = "
INTERFACE I_Axis
METHOD Move : BOOL
VAR_INPUT
    mode : E_Missing;
END_VAR
END_METHOD
END_INTERFACE
        ";
    let input =
        ironplc_parser::parse_program(program, &FileId::default(), &interface_options()).unwrap();
    let mut type_environment = TypeEnvironment::new();
    let (_, diagnostics) = apply(input, &mut type_environment).unwrap();

    assert_eq!(1, diagnostics.len());
    assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);
}
