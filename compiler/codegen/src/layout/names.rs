//! The names the layout knows, each recorded once, in the scope that
//! declares it, and the one lookup that says which of them a body sees.
//!
//! A scope is the globals, the program, or one body: a function's, a
//! function block's or a method's. The layout records each of a name's
//! entries (its slot, its type, its string, array, structure or instance)
//! under the scope whose declaration lays it out.
//!
//! A body sees its own names first; a method then its block's. Of the names
//! the globals and the program declare, a function or a block sees only
//! some, and which depends on the kind of entry ([`Reach`]): for example, a
//! global array is not visible in a function. These rules are the ones the
//! layout has always applied, kept as they are because changing one would
//! change the compiled program.

use std::collections::HashMap;

use ironplc_container::{FunctionId, VarIndex};
use ironplc_dsl::core::Id;

/// A scope that declares names the layout knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ScopeId {
    /// The globals, the compiler-provided ones included.
    Global,
    /// The program's own variables.
    Program,
    /// A user function's parameters, locals, outputs and result.
    Function(FunctionId),
    /// A function block's copy slots, one for each field.
    Block(FunctionId),
    /// The parameters, locals, outputs and result of the method `method`
    /// of the block whose body is `block`.
    Method {
        block: FunctionId,
        method: FunctionId,
    },
}

/// One kind of entry the layout records for a name, under the scope that
/// declares the name.
pub(crate) struct Scoped<V> {
    by_scope: HashMap<ScopeId, HashMap<Id, V>>,
}

impl<V> Default for Scoped<V> {
    fn default() -> Self {
        Self {
            by_scope: HashMap::new(),
        }
    }
}

impl<V> Scoped<V> {
    /// Records `value` for `name`, which `scope` declares, in place of any
    /// entry `scope` already had for it.
    pub(crate) fn insert(&mut self, scope: ScopeId, name: Id, value: V) {
        self.by_scope.entry(scope).or_default().insert(name, value);
    }

    /// The entry `scope` itself records for `name`.
    fn own(&self, scope: ScopeId, name: &Id) -> Option<&V> {
        self.by_scope.get(&scope)?.get(name)
    }

    /// Whether `scope` itself records an entry for `name`.
    pub(crate) fn declares(&self, scope: ScopeId, name: &Id) -> bool {
        self.own(scope, name).is_some()
    }

    /// The entries `f` makes of each of `self`'s, under the same scope and
    /// name.
    pub(crate) fn map<W>(&self, f: impl Fn(&V) -> W) -> Scoped<W> {
        Scoped {
            by_scope: self
                .by_scope
                .iter()
                .map(|(scope, names)| {
                    let names = names
                        .iter()
                        .map(|(name, value)| (name.clone(), f(value)))
                        .collect();
                    (*scope, names)
                })
                .collect(),
        }
    }

    /// Each entry one of `self` and `other` records that the other does
    /// not record the same: the scope, the name, and each one's entry.
    pub(crate) fn differences(&self, other: &Self) -> Vec<String>
    where
        V: PartialEq + std::fmt::Debug,
    {
        let mut differences = Vec::new();
        let scopes = self.by_scope.keys().chain(
            other
                .by_scope
                .keys()
                .filter(|scope| !self.by_scope.contains_key(scope)),
        );
        for scope in scopes {
            let names = self.names(*scope).chain(
                other
                    .names(*scope)
                    .filter(|name| !self.declares(*scope, name)),
            );
            for name in names {
                let (mine, theirs) = (self.own(*scope, name), other.own(*scope, name));
                if mine != theirs {
                    differences.push(format!("{scope:?} {name}: {mine:?} and {theirs:?}"));
                }
            }
        }
        differences.sort();
        differences
    }

    /// The names `scope` itself records an entry for.
    fn names(&self, scope: ScopeId) -> impl Iterator<Item = &Id> {
        self.by_scope
            .get(&scope)
            .into_iter()
            .flat_map(HashMap::keys)
    }
}

/// Which of the names the globals and the program declare a body other
/// than the program sees, for one kind of entry.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reach {
    /// What the body of a user function sees.
    function: Seen,
    /// What the body of a function block, or of one of its methods, sees.
    block: Seen,
}

/// Which of the names the globals and the program declare a body sees.
#[derive(Clone, Copy, Debug)]
enum Seen {
    /// None of them.
    Nothing,
    /// The globals, except one whose name the program declares a variable
    /// of too.
    Globals,
    /// Every one, the program's in place of a global of the same name.
    Program,
}

/// The slots and what is known of each slot's variable: its type, string,
/// structure, array of structures, the strings a reference to strings
/// reaches, or the descriptor of the array or structure a reference
/// reaches. A function or a block sees the globals'.
pub(crate) const SLOTS: Reach = Reach {
    function: Seen::Globals,
    block: Seen::Globals,
};

/// The arrays. A function or a block sees none of the globals' or the
/// program's.
pub(crate) const ARRAYS: Reach = Reach {
    function: Seen::Nothing,
    block: Seen::Nothing,
};

/// The function block instances. A function sees every one the globals
/// and the program declare; a block sees none of them.
pub(crate) const INSTANCES: Reach = Reach {
    function: Seen::Program,
    block: Seen::Nothing,
};

/// The `VAR_IN_OUT` parameters. A function sees none of the program's; a
/// block sees every one.
pub(crate) const IN_OUTS: Reach = Reach {
    function: Seen::Nothing,
    block: Seen::Program,
};

/// The entry of `map` for `name` that the body of `scope` sees: its own;
/// for a method, then its block's; then the one of the globals or the
/// program that `reach` lets it see. `slots` is the layout's slots, which
/// say which names the globals and the program declare.
pub(crate) fn lookup<'m, V>(
    map: &'m Scoped<V>,
    slots: &Scoped<VarIndex>,
    reach: Reach,
    scope: ScopeId,
    name: &Id,
) -> Option<&'m V> {
    match scope {
        ScopeId::Global => map.own(ScopeId::Global, name),
        ScopeId::Program => map
            .own(ScopeId::Program, name)
            .or_else(|| map.own(ScopeId::Global, name)),
        ScopeId::Function(_) => map
            .own(scope, name)
            .or_else(|| outer(map, slots, reach.function, name)),
        ScopeId::Block(_) => map
            .own(scope, name)
            .or_else(|| outer(map, slots, reach.block, name)),
        ScopeId::Method { block, .. } => map
            .own(scope, name)
            .or_else(|| map.own(ScopeId::Block(block), name))
            .or_else(|| outer(map, slots, reach.block, name)),
    }
}

/// The entry of `map` for `name`, of the globals or the program, that a
/// body which sees `seen` of them sees.
fn outer<'m, V>(
    map: &'m Scoped<V>,
    slots: &Scoped<VarIndex>,
    seen: Seen,
    name: &Id,
) -> Option<&'m V> {
    match seen {
        Seen::Nothing => None,
        Seen::Globals => {
            if is_visible_global(slots, name) {
                map.own(ScopeId::Global, name)
            } else {
                None
            }
        }
        Seen::Program => map
            .own(ScopeId::Program, name)
            .or_else(|| map.own(ScopeId::Global, name)),
    }
}

/// Whether `name` is a global that a function or a block sees: one the
/// program declares no variable of.
fn is_visible_global(slots: &Scoped<VarIndex>, name: &Id) -> bool {
    slots.declares(ScopeId::Global, name) && !slots.declares(ScopeId::Program, name)
}

/// The slot after the highest one recorded under `scope` or a scope its
/// body sees, which is where a scratch slot laid out in `scope` is numbered
/// from.
///
/// The body being laid out holds the highest of those slots: the globals,
/// and a method's block fields, are below it. So this is the next free slot
/// of that body's run. It is not [`slot_count`], which for a function
/// counts the globals and the function's own slots but not the program's
/// slots between them.
pub(crate) fn next_free_slot(slots: &Scoped<VarIndex>, scope: ScopeId) -> u16 {
    let seen = match scope {
        ScopeId::Global => vec![ScopeId::Global],
        ScopeId::Program | ScopeId::Function(_) | ScopeId::Block(_) => {
            vec![scope, ScopeId::Global]
        }
        ScopeId::Method { block, .. } => vec![scope, ScopeId::Block(block), ScopeId::Global],
    };
    seen.iter()
        .filter_map(|scope| slots.by_scope.get(scope))
        .flat_map(HashMap::values)
        .map(|index| index.raw() + 1)
        .max()
        .unwrap_or(0)
}

/// How many names have a slot that the body of `scope` sees, which is
/// where the next variable of the globals or the program is numbered from.
pub(crate) fn slot_count(slots: &Scoped<VarIndex>, scope: ScopeId) -> usize {
    let globals = || {
        slots
            .names(ScopeId::Global)
            .filter(|name| is_visible_global(slots, name))
    };
    match scope {
        ScopeId::Global => slots.names(ScopeId::Global).count(),
        ScopeId::Program => {
            slots.names(ScopeId::Program).count()
                + slots
                    .names(ScopeId::Global)
                    .filter(|name| !slots.declares(ScopeId::Program, name))
                    .count()
        }
        ScopeId::Function(_) | ScopeId::Block(_) => {
            slots.names(scope).count()
                + globals()
                    .filter(|name| !slots.declares(scope, name))
                    .count()
        }
        ScopeId::Method { block, .. } => {
            let block = ScopeId::Block(block);
            slots.names(scope).count()
                + slots
                    .names(block)
                    .filter(|name| !slots.declares(scope, name))
                    .count()
                + globals()
                    .filter(|name| !slots.declares(scope, name) && !slots.declares(block, name))
                    .count()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBAL: &str = "global";
    const LAST_GLOBAL: &str = "last_global";
    const SHADOWED: &str = "shadowed";
    const LOCAL: &str = "local";
    const ORPHAN: &str = "orphan";

    const FUNCTION: ScopeId = ScopeId::Function(FunctionId::new(5));
    const BLOCK: ScopeId = ScopeId::Block(FunctionId::new(2));
    const METHOD: ScopeId = ScopeId::Method {
        block: FunctionId::new(2),
        method: FunctionId::new(7),
    };
    const OTHER_METHOD: ScopeId = ScopeId::Method {
        block: FunctionId::new(2),
        method: FunctionId::new(8),
    };

    fn id(name: &str) -> Id {
        Id::from(name)
    }

    /// The slots of three globals and of the program's two variables, one
    /// of which has the name of the third global.
    fn slots() -> Scoped<VarIndex> {
        let mut slots = Scoped::default();
        slots.insert(ScopeId::Global, id(GLOBAL), VarIndex::new(0));
        slots.insert(ScopeId::Global, id(LAST_GLOBAL), VarIndex::new(1));
        slots.insert(ScopeId::Global, id(SHADOWED), VarIndex::new(2));
        slots.insert(ScopeId::Program, id(SHADOWED), VarIndex::new(3));
        slots.insert(ScopeId::Program, id(LOCAL), VarIndex::new(4));
        slots
    }

    /// An entry for every name, each recording the scope it is under: the
    /// globals' for the globals, the program's for the program's
    /// variables and for a name that has no slot.
    fn entries() -> Scoped<&'static str> {
        let mut entries = Scoped::default();
        for name in [GLOBAL, LAST_GLOBAL, SHADOWED] {
            entries.insert(ScopeId::Global, id(name), "global");
        }
        for name in [SHADOWED, LOCAL, ORPHAN] {
            entries.insert(ScopeId::Program, id(name), "program");
        }
        entries
    }

    fn seen(reach: Reach, scope: ScopeId, name: &str) -> Option<&'static str> {
        lookup(&entries(), &slots(), reach, scope, &id(name)).copied()
    }

    #[test]
    fn lookup_when_function_or_block_reads_slot_then_sees_only_globals() {
        for scope in [FUNCTION, BLOCK, METHOD] {
            assert_eq!(seen(SLOTS, scope, GLOBAL), Some("global"));
            assert_eq!(seen(SLOTS, scope, LAST_GLOBAL), Some("global"));
            assert_eq!(seen(SLOTS, scope, LOCAL), None);
            assert_eq!(seen(SLOTS, scope, ORPHAN), None);
        }
    }

    #[test]
    fn lookup_when_program_declares_name_of_global_then_function_or_block_sees_neither() {
        for scope in [FUNCTION, BLOCK, METHOD] {
            assert_eq!(seen(SLOTS, scope, SHADOWED), None);
        }
    }

    #[test]
    fn lookup_when_program_reads_name_then_program_entry_before_global() {
        assert_eq!(seen(SLOTS, ScopeId::Program, SHADOWED), Some("program"));
        assert_eq!(seen(SLOTS, ScopeId::Program, GLOBAL), Some("global"));
        assert_eq!(seen(SLOTS, ScopeId::Global, LOCAL), None);
    }

    #[test]
    fn lookup_when_function_or_block_reads_array_then_sees_none_of_programs() {
        for scope in [FUNCTION, BLOCK, METHOD] {
            assert_eq!(seen(ARRAYS, scope, GLOBAL), None);
            assert_eq!(seen(ARRAYS, scope, LOCAL), None);
        }
    }

    #[test]
    fn lookup_when_function_reads_instance_then_sees_every_one_of_programs() {
        assert_eq!(seen(INSTANCES, FUNCTION, GLOBAL), Some("global"));
        assert_eq!(seen(INSTANCES, FUNCTION, LOCAL), Some("program"));
        assert_eq!(seen(INSTANCES, FUNCTION, SHADOWED), Some("program"));
        assert_eq!(seen(INSTANCES, FUNCTION, ORPHAN), Some("program"));
    }

    #[test]
    fn lookup_when_block_reads_instance_then_sees_none_of_programs() {
        for scope in [BLOCK, METHOD] {
            assert_eq!(seen(INSTANCES, scope, GLOBAL), None);
            assert_eq!(seen(INSTANCES, scope, LOCAL), None);
        }
    }

    #[test]
    fn lookup_when_function_reads_in_out_then_sees_none_of_programs() {
        assert_eq!(seen(IN_OUTS, FUNCTION, GLOBAL), None);
        assert_eq!(seen(IN_OUTS, FUNCTION, LOCAL), None);
    }

    #[test]
    fn lookup_when_block_reads_in_out_then_sees_every_one_of_programs() {
        for scope in [BLOCK, METHOD] {
            assert_eq!(seen(IN_OUTS, scope, GLOBAL), Some("global"));
            assert_eq!(seen(IN_OUTS, scope, LOCAL), Some("program"));
            assert_eq!(seen(IN_OUTS, scope, ORPHAN), Some("program"));
        }
    }

    #[test]
    fn lookup_when_body_declares_name_then_own_entry_before_any_other() {
        let mut map = entries();
        let mut slots = slots();
        for scope in [FUNCTION, BLOCK, METHOD] {
            map.insert(scope, id(GLOBAL), "own");
            slots.insert(scope, id(GLOBAL), VarIndex::new(9));
        }
        for scope in [FUNCTION, BLOCK, METHOD] {
            for reach in [SLOTS, ARRAYS, INSTANCES, IN_OUTS] {
                assert_eq!(
                    lookup(&map, &slots, reach, scope, &id(GLOBAL)),
                    Some(&"own")
                );
            }
        }
    }

    #[test]
    fn lookup_when_method_reads_name_then_sees_its_blocks_but_not_another_methods() {
        let mut map = entries();
        map.insert(BLOCK, id("field"), "block");
        map.insert(OTHER_METHOD, id("param"), "other method");

        let found = |name| lookup(&map, &slots(), SLOTS, METHOD, &id(name)).copied();

        assert_eq!(found("field"), Some("block"));
        assert_eq!(found("param"), None);
    }

    #[test]
    fn slot_count_when_scope_is_program_then_counts_globals_and_program_once_each() {
        assert_eq!(slot_count(&slots(), ScopeId::Global), 3);
        assert_eq!(slot_count(&slots(), ScopeId::Program), 4);
    }

    #[test]
    fn slot_count_when_scope_is_body_then_counts_own_and_visible_globals_once_each() {
        let mut slots = slots();
        slots.insert(FUNCTION, id("param"), VarIndex::new(5));
        slots.insert(FUNCTION, id(GLOBAL), VarIndex::new(6));
        slots.insert(BLOCK, id("field"), VarIndex::new(7));
        slots.insert(METHOD, id("field"), VarIndex::new(8));
        slots.insert(METHOD, id("result"), VarIndex::new(9));

        // The function's two, and `last_global`: `global` is its own, and
        // the program declares `shadowed`.
        assert_eq!(slot_count(&slots, FUNCTION), 3);
        // The block's field, and the two visible globals.
        assert_eq!(slot_count(&slots, BLOCK), 3);
        // The method's two, of which `field` hides the block's, and the two
        // visible globals.
        assert_eq!(slot_count(&slots, METHOD), 4);
    }

    #[test]
    fn next_free_slot_when_no_slot_then_first_slot() {
        assert_eq!(next_free_slot(&Scoped::default(), ScopeId::Global), 0);
    }

    #[test]
    fn next_free_slot_when_scope_is_program_then_after_program_slots() {
        assert_eq!(next_free_slot(&slots(), ScopeId::Program), 5);
    }

    #[test]
    fn next_free_slot_when_scope_is_function_then_after_its_own_slots_not_name_count() {
        let mut slots = slots();
        slots.insert(FUNCTION, id("param"), VarIndex::new(5));
        slots.insert(FUNCTION, id("result"), VarIndex::new(6));

        // Past the function's own slots, which follow the program's;
        // `slot_count` is 4, the program's `local` slot.
        assert_eq!(next_free_slot(&slots, FUNCTION), 7);
    }

    #[test]
    fn next_free_slot_when_scope_is_method_then_after_its_own_slots_not_another_methods() {
        let mut slots = slots();
        slots.insert(BLOCK, id("field"), VarIndex::new(5));
        slots.insert(METHOD, id("param"), VarIndex::new(6));
        slots.insert(OTHER_METHOD, id("param"), VarIndex::new(9));

        assert_eq!(next_free_slot(&slots, METHOD), 7);
    }

    #[test]
    fn differences_when_same_entries_then_none() {
        assert!(slots().differences(&slots()).is_empty());
    }

    #[test]
    fn map_when_entries_then_each_mapped_under_same_scope_and_name() {
        let mapped = slots().map(|index| index.raw() + 10);

        let mut expected = Scoped::default();
        expected.insert(ScopeId::Global, id(GLOBAL), 10);
        expected.insert(ScopeId::Global, id(LAST_GLOBAL), 11);
        expected.insert(ScopeId::Global, id(SHADOWED), 12);
        expected.insert(ScopeId::Program, id(SHADOWED), 13);
        expected.insert(ScopeId::Program, id(LOCAL), 14);
        assert!(mapped.differences(&expected).is_empty());
    }

    #[test]
    fn differences_when_entry_differs_or_missing_then_names_each() {
        let mut other = slots();
        other.insert(ScopeId::Program, id(LOCAL), VarIndex::new(5));
        other.insert(FUNCTION, id(ORPHAN), VarIndex::new(6));

        assert_eq!(
            slots().differences(&other),
            [
                "Function(FunctionId(5)) orphan: None and Some(VarIndex(6))",
                "Program local: Some(VarIndex(4)) and Some(VarIndex(5))",
            ]
        );
    }
}
