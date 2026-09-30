//! Arrays of arrays, laid out as multi-dimensional arrays.
//!
//! An array whose element type is itself an array (`ARRAY[1..2] OF Row`
//! with `Row : ARRAY[1..3] OF DINT`) occupies the same storage as the
//! multi-dimensional array with the outer bounds followed by the inner ones
//! (`ARRAY[1..2, 1..3] OF DINT`): row-major order keeps each inner array
//! contiguous. Code generation already joins the subscripts of every
//! bracket of a chain into one index, so `rows[i][j]` addresses the element
//! `[i, j]` once the array is described by its flattened dimensions.
//!
//! The analyzer rejects a selection that stops at a whole inner array
//! (`rows[i]`), whose copy this layout does not provide.

use ironplc_analyzer::intermediate_type::{ArrayDimension, IntermediateType};

/// The innermost element type of an array with element type `element` and
/// bounds `dimensions`, and the bounds of the multi-dimensional array it is
/// laid out as: the outer bounds, then those of each nested element array.
///
/// An element that is not an array is returned with `dimensions` unchanged.
pub(crate) fn flatten<'t>(
    element: &'t IntermediateType,
    dimensions: &[ArrayDimension],
) -> (&'t IntermediateType, Vec<ArrayDimension>) {
    let mut all = dimensions.to_vec();
    let mut leaf = element;
    while let IntermediateType::Array {
        element_type,
        dimensions,
    } = leaf
    {
        all.extend(dimensions.iter().cloned());
        leaf = element_type.as_ref();
    }
    (leaf, all)
}

#[cfg(test)]
mod tests {
    use super::flatten;
    use ironplc_analyzer::intermediate_type::{ArrayDimension, ByteSized, IntermediateType};

    fn dim(lower: i32, upper: i32) -> ArrayDimension {
        ArrayDimension { lower, upper }
    }

    fn dint() -> IntermediateType {
        IntermediateType::Int {
            size: ByteSized::B32,
        }
    }

    #[test]
    fn flatten_when_element_not_array_then_dimensions_unchanged() {
        let element = dint();
        let (leaf, dimensions) = flatten(&element, &[dim(1, 2)]);
        assert_eq!(leaf, &dint());
        assert_eq!(dimensions, vec![dim(1, 2)]);
    }

    #[test]
    fn flatten_when_element_nested_arrays_then_dimensions_outer_first() {
        let element = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Array {
                element_type: Box::new(dint()),
                dimensions: vec![dim(0, 4)],
            }),
            dimensions: vec![dim(1, 3), dim(2, 5)],
        };
        let (leaf, dimensions) = flatten(&element, &[dim(1, 2)]);
        assert_eq!(leaf, &dint());
        assert_eq!(dimensions, vec![dim(1, 2), dim(1, 3), dim(2, 5), dim(0, 4)]);
    }
}
