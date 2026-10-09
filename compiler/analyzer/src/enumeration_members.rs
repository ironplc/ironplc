//! The members of an enumeration type, recorded with the type.
//!
//! An enumeration's members, the value each one has at runtime (its
//! ordinal) and the type's default are decided once, here, when the type
//! enters the type environment. Every consumer -- the semantic rules, a
//! code generator, the language server -- reads them from the type its
//! [`TypeId`](ironplc_dsl::type_id::TypeId) identifies instead of working
//! them out again from the declaration.
//!
//! ```ignore
//! TYPE
//!     COLOR : (RED, GREEN := 5, BLUE) := GREEN;   (* RED 0, GREEN 5, BLUE 6; default 5 *)
//!     PAINT : COLOR;                              (* the members of COLOR *)
//! END_TYPE
//! VAR
//!     e : (A, B);                                 (* an anonymous type: A 0, B 1; default 0 *)
//! END_VAR
//! ```

use ironplc_dsl::common::EnumeratedValue;
use ironplc_dsl::core::Id;

use crate::intermediates::enumeration::resolve_ordinal_values;
use crate::semantic_type::SemanticType;

/// One member of an enumeration.
#[derive(Debug, Clone)]
pub struct EnumerationMember {
    /// The member's name, as the declaration spells it.
    pub name: Id,
    /// The member's value at runtime: its explicit value (`GREEN := 5`), or
    /// one more than the member before it, starting at 0.
    pub ordinal: i64,
}

/// The members of an enumeration type in declaration order, and its default.
///
/// An alias (`PAINT : COLOR`) has the members of the enumeration its alias
/// chain ends at, and that enumeration's default unless it declares its own.
///
/// The members take no part in comparing two
/// [`SemanticType`](crate::semantic_type::SemanticType)s, so
/// `PartialEq` is always true. A type is identified by its `TypeId`
/// (ADR-0055); comparing representations compares their shapes, as it did
/// before the members were recorded.
#[derive(Debug, Clone, Default)]
pub struct EnumerationMembers {
    members: Vec<EnumerationMember>,
    /// Index in `members` of the default member.
    default: usize,
}

impl PartialEq for EnumerationMembers {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl EnumerationMembers {
    /// The members the value list `values` declares, numbered by
    /// [`resolve_ordinal_values`], with the default `default`. Without a
    /// default, or with one that is not a member (a semantic rule reports
    /// it), the default is the first member.
    pub fn from_values(values: &[EnumeratedValue], default: Option<&Id>) -> Self {
        let members = values
            .iter()
            .zip(resolve_ordinal_values(values))
            .map(|(value, ordinal)| EnumerationMember {
                name: value.value.clone(),
                ordinal,
            })
            .collect();
        let members = Self {
            members,
            default: 0,
        };
        match default {
            Some(default) => members.with_default(default),
            None => members,
        }
    }

    /// These members with the default `default`, as an alias that declares
    /// its own default has. Unchanged when `default` is not a member.
    pub fn with_default(self, default: &Id) -> Self {
        match self.members.iter().position(|m| m.name == *default) {
            Some(index) => Self {
                default: index,
                ..self
            },
            None => self,
        }
    }

    /// The members in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &EnumerationMember> {
        self.members.iter()
    }

    /// Whether `value` is a member.
    pub fn contains(&self, value: &Id) -> bool {
        self.ordinal_of(value).is_some()
    }

    /// The ordinal of the member `value`, `None` when it is not a member.
    pub fn ordinal_of(&self, value: &Id) -> Option<i64> {
        self.members
            .iter()
            .find(|m| m.name == *value)
            .map(|m| m.ordinal)
    }

    /// The member a variable of this type starts at when its declaration
    /// gives no initial value. `None` for an enumeration without members,
    /// which the grammar does not allow.
    pub fn default_member(&self) -> Option<&EnumerationMember> {
        self.members.get(self.default)
    }

    /// The ordinal a variable of this type starts at when its declaration
    /// gives no initial value: the default member's. 0 for an enumeration
    /// without members, which the grammar does not allow.
    pub fn default_ordinal(&self) -> i64 {
        self.members.get(self.default).map_or(0, |m| m.ordinal)
    }

    /// Whether `other` has the same members, with the same ordinals, in the
    /// same order: true for an enumeration and its aliases, and for two
    /// declarations that spell the same list.
    pub fn same_members(&self, other: &Self) -> bool {
        self.members.len() == other.members.len()
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|(a, b)| a.name == b.name && a.ordinal == b.ordinal)
    }
}

impl SemanticType {
    /// The members of an enumeration type; `None` for any other type.
    pub fn enumeration_members(&self) -> Option<&EnumerationMembers> {
        match self {
            SemanticType::Enumeration { members, .. } => Some(members),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::SignedInteger;
    use ironplc_dsl::core::SourceSpan;

    fn value(name: &str) -> EnumeratedValue {
        EnumeratedValue::new(name)
    }

    fn value_with(name: &str, explicit: i64) -> EnumeratedValue {
        EnumeratedValue {
            explicit_value: Some(
                SignedInteger::new(&explicit.to_string(), SourceSpan::default()).unwrap(),
            ),
            ..EnumeratedValue::new(name)
        }
    }

    fn ordinals(members: &EnumerationMembers) -> Vec<(String, i64)> {
        members
            .iter()
            .map(|m| (m.name.to_string(), m.ordinal))
            .collect()
    }

    #[test]
    fn from_values_when_implicit_values_then_numbers_from_zero() {
        let members = EnumerationMembers::from_values(&[value("RED"), value("GREEN")], None);

        assert_eq!(
            ordinals(&members),
            [("RED".to_string(), 0), ("GREEN".to_string(), 1)]
        );
    }

    #[test]
    fn from_values_when_explicit_values_then_uses_them() {
        let members = EnumerationMembers::from_values(
            &[value_with("X", 1), value_with("Y", 5), value("Z")],
            None,
        );

        assert_eq!(members.ordinal_of(&Id::from("x")), Some(1));
        assert_eq!(members.ordinal_of(&Id::from("Y")), Some(5));
        assert_eq!(members.ordinal_of(&Id::from("Z")), Some(6));
        assert_eq!(members.ordinal_of(&Id::from("W")), None);
    }

    #[test]
    fn from_values_when_no_default_then_default_is_first_member_value() {
        let members =
            EnumerationMembers::from_values(&[value_with("X", 1), value_with("Y", 5)], None);

        assert_eq!(members.default_ordinal(), 1);
    }

    #[test]
    fn from_values_when_default_then_default_is_its_value() {
        let members = EnumerationMembers::from_values(
            &[value("LOW"), value("HIGH")],
            Some(&Id::from("HIGH")),
        );

        assert_eq!(members.default_ordinal(), 1);
    }

    #[test]
    fn from_values_when_default_not_a_member_then_default_is_first_member() {
        let members = EnumerationMembers::from_values(
            &[value("LOW"), value("HIGH")],
            Some(&Id::from("MEDIUM")),
        );

        assert_eq!(members.default_ordinal(), 0);
    }

    #[test]
    fn with_default_when_member_then_changes_default_only() {
        let members = EnumerationMembers::from_values(&[value("A"), value("B")], None)
            .with_default(&Id::from("B"));

        assert_eq!(members.default_ordinal(), 1);
        assert_eq!(
            ordinals(&members),
            [("A".to_string(), 0), ("B".to_string(), 1)]
        );
    }

    #[test]
    fn same_members_when_same_list_then_true_else_false() {
        let a = EnumerationMembers::from_values(&[value("A"), value("B")], None);
        let same = EnumerationMembers::from_values(&[value("a"), value("b")], Some(&Id::from("B")));
        let other = EnumerationMembers::from_values(&[value("B"), value("A")], None);

        assert!(a.same_members(&same));
        assert!(!a.same_members(&other));
    }

    #[test]
    fn eq_when_members_differ_then_equal() {
        let a = EnumerationMembers::from_values(&[value("A")], None);
        let b = EnumerationMembers::from_values(&[value("B")], None);

        assert_eq!(a, b);
    }
}
