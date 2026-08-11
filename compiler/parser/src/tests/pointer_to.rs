//! `POINTER TO` pointer type declarations (`--allow-pointer-to`).

use super::common::*;

/// Parse with `allow_pointer_to` enabled. The default (strict IEC 61131-3)
/// dialect rejects the syntax.
fn parse_text_pointer_to(source: &str) -> Library {
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    result.unwrap()
}

#[test]
fn pointer_to_when_flag_enabled_then_parses() {
    let lib = parse_text_pointer_to(
        "PROGRAM main
VAR
    p : POINTER TO INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert_eq!(init.syntax, dsl::common::RefSyntax::PointerTo);
    assert_eq!(init.target.type_name().unwrap().to_string(), "INT");
}

#[test]
fn pointer_to_when_flag_disabled_then_rejected() {
    let source = "PROGRAM main
VAR
    p : POINTER TO INT;
END_VAR
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err(), "POINTER TO must be rejected without the flag");
}

/// With the flag off, `POINTER` demotes to an identifier and remains usable
/// as an ordinary variable name.
#[test]
fn pointer_to_when_flag_disabled_then_pointer_is_identifier() {
    let source = "PROGRAM main
VAR
    POINTER : INT;
END_VAR
    POINTER := 5;
END_PROGRAM";
    let lib = parse_text(source);
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let name = prog.variables[0].identifier.symbolic_id().unwrap();
    assert_eq!(name.to_string(), "POINTER");
}

#[test]
fn pointer_to_when_type_declaration_then_tagged_pointer_to() {
    let lib = parse_text_pointer_to("TYPE IntPtr : POINTER TO INT; END_TYPE");
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.type_name.to_string(), "IntPtr");
    assert_eq!(decl.syntax, dsl::common::RefSyntax::PointerTo);
}

#[test]
fn pointer_to_when_array_element_then_tagged_pointer_to() {
    let lib = parse_text_pointer_to(
        "PROGRAM main
VAR
    a : ARRAY[0..3] OF POINTER TO INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.ref_to, Some(dsl::common::RefSyntax::PointerTo));
}

#[test]
fn pointer_to_when_pointer_to_array_then_target_is_array() {
    let lib = parse_text_pointer_to(
        "PROGRAM main
VAR
    p : POINTER TO ARRAY[1..10] OF INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(init.target, ReferenceTarget::Array(_)));
}

/// The `^` dereference works on `POINTER TO` variables with only
/// `allow_pointer_to` set (no `allow_ref_to` needed).
#[test]
fn pointer_to_when_deref_then_parses() {
    let lib = parse_text_pointer_to(
        "PROGRAM main
VAR
    p : POINTER TO INT;
    value : INT;
END_VAR
    value := p^;
    p^ := 42;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 2);
    let store = cast!(&s.body[1], StmtKind::Assignment);
    assert!(store.deref);
}

/// `NULL` and `REF()` initializers come from the `REF_TO` family, so a
/// pointer can be initialized when `allow_ref_to` is also on (e.g. the
/// `codesys` dialect).
#[test]
fn pointer_to_when_ref_init_and_ref_to_enabled_then_parses() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM main
VAR
    counter : INT;
    p : POINTER TO INT := REF(counter);
    q : POINTER TO INT := NULL;
END_VAR
END_PROGRAM";
    let lib = parse_program(source, &FileId::default(), &options).unwrap();
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let p_init = cast!(
        &prog.variables[1].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(
        p_init.initial_value,
        Some(dsl::common::ReferenceInitialValue::Ref(_))
    ));
    let q_init = cast!(
        &prog.variables[2].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(
        q_init.initial_value,
        Some(dsl::common::ReferenceInitialValue::Null(_))
    ));
}
