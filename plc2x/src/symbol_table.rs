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

pub trait NodeData: Clone {}

pub trait Key: Eq + Hash + Clone {}

struct Scope<K: Key, T: NodeData> {
    table: HashMap<K, T>,
}

impl<K: Key, T: NodeData> Scope<K, T> {
    fn new() -> Self {
        Scope {
            table: HashMap::new(),
        }
    }

    fn add(&mut self, name: &K, value: T) {
        self.table.insert(name.clone(), value);
    }

    fn find(&mut self, name: &K) -> Option<&T> {
        self.table.get(name)
    }

    #[allow(unused)]
    fn remove(&mut self, name: &K) -> Option<T> {
        self.table.remove(name)
    }
}

pub struct SymbolTable<K: Key, V: NodeData> {
    stack: LinkedList<Scope<K, V>>,
}

impl<K: Key, V: NodeData> SymbolTable<K, V> {
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

    /// Adds the given name to the scope with the specified value.
    pub fn add(&mut self, name: &K, value: V) {
        match self.stack.front_mut() {
            None => {}
            Some(scope) => {
                scope.add(name, value);
            }
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
