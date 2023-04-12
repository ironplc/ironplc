//! A table for symbols. The table maintains contexts and a mapping
//! of a string to data for each item in the context.
//!
//! The typical way to use the symbol table is to implement Visitor
//! for the table. Then use context functions (enter, exit) based on
//! visited objects that delineate context and use tem functions (add,
//! remove) as individual items go into and out of definition.
//!
//! # Example
//!
//! ```ignore
//! use crate::symbol_table::{self, NodeData, SymbolTable};
//! use ironplc_dsl::dsl::Library;
//! use ironplc_dsl::visitor::Visitor;
//!
//! // The value in the symbol table. In this example, the value
//! // has no additional associated data, but you must still define
//! // data items.
//! struct DummyData {}
//! impl NodeData for DummyData {}
//!
//! impl Visitor<String> for SymbolTable<DummyData> {}
//!
//! fn uses_symbol_table(lib: &Library) {
//!    let mut visitor: SymbolTable<DummyData> = symbol_table::SymbolTable::new();
//!     visitor.walk(&lib);
//! }
//! ```
use std::collections::HashMap;
use std::collections::LinkedList;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait Key: Eq + Hash + Clone {}

struct Scope<'a, K: Key, V: 'a> {
    table: HashMap<K, V>,
    phantom: PhantomData<&'a V>,
}

impl<'a, K: Key, V: 'a> Scope<'a, K, V> {
    fn new() -> Self {
        Scope {
            table: HashMap::new(),
            phantom: PhantomData,
        }
    }

    fn add(&mut self, name: &K, value: V) -> Option<V> {
        self.table.insert(name.clone(), value)
    }

    /// Tries to add the name into the scope with the specified value.
    ///
    /// If the scope does not have this name, adds the name with the value.
    ///
    /// If the scope does have this name, then value is not updated. The
    /// existing key and value are returned.
    fn try_add(&mut self, name: &K, value: V) -> Option<(&K, &V)> {
        // We want the map to be unmodified if the key already exists, so we
        // must first test if the key exists.
        if !self.table.contains_key(name) {
            self.table.insert(name.clone(), value);
            None
        } else {
            let existing = self.table.get_key_value(name).unwrap();
            Some(existing)
        }
    }

    fn find(&mut self, name: &K) -> Option<&V> {
        self.table.get(name)
    }

    #[allow(unused)]
    fn remove(&mut self, name: &K) -> Option<V> {
        self.table.remove(name)
    }
}

pub struct SymbolTable<'a, K: Key, V: 'a> {
    stack: LinkedList<Scope<'a, K, V>>,
}

impl<'a, K: Key, V: 'a> SymbolTable<'a, K, V> {
    /// Creates an empty `SymbolTable`.
    pub fn new() -> Self {
        let mut stack = LinkedList::new();
        stack.push_back(Scope::new());
        SymbolTable { stack }
    }

    /// Enters a new scope.
    ///
    /// This creates a new context that can hide declarations
    /// from outer scopes.
    pub fn enter(&mut self) {
        self.stack.push_front(Scope::new())
    }

    /// Exits the current scope.
    ///
    /// This removes the current scope.
    pub fn exit(&mut self) {
        self.stack.pop_front();
    }

    /// Adds the key to the scope with the specified value.
    ///
    /// If the table does not have this key present in scope, None is returned.
    ///
    /// If the table does have this key present in scope, the value is updated,
    /// and the old value is returned. The key is not updated. This matters
    /// particularly for Id's which can be equal even if not identical.
    pub fn add(&mut self, name: &K, value: V) -> Option<V> {
        match self.stack.front_mut() {
            None => None,
            Some(scope) => scope.add(name, value),
        }
    }

    /// Tries to add the key to the scope with the specified value.
    ///
    /// If the table does not have this key present in scope, None is returned.
    ///
    /// If the table does have this key present in scope, the value is not
    /// updated. The existing key and value are returned.
    pub fn try_add(&mut self, name: &K, value: V) -> Option<(&K, &V)> {
        match self.stack.front_mut() {
            None => None,
            Some(scope) => scope.try_add(name, value),
        }
    }

    /// Returns the value for the given name.
    pub fn find(&mut self, name: &K) -> Option<&V> {
        self.stack.iter_mut().find_map(|scope| scope.find(name))
    }

    /// Removes the name from the inner-most scope if
    /// the name is in the scope.
    ///
    /// Returns the value or `None` if value is not in
    /// the inner-most scope.
    #[allow(unused)]
    pub fn remove(&mut self, name: &K) -> Option<V> {
        match self.stack.front_mut() {
            None => None,
            Some(scope) => scope.remove(name),
        }
    }
}
