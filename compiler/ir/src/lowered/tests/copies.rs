//! Tests of the `Copy` statement's constructor and text form.

use super::*;

impl Fixture {
    /// A global of a new type of kind `kind`.
    fn global(&mut self, text: &str, kind: TypeKind) -> VarId {
        let ty = self.b.push_type(entry(kind), &span()).unwrap();
        self.b.push_variable(name(text), Owner::Global, ty).unwrap()
    }

    /// A global of a new `ARRAY` type of `DINT` with `dimensions`.
    fn array(&mut self, text: &str, dimensions: Vec<(i32, i32)>) -> VarId {
        let element = self.ty(self.dint);
        self.global(
            text,
            TypeKind::Array {
                element,
                dimensions,
            },
        )
    }

    /// A global of a new structure type with a `DINT` field of each name.
    fn structure(&mut self, text: &str, fields: &[&str]) -> VarId {
        let ty = self.ty(self.dint);
        let fields = fields
            .iter()
            .map(|field| Field::new(field, span(), None, ty))
            .collect();
        self.global(text, TypeKind::Structure { fields })
    }

    fn copy(&self, dst: VarId, src: VarId) -> Result<Stmt, ironplc_dsl::diagnostic::Diagnostic> {
        self.b.copy(self.place(dst), self.place(src), span())
    }
}

/// REQ-LOW-lowering-061: an aggregate is copied from a place of its own
/// type.
#[test]
fn copy_when_same_type_then_copy_statement() {
    let mut f = fixture();
    let a = f.array("a", vec![(1, 2)]);
    let stmt = f.copy(a, a).unwrap();
    assert!(matches!(stmt.kind(), StmtKind::Copy { .. }));
    assert_eq!(stmt.to_string(), "copy v8 v8");
}

/// REQ-LOW-lowering-061: two array types of the same dimensions and element
/// have the same shape, as two inline `ARRAY[1..2] OF DINT` do.
#[test]
fn copy_when_arrays_of_identical_shape_then_copy_statement() {
    let mut f = fixture();
    let a = f.array("a", vec![(1, 2), (0, 3)]);
    let b = f.array("b", vec![(1, 2), (0, 3)]);
    assert_eq!(f.copy(a, b).unwrap().to_string(), "copy v8 v9");
}

/// REQ-LOW-lowering-061: two structure types with the same fields, by name
/// in any case, have the same shape.
#[test]
fn copy_when_structures_of_identical_shape_then_copy_statement() {
    let mut f = fixture();
    let a = f.structure("a", &["x", "y"]);
    let b = f.structure("b", &["X", "y"]);
    assert!(f.copy(a, b).is_ok());
}

/// REQ-LOW-lowering-061: arrays of other extents differ in shape.
#[test]
fn copy_when_extents_differ_then_internal_error() {
    let mut f = fixture();
    let a = f.array("a", vec![(1, 2)]);
    let b = f.array("b", vec![(1, 5)]);
    assert!(is_internal_error_naming(f.copy(a, b), "Stmt::copy"));
}

/// REQ-LOW-lowering-061: arrays of as many elements in other dimensions
/// differ in shape.
#[test]
fn copy_when_dimensions_differ_with_same_count_then_internal_error() {
    let mut f = fixture();
    let a = f.array("a", vec![(1, 6)]);
    let b = f.array("b", vec![(1, 2), (1, 3)]);
    assert!(is_internal_error_naming(f.copy(a, b), "Stmt::copy"));
}

/// REQ-LOW-lowering-061: structures with fields of other names differ in
/// shape.
#[test]
fn copy_when_field_names_differ_then_internal_error() {
    let mut f = fixture();
    let a = f.structure("a", &["x", "y"]);
    let b = f.structure("b", &["x", "z"]);
    assert!(is_internal_error_naming(f.copy(a, b), "Stmt::copy"));
}

/// REQ-LOW-lowering-061: an array and a structure differ in shape.
#[test]
fn copy_when_array_from_structure_then_internal_error() {
    let mut f = fixture();
    let a = f.array("a", vec![(1, 1)]);
    let b = f.structure("b", &["x"]);
    assert!(is_internal_error_naming(f.copy(a, b), "Stmt::copy"));
}

/// A scalar is assigned, not copied.
#[test]
fn copy_when_destination_scalar_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.copy(f.dint, f.dint),
        "Stmt::copy"
    ));
}
