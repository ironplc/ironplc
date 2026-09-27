//! Enumerations: stored as `DINT` ordinals, as the bytecode stores them.

use std::collections::HashMap;

use ironplc_analyzer::resolve_ordinal_values;
use ironplc_dsl::common::{
    DataTypeDeclarationKind, EnumeratedValue, Library, LibraryElementKind, SpecificationKind,
};

/// The ordinals of the enumerations declared in a library.
pub(crate) struct Enums {
    /// Values of each type, in declaration order, with their ordinals.
    types: HashMap<String, Vec<(String, i64)>>,
    /// Default ordinal of each type.
    defaults: HashMap<String, i64>,
}

impl Enums {
    pub fn new(lib: &Library) -> Self {
        let mut types = HashMap::new();
        let mut defaults = HashMap::new();
        for e in &lib.elements {
            let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(d)) =
                e
            else {
                continue;
            };
            let SpecificationKind::Inline(spec) = &d.spec_init.spec else {
                continue;
            };
            let name = d.type_name.to_string().to_uppercase();
            let values: Vec<(String, i64)> = spec
                .values
                .iter()
                .zip(resolve_ordinal_values(&spec.values))
                .map(|(v, o)| (v.value.to_string().to_uppercase(), o))
                .collect();
            let default = d
                .spec_init
                .default
                .as_ref()
                .and_then(|v| ordinal_in(&values, &v.value.to_string()))
                .unwrap_or(0);
            defaults.insert(name.clone(), default);
            types.insert(name, values);
        }
        Enums { types, defaults }
    }

    /// The ordinal of a value, qualified or not.
    pub fn ordinal(&self, v: &EnumeratedValue) -> Option<i64> {
        let value = v.value.to_string();
        match &v.type_name {
            Some(t) => ordinal_in(self.types.get(&t.to_string().to_uppercase())?, &value),
            None => self.types.values().find_map(|vs| ordinal_in(vs, &value)),
        }
    }

    /// The values of a type, for the symbol map.
    pub fn values(&self, type_name: &str) -> Option<&Vec<(String, i64)>> {
        self.types.get(&type_name.to_uppercase())
    }

    pub fn default(&self, type_name: &str) -> i64 {
        self.defaults
            .get(&type_name.to_uppercase())
            .copied()
            .unwrap_or(0)
    }
}

fn ordinal_in(values: &[(String, i64)], name: &str) -> Option<i64> {
    values
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, o)| *o)
}
