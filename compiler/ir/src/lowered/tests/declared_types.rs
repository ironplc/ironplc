//! Tests of types declared before they are defined (REQ-LOW-ir-194): a
//! function block that refers to itself through a reference field.

use super::*;

/// A program with a function block `Node`, declared, and a nullable
/// reference to it, which the block's field `next` will be.
struct Declared {
    b: ProgramBuilder,
    body: PouId,
    node: TypeId,
    reference: TypeId,
}

fn declared() -> Declared {
    let mut b = ProgramBuilder::new();
    let body = b.push_pou(name("Node"));
    let node = b.declare_type();
    let reference = b
        .push_type(
            entry(TypeKind::Reference {
                target: node,
                nullability: Nullability::Nullable,
            }),
            &span(),
        )
        .unwrap();
    Declared {
        b,
        body,
        node,
        reference,
    }
}

/// `Node`, whose one input `next` is a reference to `Node`.
fn node_entry(d: &Declared) -> TypeEntry {
    entry(TypeKind::FunctionBlock {
        fields: vec![Field::new(
            "next",
            span(),
            Some(Section::Input),
            d.reference,
        )],
        body: d.body,
        base: None,
    })
}

/// A reference refers to a declared type before its definition, and once
/// defined the type has the entry it was given, at the id it was declared
/// with: a block whose field refers to the block itself.
#[test]
fn define_type_when_block_refers_to_itself_then_defined_at_declared_id() {
    let mut d = declared();
    assert!(d.b.types().get(d.node).is_none());

    let entry = node_entry(&d);
    d.b.define_type(d.node, entry, &span()).unwrap();

    let fields = d.b.types().fields(d.node).unwrap();
    assert_eq!(fields[0].ty, d.reference);
    assert_eq!(d.b.types().reference(d.reference).unwrap().0, d.node);
    assert!(d.b.types().iter().any(|(id, _)| id == d.node));
}

/// Only a reference refers to a type before its definition: an array of
/// it, or a field of it, is refused, as for a type the table does not
/// hold.
#[test]
fn push_type_when_part_other_than_reference_target_is_declared_then_internal_error() {
    let mut d = declared();
    let node = d.node;
    assert!(is_internal_error_naming(
        d.b.push_type(
            entry(TypeKind::Array {
                element: node,
                dimensions: vec![(1, 2)],
            }),
            &span(),
        ),
        "TypeTable::push"
    ));
    assert!(is_internal_error_naming(
        d.b.push_type(
            entry(TypeKind::Structure {
                fields: vec![Field::new("n", span(), None, node)],
            }),
            &span(),
        ),
        "TypeTable::push"
    ));
}

/// A type is defined once, and only a declared one is: a second
/// definition, an id the table did not declare, and an entry the table
/// would refuse are each a P9998.
#[test]
fn define_type_when_not_a_declared_undefined_type_then_internal_error() {
    let mut d = declared();
    let entry = node_entry(&d);
    d.b.define_type(d.node, entry, &span()).unwrap();
    let again = node_entry(&d);
    assert!(is_internal_error_naming(
        d.b.define_type(d.node, again, &span()),
        "TypeTable::define"
    ));

    let other = declared();
    let foreign = node_entry(&d);
    assert!(is_internal_error_naming(
        d.b.define_type(other.node, foreign, &span()),
        "TypeTable::define"
    ));

    let unknown = d.b.declare_type();
    assert!(is_internal_error_naming(
        d.b.define_type(unknown, entry_of_inverted_array(d.reference), &span()),
        "TypeTable::define"
    ));
}

/// A definition's body is a POU of the same program, as for `push_type`.
#[test]
fn define_type_when_body_from_another_program_then_internal_error() {
    let mut d = declared();
    let mut other = ProgramBuilder::new();
    let foreign = other.push_pou(name("elsewhere"));
    let reference = d.reference;
    assert!(is_internal_error_naming(
        d.b.define_type(
            d.node,
            entry(TypeKind::FunctionBlock {
                fields: vec![Field::new("next", span(), Some(Section::Input), reference)],
                body: foreign,
                base: None,
            }),
            &span(),
        ),
        "ProgramBuilder::define_type"
    ));
}

/// A declared type that is never defined has no entry: nothing finds its
/// kind or fields, and the table's iteration skips it.
#[test]
fn types_when_declared_type_not_defined_then_no_entry() {
    let d = declared();
    assert!(d.b.types().kind(d.node).is_none());
    assert!(d.b.types().fields(d.node).is_none());
    assert!(d.b.types().operation_type(d.node).is_none());
    assert!(!d.b.types().iter().any(|(id, _)| id == d.node));
}

/// An array whose one dimension is inverted, which the table refuses.
fn entry_of_inverted_array(element: TypeId) -> TypeEntry {
    entry(TypeKind::Array {
        element,
        dimensions: vec![(2, 1)],
    })
}
