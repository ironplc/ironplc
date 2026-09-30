//! Unit tests for `xform_toposort_declarations`: the ordering of
//! declarations and the set reachable from the programs.
use super::*;

use crate::test_helpers::parse_only;
use ironplc_parser::{options::CompilerOptions, parse_program};
use ironplc_test::cast;

/// A repeated name keeps both declarations: the environments built from
/// the sorted library diagnose the repeat, so the sort must not hide it.
#[test]
fn apply_when_function_name_repeated_then_both_declarations_kept() {
    let program = "
FUNCTION F : INT
  F := 1;
END_FUNCTION

FUNCTION F : INT
  F := 2;
END_FUNCTION";
    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (sorted, _) = apply(library).unwrap();
    let functions = sorted
        .elements
        .iter()
        .filter(
            |e| matches!(e, LibraryElementKind::FunctionDeclaration(f) if f.name == Id::from("F")),
        )
        .count();
    assert_eq!(functions, 2);
}

#[test]
fn apply_when_function_block_recursive_call_in_self_then_return_error() {
    let program = "
        FUNCTION_BLOCK SelfRecursive
            VAR
               SelfRecursiveInstance : SelfRecursive;
            END_VAR

        END_FUNCTION_BLOCK";

    let library = parse_only(program);
    let result = apply(library);
    assert_eq!(
        result.unwrap_err().first().unwrap().code,
        Problem::RecursiveCycle.code().to_string()
    );
}

#[test]
fn apply_when_function_block_not_recursive_call_in_self_then_return_ok() {
    let program = "
        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR

        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR

        END_FUNCTION_BLOCK";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Callee"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Caller"));
}

// ---------------------------------------------------------------------
// FUNCTION_BLOCK EXTENDS dependency edge.
// ---------------------------------------------------------------------

fn parse_with_fb_inheritance(program: &str) -> Library {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    parse_program(program, &FileId::default(), &options).unwrap()
}

#[test]
fn apply_when_function_block_extends_cycle_then_return_error() {
    let program = "
FUNCTION_BLOCK FB_A EXTENDS FB_B
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
END_FUNCTION_BLOCK";

    let library = parse_with_fb_inheritance(program);
    let result = apply(library);
    assert_eq!(
        result.unwrap_err().first().unwrap().code,
        Problem::RecursiveCycle.code().to_string()
    );
}

#[test]
fn apply_when_function_block_extends_forward_reference_then_base_ordered_first() {
    // The derived FB is declared textually *before* its base -- the
    // new dependency edge must still order the base first.
    let program = "
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK";

    let library = parse_with_fb_inheritance(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("FB_Base"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("FB_Derived"));
}

#[test]
fn apply_when_function_block_no_extends_then_return_ok() {
    let program = "
FUNCTION_BLOCK FB_Plain
END_FUNCTION_BLOCK";

    let library = parse_with_fb_inheritance(program);
    let result = apply(library);
    assert!(result.is_ok());
}

#[test]
fn apply_when_eager_function_block_initializer_forward_reference_then_referenced_type_ordered_first(
) {
    // Regression for a dependency-graph edge-direction bug: the
    // FunctionBlock arms (both this dedicated visitor and the inline
    // arm in visit_initial_value_assignment_kind) previously added the
    // edge in the opposite direction to the Structure/LateResolvedType
    // arms, ordering a referenced type *after* its referencing POU and
    // producing a spurious P2011 "Parent type is not declared"
    // downstream.
    //
    // A bare `CalleeInstance : Callee;` declaration parses to
    // LateResolvedType (the correct arm, already covered above), so it
    // does not exercise this. An *eager* InitialValueAssignmentKind::
    // FunctionBlock initializer is what the CODESYS/TwinCAT call-style
    // instance initializer (`name : FB_Type(args);`) constructs at
    // parse time -- but that grammar lands in a separate PR. To keep
    // this regression independent of it, construct the eager
    // FunctionBlock initializer directly on the parsed AST.
    let mut library = parse_only(
        "
        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR
        END_FUNCTION_BLOCK",
    );

    // Rewrite Caller's forward reference to Callee into the eager
    // FunctionBlock form. Caller is declared first, so the referenced
    // type Callee must be reordered before it.
    for element in library.elements.iter_mut() {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
            if fb.name == TypeName::from("Caller") {
                fb.variables[0].initializer = InitialValueAssignmentKind::FunctionBlock(
                    FunctionBlockInitialValueAssignment {
                        type_name: TypeName::from("Callee"),
                        init: vec![],
                    },
                );
            }
        }
    }

    let (library, _reachable) = apply(library).unwrap();

    // Callee (the referenced type) must come before Caller.
    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Callee"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Caller"));
}

#[test]
fn apply_when_function_block_call_style_init_then_referenced_type_ordered_first() {
    // The call-style FB instance initializer (`name : FB_Type(args)`)
    // parses to InitialValueAssignmentKind::FunctionBlockCall, a distinct
    // node that must get the same referenced-type-before-POU dependency
    // edge as the FunctionBlock arm. Caller is declared first but
    // references Callee, so Callee must be reordered before it.
    let program = "
        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee(IN1 := TRUE);
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Callee
            VAR_INPUT
               IN1: BOOL;
            END_VAR
        END_FUNCTION_BLOCK";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Callee"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::FunctionBlockDeclaration);
    assert_eq!(decl.name, TypeName::from("Caller"));
}

#[test]
fn apply_when_nested_enumeration_types() {
    let program = "
TYPE
LEVEL_ALIAS : LEVEL;
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
    assert_eq!(decl.type_name, TypeName::from("LEVEL"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
    assert_eq!(decl.data_type_name, TypeName::from("LEVEL_ALIAS"));
}

#[test]
fn apply_when_nested_string_types() {
    let program = "
TYPE
TYPE_NAME_ALIAS : TYPE_NAME;
TYPE_NAME : STRING[5];
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::String);
    assert_eq!(decl.type_name, TypeName::from("TYPE_NAME"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
    assert_eq!(decl.data_type_name, TypeName::from("TYPE_NAME_ALIAS"));
}

#[test]
fn apply_when_nested_subrange_types() {
    let program = "
TYPE
TYPE_NAME_ALIAS : TYPE_NAME;
TYPE_NAME : INT (1..128);
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Subrange);
    assert_eq!(decl.type_name, TypeName::from("TYPE_NAME"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::LateBound);
    assert_eq!(decl.data_type_name, TypeName::from("TYPE_NAME_ALIAS"));
}

#[test]
fn apply_when_array_of_enum_types() {
    let program = "
TYPE
COLORS_ARRAY : ARRAY[1..2] OF COLOR;
COLOR : (RED, GREEN, BLUE);
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
    assert_eq!(decl.type_name, TypeName::from("COLOR"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Array);
    assert_eq!(decl.type_name, TypeName::from("COLORS_ARRAY"));
}

#[test]
fn apply_when_nested_simple_types() {
    let program = "
TYPE
DEFAULT_2 : DEFAULT_1 := 2;
DEFAULT_1 : INT := 1;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Simple);
    assert_eq!(decl.type_name, TypeName::from("DEFAULT_1"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Simple);
    assert_eq!(decl.type_name, TypeName::from("DEFAULT_2"));
}

#[test]
fn apply_when_nested_structure_types() {
    let program = "
TYPE

OUTER_STRUCT : STRUCT
   MEMBER : INNER_STRUCT;
END_STRUCT;

INNER_STRUCT: STRUCT
   MEMBER : ENUM_TYPE;
END_STRUCT;

ENUM_TYPE : (A, B, C);

END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Enumeration);
    assert_eq!(decl.type_name, TypeName::from("ENUM_TYPE"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(decl.type_name, TypeName::from("INNER_STRUCT"));

    let decl = library.elements.get(2).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(decl.type_name, TypeName::from("OUTER_STRUCT"));
}

#[test]
fn apply_when_initialized_structure_types() {
    let program = "
TYPE

INIT_STRUCT : MY_STRUCT := (MEMBER := 2);

MY_STRUCT : STRUCT
   MEMBER : INT := 1;
END_STRUCT;

END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(decl.type_name, TypeName::from("MY_STRUCT"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::StructureInitialization);
    assert_eq!(decl.type_name, TypeName::from("INIT_STRUCT"));
}

#[test]
fn apply_when_function_calls_another_function_then_callee_ordered_first() {
    let program = "
        FUNCTION INNER : REAL
        VAR_INPUT
            X : REAL;
        END_VAR
            INNER := X * 2.0;
        END_FUNCTION

        FUNCTION OUTER : REAL
        VAR_INPUT
            Y : REAL;
        END_VAR
            OUTER := INNER(X := Y);
        END_FUNCTION

        PROGRAM main
        VAR
            result : REAL;
        END_VAR
            result := OUTER(Y := 3.0);
        END_PROGRAM";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    // INNER must come before OUTER (callee before caller), both before main.
    // Collect just the function declarations in order.
    let func_names: Vec<&Id> = library
        .elements
        .iter()
        .filter_map(|e| {
            if let LibraryElementKind::FunctionDeclaration(f) = e {
                Some(&f.name)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(func_names.len(), 2);
    assert_eq!(func_names[0], &Id::from("INNER"));
    assert_eq!(func_names[1], &Id::from("OUTER"));
}

#[test]
fn apply_when_array_element_is_struct_then_ok() {
    let program = "TYPE subrange_element_type :
  STRUCT
	DAY : SINT;
  END_STRUCT;
END_TYPE

TYPE
  array_container 	: ARRAY [0..29] OF subrange_element_type;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(decl.type_name, TypeName::from("subrange_element_type"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Array);
    assert_eq!(decl.type_name, TypeName::from("array_container"));
}

#[test]
fn apply_when_array_element_is_struct_needs_reorder_then_ok() {
    let program = "
TYPE
  array_container 	: ARRAY [0..29] OF subrange_element_type;
END_TYPE

TYPE subrange_element_type :
  STRUCT
	DAY : SINT;
  END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(decl.type_name, TypeName::from("subrange_element_type"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Array);
    assert_eq!(decl.type_name, TypeName::from("array_container"));
}

#[test]
fn apply_when_unused_function_then_not_in_reachable_set() {
    let program = "
        FUNCTION INNER : REAL
        VAR_INPUT X : REAL; END_VAR
            INNER := X * 2.0;
        END_FUNCTION

        FUNCTION UNUSED : REAL
        VAR_INPUT X : REAL; END_VAR
            UNUSED := X;
        END_FUNCTION

        FUNCTION OUTER : REAL
        VAR_INPUT A : REAL; END_VAR
            OUTER := INNER(X := A);
        END_FUNCTION

        PROGRAM main
        VAR result : REAL; END_VAR
            result := OUTER(A := 3.0);
        END_PROGRAM";

    let library = parse_only(program);
    let (_library, reachable) = apply(library).unwrap();

    assert!(reachable.contains(&Id::from("main")));
    assert!(reachable.contains(&Id::from("OUTER")));
    assert!(reachable.contains(&Id::from("INNER")));
    assert!(!reachable.contains(&Id::from("UNUSED")));
}

#[test]
fn apply_when_top_level_var_global_then_return_ok() {
    let program = "
        VAR_GLOBAL CONSTANT
            MY_LENGTH : INT := 250;
        END_VAR

        FUNCTION MY_FUNC : INT
        VAR_INPUT
            x : INT;
        END_VAR
            MY_FUNC := x;
        END_FUNCTION

        PROGRAM main
        VAR
            result : INT;
        END_VAR
            result := MY_FUNC(x := 1);
        END_PROGRAM";

    let library = {
        use ironplc_parser::{options::CompilerOptions, parse_program};
        parse_program(
            program,
            &FileId::default(),
            &CompilerOptions {
                allow_top_level_var_global: true,
                ..Default::default()
            },
        )
        .unwrap()
    };

    let (library, _reachable) = apply(library).unwrap();

    // Global var declarations should come first
    let first = library.elements.first().unwrap();
    assert!(matches!(
        first,
        LibraryElementKind::GlobalVarDeclarations(_)
    ));
}

// ---------------------------------------------------------------------
// Array element type dependency edge for array-typed struct fields.
// ---------------------------------------------------------------------

/// Returns the position of the named structure declaration in the sorted
/// library, or `None` when the library does not declare that structure.
fn structure_position(library: &Library, name: &str) -> Option<usize> {
    library.elements.iter().position(|element| match element {
        LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(decl)) => {
            decl.type_name == TypeName::from(name)
        }
        _ => false,
    })
}

/// Position of the enumeration declaration named `name` in the sorted
/// library, or `None` when the library does not declare that enumeration.
fn enumeration_position(library: &Library, name: &str) -> Option<usize> {
    library.elements.iter().position(|element| match element {
        LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(decl)) => {
            decl.type_name == TypeName::from(name)
        }
        _ => false,
    })
}

#[test]
fn apply_when_struct_array_field_element_declared_first_then_element_ordered_first() {
    // Declaration order already matches dependency order. The element type
    // must still be ordered ahead of the struct that arrays over it --
    // without a dependency edge the sort is free to emit either order.
    let program = "
TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let item = structure_position(&library, "Item").unwrap();
    let holder = structure_position(&library, "Holder").unwrap();
    assert!(item < holder, "Item must be ordered before Holder");
}

#[test]
fn apply_when_struct_array_field_element_declared_last_then_element_ordered_first() {
    // Forward reference: the element type is declared textually *after*
    // the struct whose array field references it.
    let program = "
TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
END_STRUCT;
END_TYPE

TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let item = structure_position(&library, "Item").unwrap();
    let holder = structure_position(&library, "Holder").unwrap();
    assert!(item < holder, "Item must be ordered before Holder");
}

#[test]
fn apply_when_struct_array_field_element_is_elementary_then_return_ok() {
    // Elementary element types have no declaration to order against. The
    // added edge must not make the graph unsortable.
    let program = "
TYPE Holder : STRUCT
    Nums : ARRAY[1..4] OF INT;
    Flags : ARRAY[1..2] OF BOOL;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    assert!(structure_position(&library, "Holder").is_some());
}

#[test]
fn apply_when_struct_array_field_is_self_recursive_then_return_error() {
    // An array of the enclosing struct is infinitely sized. The new edge
    // makes this a genuine cycle, which must be reported as such.
    let program = "
TYPE A : STRUCT
    Items : ARRAY[1..2] OF A;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let result = apply(library);
    assert_eq!(
        result.unwrap_err().first().unwrap().code,
        Problem::RecursiveCycle.code().to_string()
    );
}

#[test]
fn apply_when_struct_array_fields_are_mutually_recursive_then_return_error() {
    let program = "
TYPE A : STRUCT
    Items : ARRAY[1..2] OF B;
END_STRUCT;
END_TYPE

TYPE B : STRUCT
    Items : ARRAY[1..2] OF A;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let result = apply(library);
    assert_eq!(
        result.unwrap_err().first().unwrap().code,
        Problem::RecursiveCycle.code().to_string()
    );
}

#[test]
fn resolve_types_when_struct_array_field_element_declared_before_program_then_return_ok() {
    // Pipeline-level regression guard for the reported symptom: this
    // layout previously failed with P2013 because the element type was
    // absent from the type environment when the array field was resolved.
    // See https://github.com/ironplc/ironplc/issues/1376.
    use ironplc_parser::options::CompilerOptions;

    let program = "
TYPE Item : STRUCT
    Flag : BOOL;
END_STRUCT;
END_TYPE

TYPE Holder : STRUCT
    Items : ARRAY[1..6] OF Item;
    Other : BOOL;
END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    H : Holder;
END_VAR
    H.Other := TRUE;
END_PROGRAM";

    let library = parse_only(program);
    let (_library, context) =
        crate::stages::resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    assert!(
        !context.has_diagnostics(),
        "expected type resolution to succeed, got {:?}",
        context.diagnostics()
    );
}

#[test]
fn apply_when_struct_enum_field_initialized_and_enum_declared_first_then_enum_ordered_first() {
    // Declaration order already matches dependency order. The qualified
    // initializer parses straight to the EnumeratedType arm, which must
    // record the edge so the sort is not free to emit either order.
    let program = "
TYPE Color : (RED, GREEN, BLUE); END_TYPE

TYPE Thing : STRUCT
    c : Color := Color#GREEN;
    n : INT;
END_STRUCT;
END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let color = enumeration_position(&library, "Color").unwrap();
    let thing = structure_position(&library, "Thing").unwrap();
    assert!(color < thing, "Color must be ordered before Thing");
}

#[test]
fn apply_when_struct_enum_field_initialized_and_enum_declared_last_then_enum_ordered_first() {
    // Forward reference: the enumeration is declared textually *after*
    // the struct whose initialized field references it.
    let program = "
TYPE Thing : STRUCT
    c : Color := Color#GREEN;
    n : INT;
END_STRUCT;
END_TYPE

TYPE Color : (RED, GREEN, BLUE); END_TYPE";

    let library = parse_only(program);
    let (library, _reachable) = apply(library).unwrap();

    let color = enumeration_position(&library, "Color").unwrap();
    let thing = structure_position(&library, "Thing").unwrap();
    assert!(color < thing, "Color must be ordered before Thing");
}

#[test]
fn resolve_types_when_struct_enum_field_initialized_and_enum_declared_first_then_return_ok() {
    // Pipeline-level regression guard for the reported symptom: this
    // layout previously failed with P2021 and P2004 because the
    // enumeration was absent from the type environment when the
    // initialized field was resolved. Removing the initializer, or
    // swapping the two TYPE blocks, made it pass.
    // See https://github.com/ironplc/ironplc/issues/1593.
    use ironplc_parser::options::CompilerOptions;

    let program = "
TYPE Color : (RED, GREEN, BLUE); END_TYPE

TYPE Thing : STRUCT
    c : Color := Color#GREEN;
    n : INT;
END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    t : Thing;
    r : INT;
END_VAR
    r := t.n;
END_PROGRAM";

    let library = parse_only(program);
    let (_library, context) =
        crate::stages::resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    assert!(
        !context.has_diagnostics(),
        "expected type resolution to succeed, got {:?}",
        context.diagnostics()
    );
}
#[test]
fn apply_when_reference_target_declared_last_then_target_ordered_first() {
    // `TYPE ArrRef : REF_TO ARR4` depends on ARR4 exactly as an array
    // alias depends on its base type.
    // See https://github.com/ironplc/ironplc/issues/1580.
    use ironplc_parser::options::{CompilerOptions, Dialect};

    let program = "
TYPE
  ArrRef : REF_TO ARR4;
  ARR4 : ARRAY[0..3] OF INT;
END_TYPE";

    let library = ironplc_parser::parse_program(
        program,
        &FileId::default(),
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
    .unwrap();
    let (library, _reachable) = apply(library).unwrap();

    let decl = library.elements.first().unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Array);
    assert_eq!(decl.type_name, TypeName::from("ARR4"));

    let decl = library.elements.get(1).unwrap();
    let decl = cast!(decl, LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(decl, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.type_name, TypeName::from("ArrRef"));
}

#[test]
fn resolve_types_when_reference_type_targets_named_array_type_then_return_ok() {
    // Pipeline-level guard: before the reference declaration carried a
    // dependency edge, this layout resolved `ArrRef` before `ARR4` in
    // some runs and reported P2011 for a type that is declared.
    // See https://github.com/ironplc/ironplc/issues/1580.
    use ironplc_parser::options::{CompilerOptions, Dialect};

    let program = "
TYPE
  ARR4 : ARRAY[0..3] OF INT;
  ArrRef : REF_TO ARR4;
END_TYPE

PROGRAM Main
VAR
    arr : ARR4;
    pt : ArrRef;
END_VAR
    pt := REF(arr);
END_PROGRAM";

    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let library = ironplc_parser::parse_program(program, &FileId::default(), &options).unwrap();
    let (_library, context) = crate::stages::resolve_types(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "expected type resolution to succeed, got {:?}",
        context.diagnostics()
    );
}
