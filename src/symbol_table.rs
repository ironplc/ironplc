use crate::ironplc_dsl::dsl::*;
use crate::ironplc_dsl::visitor::{walk_library, LibraryVisitor};
use std::collections::HashMap;
use std::collections::LinkedList;

pub trait NodeData: Clone {}

struct Scope<T: NodeData> {
    table: HashMap<String, T>
}

impl<T: NodeData> Scope<T> {
    fn new() -> Self {
        Scope { table: HashMap::new() }
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
    stack: LinkedList<Scope<T>>
}

impl<T: NodeData> SymbolTable<T> {
    /// Creates an empty `SymbolTable`.
    ///
    /// # Examples
    /// 
    /// ```
    /// let table: SymbolTable<bool> = SymbolTable::new();
    /// ```
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
            None => {},
            Some(scope) => { scope.add(name, value); }
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
            Some(scope) => scope.remove(name)
        }
    }
}


pub fn from(lib: &Library) -> HashMap<String, TypeDefinitionKind>{
    let type_map = HashMap::new();
    let mut visitor = TypeDefinitionFinder { types: type_map };
    walk_library(&mut visitor, lib);
    return type_map;
}



// Finds types that are valid as variable types. These include enumerations,
// function blocks, functions, structures.
struct TypeDefinitionFinder {
    types: HashMap<String, TypeDefinitionKind>,
}
impl ironplc_dsl::visitor::LibraryVisitor<()> for TypeDefinitionFinder {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) {}
    fn visit_data_type_declaration(&mut self, dts: &Vec<EnumerationDeclaration>) {
        for dt in dts {
            self.types
                .insert(dt.name.clone(), TypeDefinitionKind::Enumeration);
        }
    }
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) {}
    fn visit_function_block_declaration(&mut self, l: &FunctionBlockDeclaration) {}
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) {}
}

struct LateBoundTypeResolver {
    types: HashMap<String, String>,
}
impl ironplc_dsl::visitor::LibraryVisitor<()> for LateBoundTypeResolver {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) {}
    fn visit_data_type_declaration(&mut self, dts: &Vec<EnumerationDeclaration>) {}
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) {}
    fn visit_function_block_declaration(&mut self, fb: &FunctionBlockDeclaration) {
        for var_decl in &fb.var_decls {
            match var_decl {
                VarInitKind::LocatedVarInit(located) => {
                    if let TypeInitializer::LateResolvedType(type_name) = &located.initializer {
                        let type_kind = self.types.get(type_name);
                        /*located.initializer = TypeInitializer::FunctionBlock{
                            type_name: type_name.to_string(),
                        };*/
                    }
                }
                VarInitKind::VarInit(var) => {
                    if let Some(TypeInitializer::LateResolvedType(tn)) = &var.initializer {}
                }
            }
        }
    }
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) {}
}
