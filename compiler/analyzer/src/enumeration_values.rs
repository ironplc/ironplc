//! The values each enumeration type declares.
//!
//! The symbol environment keys symbols by name, so it can hold only one
//! enumeration type per value name. Two enumerations may declare the same
//! value name (`A : (X, Y); B : (X, Z)`), so the values of each type are kept
//! here, per type, instead of being derived from the name-keyed table.
//!
//! An alias (`B : A;`) declares no values of its own: it has the values of
//! the enumeration it names, which a lookup finds by following the alias
//! links. Aliases are recorded from their declarations, never inferred from
//! two types having an equal representation, because every enumeration with
//! the same underlying type has an equal representation.

use indexmap::IndexMap;
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;

/// Values per enumeration type, and alias links between enumeration types.
///
/// Both maps keep insertion order, so the values of a type are returned in
/// declaration order, run after run.
#[derive(Debug, Default)]
pub struct EnumerationValues {
    /// The values an enumeration declaration lists, in declaration order.
    declared: IndexMap<TypeName, Vec<Id>>,
    /// Each enumeration alias, to the type it names.
    aliases: IndexMap<TypeName, TypeName>,
}

impl EnumerationValues {
    /// Records that the enumeration `enum_type` declares `value`.
    pub fn insert(&mut self, enum_type: &TypeName, value: &Id) {
        self.declared
            .entry(enum_type.clone())
            .or_default()
            .push(value.clone());
    }

    /// Records that `alias` is declared as an alias of `base`.
    pub fn insert_alias(&mut self, alias: &TypeName, base: &TypeName) {
        self.aliases.insert(alias.clone(), base.clone());
    }

    /// The values of `enum_type`: those its declaration lists or, for an
    /// alias, those of the enumeration the alias chain ends at. Empty for a
    /// type that is not an enumeration, and for an alias chain that loops.
    pub fn values_of(&self, enum_type: &TypeName) -> Vec<&Id> {
        let mut current = enum_type;
        // A chain visits each alias at most once, so a longer walk is a loop.
        for _ in 0..=self.aliases.len() {
            match self.aliases.get(current) {
                Some(base) => current = base,
                None => {
                    return self
                        .declared
                        .get(current)
                        .map(|values| values.iter().collect())
                        .unwrap_or_default();
                }
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(values: Vec<&Id>) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn values_of_when_types_share_value_name_then_each_keeps_it() {
        let mut env = EnumerationValues::default();
        env.insert(&TypeName::from("A"), &Id::from("X"));
        env.insert(&TypeName::from("A"), &Id::from("Y"));
        env.insert(&TypeName::from("B"), &Id::from("X"));
        env.insert(&TypeName::from("B"), &Id::from("Z"));

        assert_eq!(names(env.values_of(&TypeName::from("A"))), ["X", "Y"]);
        assert_eq!(names(env.values_of(&TypeName::from("B"))), ["X", "Z"]);
    }

    #[test]
    fn values_of_when_alias_chain_then_values_of_declaring_type() {
        let mut env = EnumerationValues::default();
        // The alias links are recorded before the values, as a reminder
        // that a lookup does not depend on the order of the records.
        env.insert_alias(&TypeName::from("L2"), &TypeName::from("L1"));
        env.insert_alias(&TypeName::from("L1"), &TypeName::from("L"));
        env.insert(&TypeName::from("L"), &Id::from("INFO"));

        assert_eq!(names(env.values_of(&TypeName::from("L2"))), ["INFO"]);
        assert_eq!(names(env.values_of(&TypeName::from("L"))), ["INFO"]);
    }

    #[test]
    fn values_of_when_alias_loop_then_empty() {
        let mut env = EnumerationValues::default();
        env.insert_alias(&TypeName::from("A"), &TypeName::from("B"));
        env.insert_alias(&TypeName::from("B"), &TypeName::from("A"));

        assert!(env.values_of(&TypeName::from("A")).is_empty());
    }

    #[test]
    fn values_of_when_type_unknown_then_empty() {
        let env = EnumerationValues::default();

        assert!(env.values_of(&TypeName::from("A")).is_empty());
    }
}
