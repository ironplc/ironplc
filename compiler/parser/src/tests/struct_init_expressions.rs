//! General expressions as struct/FB-instance initializer values, e.g.
//! `tonDelta : TON := (PT := pDevice^.Delta);`. The parser accepts the value
//! expression unconditionally (a permissive superset); the
//! `--allow-struct-initializer-expressions` flag is enforced by a later
//! semantic rule, not here.
//!
//! The declaration itself parses to `LateResolvedTypeInit`, not `Structure`:
//! `x : T := (a := 1)` is the same spelling whether `T` names a STRUCT or a
//! function block, and nothing here knows which.

use super::common::*;
use dsl::common::StructInitialValueAssignmentKind;

#[test]
fn parse_when_struct_init_value_is_deref_member_expr_then_parses_as_expression() {
    // Real motivating shape: a call-style FB-instance initializer whose
    // value is a genuinely runtime expression (dereference + member
    // access), not a compile-time constant.
    let source = "
FUNCTION_BLOCK FB_Device
VAR_INPUT
    Delta : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    pDevice : REF_TO FB_Device;
    tonDelta : TON := (PT := pDevice^.Delta);
END_VAR
END_FUNCTION_BLOCK";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    let struct_init = cast!(
        &fb.variables[1].initializer,
        InitialValueAssignmentKind::LateResolvedTypeInit
    );
    assert_eq!(struct_init.elements_init.len(), 1);
    assert!(matches!(
        struct_init.elements_init[0].init,
        StructInitialValueAssignmentKind::Expression(_)
    ));
}

/// A bare identifier is recorded as unresolved, not guessed at.
///
/// `g` here is a variable and `RED` in the next test is an enumeration
/// value, and the two are the same token in the same position: the parser
/// has no declarations in scope and cannot tell them apart. Committing to
/// either would make a later diagnostic name a construct the program does
/// not contain, so both parse to `LateBound` and
/// `xform_resolve_late_bound_expr_kind` decides.
#[test]
fn parse_when_struct_init_value_is_bare_identifier_then_parses_as_late_bound() {
    let source = "
TYPE MyStruct :
STRUCT
    x : INT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    g : INT;
    s : MyStruct := (x := g);
END_VAR
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();

    assert!(matches!(
        sole_struct_element_init(&library),
        StructInitialValueAssignmentKind::LateBound(late_bound) if late_bound.value == Id::from("g")
    ));
}

/// A name that happens to be an enumeration value parses identically -- the
/// parser cannot and does not distinguish them.
#[test]
fn parse_when_struct_init_value_is_enumeration_value_then_also_parses_as_late_bound() {
    let source = "
TYPE Color : (RED, GREEN); END_TYPE

TYPE MyStruct :
STRUCT
    c : Color;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    s : MyStruct := (c := RED);
END_VAR
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();

    assert!(matches!(
        sole_struct_element_init(&library),
        StructInitialValueAssignmentKind::LateBound(late_bound)
            if late_bound.value == Id::from("RED")
    ));
}

/// A qualified `Type#VALUE` needs no resolution and keeps its own node.
#[test]
fn parse_when_struct_init_value_is_qualified_enumeration_value_then_parses_as_enumerated_value() {
    let source = "
TYPE Color : (RED, GREEN); END_TYPE

TYPE MyStruct :
STRUCT
    c : Color;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    s : MyStruct := (c := Color#GREEN);
END_VAR
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();

    assert!(matches!(
        sole_struct_element_init(&library),
        StructInitialValueAssignmentKind::EnumeratedValue(value)
            if value.type_name == Some(TypeName::from("Color"))
    ));
}

/// Returns the initializer value of the program's single structure element.
fn sole_struct_element_init(library: &Library) -> &StructInitialValueAssignmentKind {
    let program = library
        .elements
        .iter()
        .find_map(|e| match e {
            LibraryElementKind::ProgramDeclaration(program) => Some(program),
            _ => None,
        })
        .unwrap();
    let struct_init = cast!(
        &program.variables.last().unwrap().initializer,
        InitialValueAssignmentKind::LateResolvedTypeInit
    );
    &struct_init.elements_init[0].init
}
