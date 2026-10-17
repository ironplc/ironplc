//! Whether two type ids are the same type.

use ironplc_dsl::type_id::TypeId;

use super::TypeEnvironment;

impl TypeEnvironment {
    /// Whether the types `a` and `b` are the same type.
    ///
    /// A type is identified by its id, so a type and an alias of it are two
    /// types. A type spelled out in place (`ARRAY[1..3] OF INT`,
    /// `STRING[10]`) is a type of its own, so two declarations that spell one
    /// have two ids; they are the same type when they spell the same shape.
    /// A type is spelled out in place when no name maps to its id: a sized
    /// string has the name of its elementary string type, but that name maps
    /// to the elementary type.
    ///
    /// Comparing two shapes does not compare the members of an enumeration,
    /// so an enumeration spelled in place would need them compared here as
    /// well.
    pub(crate) fn same_type(&self, a: TypeId, b: TypeId) -> bool {
        if a == b {
            return true;
        }
        if self.is_named(a) || self.is_named(b) {
            return false;
        }
        match (self.get_by_id(a), self.get_by_id(b)) {
            (Some(a), Some(b)) => a.representation == b.representation,
            _ => false,
        }
    }

    /// Whether the types `a` and `b` are the same type, as
    /// [`Self::same_type`], when both resolved. A type that did not resolve
    /// is the same as any: the rule that reports it is not the one asking.
    pub(super) fn same_type_if_resolved(&self, a: Option<TypeId>, b: Option<TypeId>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => self.same_type(a, b),
            _ => true,
        }
    }

    /// Whether a name maps to `id`, rather than `id` being a type spelled
    /// out in place.
    fn is_named(&self, id: TypeId) -> bool {
        self.name_of(id).and_then(|name| self.id_of(name)) == Some(id)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::test_helpers::{declared_ids, parse_and_resolve_types_with_context};

    /// `a` and `b` declared with the types `a_type` and `b_type`.
    fn same_type(a_type: &str, b_type: &str) -> bool {
        let program = format!(
            "
TYPE
  Speed : INT;
  Triple : ARRAY[1..3] OF INT;
END_TYPE
PROGRAM main
VAR
  a : {a_type};
  b : {b_type};
END_VAR
END_PROGRAM"
        );
        let (library, context) = parse_and_resolve_types_with_context(&program);
        let ids = declared_ids(&library);
        let (a, b) = (ids["a"].unwrap(), ids["b"].unwrap());
        context.types().same_type(a, b)
    }

    #[rstest]
    #[case::elementary("INT", "INT")]
    #[case::alias("Speed", "Speed")]
    #[case::named_array("Triple", "Triple")]
    #[case::array_in_place("ARRAY[1..3] OF INT", "ARRAY[1..3] OF INT")]
    #[case::sized_string("STRING[10]", "STRING[10]")]
    fn same_type_when_same_type_then_true(#[case] a: &str, #[case] b: &str) {
        assert!(same_type(a, b));
    }

    #[rstest]
    #[case::elementary("INT", "DINT")]
    #[case::alias_and_base("Speed", "INT")]
    #[case::named_and_in_place("Triple", "ARRAY[1..3] OF INT")]
    #[case::array_bounds("ARRAY[1..3] OF INT", "ARRAY[1..4] OF INT")]
    #[case::string_length("STRING[10]", "STRING[20]")]
    #[case::string_width("STRING[10]", "WSTRING[10]")]
    #[case::sized_and_unsized_string("STRING[10]", "STRING")]
    fn same_type_when_different_type_then_false(#[case] a: &str, #[case] b: &str) {
        assert!(!same_type(a, b));
    }
}
