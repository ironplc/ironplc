//! The parts of an initializer the program wrote.
//!
//! The analyzer completes every declaration's initializer with the value the
//! declaration starts with: the default of a field the initializer leaves
//! out, the elements an array initializer does not reach, the value of a
//! declaration that states none. Every part it supplies has a synthesized
//! span (`SourceSpan::synthesized`). A rendering shows the program as it was
//! written, so it leaves those parts out.

use ironplc_dsl::common::{
    ArrayInitialElementKind, ConstantKind, EnumeratedValue, ReferenceInitialValue, SignedInteger,
    StructureElementInit,
};
use ironplc_dsl::core::Located;

/// Whether the program wrote the literal `constant`.
pub(crate) fn constant(constant: &&ConstantKind) -> bool {
    !constant.span().is_synthesized()
}

/// Whether the program wrote the subrange value `value`.
pub(crate) fn signed_integer(value: &&SignedInteger) -> bool {
    !value.value.span.is_synthesized()
}

/// Whether the program wrote the enumerated value `value`.
pub(crate) fn enumerated(value: &&EnumeratedValue) -> bool {
    !value.value.span.is_synthesized()
}

/// Whether the program wrote the reference value `value`.
pub(crate) fn reference(value: &&ReferenceInitialValue) -> bool {
    match value {
        ReferenceInitialValue::Null(span) => !span.is_synthesized(),
        ReferenceInitialValue::Ref(_) => true,
    }
}

/// The elements of an array initializer up to the last one the program
/// wrote. An element the analyzer supplied before that one (from `n()`)
/// stays, so that the elements keep their positions.
pub(crate) fn elements(elements: &[ArrayInitialElementKind]) -> &[ArrayInitialElementKind] {
    let count = elements
        .iter()
        .rposition(is_written_element)
        .map_or(0, |last| last + 1);
    &elements[..count]
}

/// The members of a structure initializer the program wrote.
pub(crate) fn members(members: &[StructureElementInit]) -> Vec<&StructureElementInit> {
    members
        .iter()
        .filter(|member| !member.name.span.is_synthesized())
        .collect()
}

fn is_written_element(element: &ArrayInitialElementKind) -> bool {
    match element {
        ArrayInitialElementKind::Constant(literal) => constant(&literal),
        ArrayInitialElementKind::EnumValue(value) => enumerated(&value),
        ArrayInitialElementKind::Repeated(_) => true,
        ArrayInitialElementKind::Structure(fields) => !members(fields).is_empty(),
        ArrayInitialElementKind::Expression(expr) => !expr.span.is_synthesized(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::IntegerLiteral;
    use ironplc_dsl::core::SourceSpan;

    fn element(span: SourceSpan) -> ArrayInitialElementKind {
        ArrayInitialElementKind::Constant(ConstantKind::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new("1", span).unwrap(),
            data_type: None,
        }))
    }

    #[test]
    fn elements_when_trailing_synthesized_then_cut_after_last_written() {
        let list = vec![
            element(SourceSpan::range(1, 2)),
            element(SourceSpan::synthesized()),
            element(SourceSpan::range(3, 4)),
            element(SourceSpan::synthesized()),
        ];

        assert_eq!(elements(&list).len(), 3);
    }

    #[test]
    fn elements_when_all_synthesized_then_empty() {
        let list = vec![element(SourceSpan::synthesized())];

        assert!(elements(&list).is_empty());
    }
}
