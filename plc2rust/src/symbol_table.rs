use ironplc_dsl::visitor::Visit;

use crate::ironplc_dsl::dsl::*;
use std::collections::HashMap;
use std::collections::LinkedList;

pub trait NodeData: Clone {}

struct Scope<T: NodeData> {
    table: HashMap<String, T>,
}

impl<T: NodeData> Scope<T> {
    fn new() -> Self {
        Scope {
            table: HashMap::new(),
        }
    }

    fn add(&mut self, name: &str, value: T) {
        self.table.insert(name.to_string(), value);
    }

    fn find(&mut self, name: &str) -> Option<&T> {
        self.table.get(name)
    }

    fn remove(&mut self, name: &str) -> Option<T> {
        self.table.remove(name)
    }
}

struct SymbolTable<T: NodeData> {
    stack: LinkedList<Scope<T>>,
}

impl<T: NodeData> SymbolTable<T> {
    /// Creates an empty `SymbolTable`.
    fn new() -> Self {
        let mut stack = LinkedList::new();
        stack.push_back(Scope::new());
        return SymbolTable { stack: stack };
    }

    /// Enters a new scope.
    ///
    /// This creates a new context that can hide declarations
    /// from outer scopes.
    fn enter(&mut self) {
        self.stack.push_front(Scope::new())
    }

    /// Exits the current scope.
    ///
    /// This removes the current scope.
    fn exit(&mut self) {
        self.stack.pop_front();
    }

    /// Adds the given name to the scope with the specified value.
    fn add(&mut self, name: &str, value: T) {
        match self.stack.front_mut() {
            None => {}
            Some(scope) => {
                scope.add(name, value);
            }
        }
    }

    /// Returns the value for the given name.
    fn find(&mut self, name: &str) -> Option<&T> {
        self.stack.iter_mut().find_map(|scope| scope.find(name))
    }

    /// Removes the name from the inner-most scope if
    /// the name is in the scope.
    ///
    /// Returns the value or `None` if value is not in
    /// the inner-most scope.
    fn remove(&mut self, name: &str) -> Option<T> {
        match self.stack.front_mut() {
            None => None,
            Some(scope) => scope.remove(name),
        }
    }
}

pub fn from(lib: &Library) -> HashMap<String, TypeDefinitionKind> {
    let mut type_map = HashMap::new();
    let mut visitor = TypeDefinitionFinder { types: &mut type_map };
    visitor.walk(lib);
    return type_map;
}

// Finds types that are valid as variable types. These include enumerations,
// function blocks, functions, structures.
struct TypeDefinitionFinder<'a> {
    types: &'a mut HashMap<String, TypeDefinitionKind>,
}
impl<'a> Visit for TypeDefinitionFinder<'a> {
    fn visit_enum_declaration(&mut self, enum_decl: &EnumerationDeclaration) {
        self.types
            .insert(enum_decl.name.clone(), TypeDefinitionKind::Enumeration);
        
    }
}
