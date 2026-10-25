//! What the layout records of each variable it gives storage to, by the
//! lowered id that names it ([`Stored`]), each under the scope whose
//! declaration lays it out.
//!
//! A scope is the globals, the program, or one body: a function's, a
//! function block's or a method's. The layout records each of a variable's
//! entries (its slot, its type, its string, array, structure or instance)
//! by its id, so a body reaches what it names by that id alone (REQ-LOW-codegen-123).

use std::collections::HashMap;

use ironplc_ir::lowered::PouId;

use crate::access::Stored;

/// A scope that declares variables the layout gives storage to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum ScopeId {
    /// The globals, the compiler-provided ones included.
    #[default]
    Global,
    /// The variables of the lowered POU whose body is compiled in it: the
    /// program's, a function's or a method's own variables, or a function
    /// block's copy slots, one for each field.
    Pou(PouId),
}

/// One kind of entry the layout records for each variable, with the scope
/// that declares the variable.
pub(crate) struct Scoped<V> {
    entries: HashMap<Stored, (ScopeId, V)>,
}

impl<V> Default for Scoped<V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<V> Scoped<V> {
    /// Records `value` for `stored`, which `scope` declares, in place of any
    /// entry already recorded for it.
    pub(crate) fn insert(&mut self, scope: ScopeId, stored: Stored, value: V) {
        self.entries.insert(stored, (scope, value));
    }

    /// The entry recorded for `stored`.
    pub(crate) fn get(&self, stored: &Stored) -> Option<&V> {
        self.entries.get(stored).map(|(_, value)| value)
    }

    /// Each entry, with the scope that declares its variable.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (ScopeId, &V)> {
        self.entries.values().map(|(scope, value)| (*scope, value))
    }
}
