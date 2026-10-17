//! Tests of the `Index` projection's constructor and text form.

use super::*;

/// An `ARRAY[1..5, 0..3] OF DINT` global, and the fixture it is added to.
fn with_array() -> (Fixture, VarId) {
    let mut f = fixture();
    let element = f.ty(f.dint);
    let array =
        f.b.push_type(
            entry(TypeKind::Array {
                element,
                dimensions: vec![(1, 5), (0, 3)],
            }),
            &span(),
        )
        .unwrap();
    let var = f.b.push_variable(name("a"), Owner::Global, array).unwrap();
    (f, var)
}

/// REQ-LOW-lowering-031: an `Index` applies to an array, with one subscript
/// per dimension, and its place has the element's type.
#[test]
fn index_when_one_subscript_per_dimension_then_element_place() {
    let (f, a) = with_array();
    let place =
        f.b.index(f.place(a), vec![int(2), f.read(f.dint)], &span())
            .unwrap();
    assert_eq!(place.ty(), f.ty(f.dint));
    assert_eq!(place.to_string(), "v8[(const i32s 2), (read i32s v0)]");
}

/// REQ-LOW-lowering-031: the number of subscripts is the number of
/// dimensions.
#[test]
fn index_when_subscript_count_differs_from_dimensions_then_internal_error() {
    let (f, a) = with_array();
    assert!(is_internal_error_naming(
        f.b.index(f.place(a), vec![int(2)], &span()),
        "Place::index"
    ));
}

/// REQ-LOW-lowering-031: only an array is indexed.
#[test]
fn index_when_place_not_array_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.index(f.place(f.dint), vec![int(1)], &span()),
        "Place::index"
    ));
}

/// REQ-LOW-lowering-161: every subscript is a `DINT`.
#[test]
fn index_when_subscript_not_dint_then_internal_error() {
    let (f, a) = with_array();
    let wide = Expr::integer(2, ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert!(is_internal_error_naming(
        f.b.index(f.place(a), vec![wide, int(0)], &span()),
        "Place::index"
    ));
}

#[test]
fn index_when_subscript_reads_another_program_then_internal_error() {
    let (f, a) = with_array();
    let g = fixture();
    assert!(is_internal_error_naming(
        f.b.index(f.place(a), vec![int(2), g.read(g.dint)], &span()),
        "Place::index"
    ));
}
