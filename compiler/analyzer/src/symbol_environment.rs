use std::collections::HashMap;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;

/// Represents the kind of scope a symbol belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Global scope (library level)
    Global,
    /// Named scope (function, function block, program, etc.)
    Named(Id),
}

/// Represents the kind of symbol
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    /// Variable declaration
    Variable,
    /// Function parameter (input)
    Parameter,
    /// Function parameter (output)
    OutputParameter,
    /// Function parameter (input/output)
    InOutParameter,
    /// Function declaration
    Function,
    /// Function block declaration
    FunctionBlock,
    /// Program declaration
    Program,
    /// Type declaration
    Type,
    /// Constant declaration
    Constant,
    /// Enumeration value
    EnumerationValue,
    /// Structure element
    StructureElement,
    /// Edge variable (rising/falling edge)
    EdgeVariable,
}

/// Metadata associated with a symbol
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// The kind of symbol
    pub kind: SymbolKind,
    /// The scope where this symbol is declared
    pub scope: ScopeKind,
    /// The scope where this symbol is visible (for scoping rules)
    pub visibility_scope: ScopeKind,
    /// Whether this symbol is a reference to an external declaration
    pub is_external: bool,
    /// The data type of the symbol (if applicable)
    pub data_type: Option<String>,
    /// Source location information
    pub span: ironplc_dsl::core::SourceSpan,
}

impl SymbolInfo {
    pub fn new(kind: SymbolKind, scope: ScopeKind, span: ironplc_dsl::core::SourceSpan) -> Self {
        Self {
            kind,
            scope: scope.clone(),
            visibility_scope: scope,
            is_external: false,
            data_type: None,
            span,
        }
    }

    pub fn with_data_type(mut self, data_type: String) -> Self {
        self.data_type = Some(data_type);
        self
    }

    pub fn with_external(mut self, is_external: bool) -> Self {
        self.is_external = is_external;
        self
    }

    pub fn with_visibility_scope(mut self, visibility_scope: ScopeKind) -> Self {
        self.visibility_scope = visibility_scope;
        self
    }
}

/// The main symbol environment that tracks all symbols across the library
pub struct SymbolEnvironment {
    /// Global symbols (types, functions, function blocks, programs)
    global_symbols: HashMap<Id, SymbolInfo>,
    /// Scoped symbols (variables within functions, function blocks, etc.)
    scoped_symbols: HashMap<ScopeKind, HashMap<Id, SymbolInfo>>,
    /// Symbol resolution cache for performance
    resolution_cache: HashMap<(Id, ScopeKind), Option<&'static SymbolInfo>>,
}

impl SymbolEnvironment {
    pub fn new() -> Self {
        Self {
            global_symbols: HashMap::new(),
            scoped_symbols: HashMap::new(),
            resolution_cache: HashMap::new(),
        }
    }

    /// Insert a symbol into the environment
    pub fn insert(
        &mut self,
        name: &Id,
        kind: SymbolKind,
        scope: &ScopeKind,
    ) -> Result<(), Diagnostic> {
        let symbol_info = SymbolInfo::new(kind, scope.clone(), name.span());
        
        match scope {
            ScopeKind::Global => {
                // Check for duplicate global symbols
                if let Some(_existing) = self.global_symbols.get(name) {
                    // For now, allow redefinition (this might be needed for forward declarations)
                    // TODO: Implement proper duplicate detection
                }
                self.global_symbols.insert(name.clone(), symbol_info);
            }
            ScopeKind::Named(scope_name) => {
                let scope_symbols = self.scoped_symbols
                    .entry(ScopeKind::Named(scope_name.clone()))
                    .or_insert_with(HashMap::new);
                
                // Check for duplicate symbols in the same scope
                if let Some(_existing) = scope_symbols.get(name) {
                    // For now, allow redefinition (this might be needed for forward declarations)
                    // TODO: Implement proper duplicate detection
                }
                
                scope_symbols.insert(name.clone(), symbol_info);
            }
        }
        
        Ok(())
    }

    /// Find a symbol in the given scope, with fallback to global scope
    pub fn find(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        // First try to find in the specified scope
        if let Some(scope_symbols) = self.scoped_symbols.get(scope) {
            if let Some(symbol) = scope_symbols.get(name) {
                return Some(symbol);
            }
        }
        
        // Fall back to global scope
        self.global_symbols.get(name)
    }

    /// Find a symbol in the current scope, searching outward through parent scopes
    pub fn find_in_scope_hierarchy(&self, name: &Id, current_scope: &ScopeKind) -> Option<&SymbolInfo> {
        // Start with current scope
        if let Some(symbol) = self.find(name, current_scope) {
            return Some(symbol);
        }
        
        // If current scope is named, also check global scope
        if let ScopeKind::Named(_) = current_scope {
            return self.global_symbols.get(name);
        }
        
        None
    }

    /// Get all symbols in a specific scope
    pub fn get_scope_symbols(&self, scope: &ScopeKind) -> Option<&HashMap<Id, SymbolInfo>> {
        match scope {
            ScopeKind::Global => Some(&self.global_symbols),
            ScopeKind::Named(_) => self.scoped_symbols.get(scope),
        }
    }

    /// Check if a symbol exists in the given scope
    pub fn contains(&self, name: &Id, scope: &ScopeKind) -> bool {
        self.find(name, scope).is_some()
    }

    /// Get a symbol by name and scope (alias for find)
    pub fn get(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        self.find(name, scope)
    }

    /// Get all global symbols
    pub fn get_global_symbols(&self) -> &HashMap<Id, SymbolInfo> {
        &self.global_symbols
    }

    /// Get all scoped symbols
    pub fn get_scoped_symbols(&self) -> &HashMap<ScopeKind, HashMap<Id, SymbolInfo>> {
        &self.scoped_symbols
    }

    /// Clear the resolution cache (useful after modifications)
    pub fn clear_cache(&mut self) {
        self.resolution_cache.clear();
    }
}

impl Default for SymbolEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SymbolEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymbolEnvironment")
            .field("global_symbols", &self.global_symbols)
            .field("scoped_symbols", &self.scoped_symbols)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::Id;

    #[test]
    fn test_symbol_environment_basic_operations() {
        let mut env = SymbolEnvironment::new();
        
        // Test inserting global symbols
        let id1 = Id::from("GLOBAL_VAR");
        let id2 = Id::from("FUNCTION_NAME");
        
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global).unwrap();
        env.insert(&id2, SymbolKind::Function, &ScopeKind::Global).unwrap();
        
        // Test finding symbols
        let symbol1 = env.find(&id1, &ScopeKind::Global).unwrap();
        assert_eq!(symbol1.kind, SymbolKind::Variable);
        
        let symbol2 = env.find(&id2, &ScopeKind::Global).unwrap();
        assert_eq!(symbol2.kind, SymbolKind::Function);
        
        // Test scoped symbols
        let scope = ScopeKind::Named(Id::from("FUNCTION_BLOCK"));
        let id3 = Id::from("LOCAL_VAR");
        
        env.insert(&id3, SymbolKind::Variable, &scope).unwrap();
        
        let symbol3 = env.find(&id3, &scope).unwrap();
        assert_eq!(symbol3.kind, SymbolKind::Variable);
        
        // Test scope hierarchy (local scope should find global symbols)
        let symbol1_in_scope = env.find_in_scope_hierarchy(&id1, &scope).unwrap();
        assert_eq!(symbol1_in_scope.kind, SymbolKind::Variable);
    }

    #[test]
    fn test_symbol_environment_scope_management() {
        let mut env = SymbolEnvironment::new();
        
        let global_id = Id::from("GLOBAL");
        let function_id = Id::from("FUNCTION");
        let local_id = Id::from("LOCAL");
        
        // Insert global symbol
        env.insert(&global_id, SymbolKind::Function, &ScopeKind::Global).unwrap();
        
        // Insert function symbol
        env.insert(&function_id, SymbolKind::Function, &ScopeKind::Global).unwrap();
        
        // Insert local symbol in function scope
        let function_scope = ScopeKind::Named(function_id.clone());
        env.insert(&local_id, SymbolKind::Variable, &function_scope).unwrap();
        
        // Verify symbols are in correct scopes
        assert!(env.find(&global_id, &ScopeKind::Global).is_some());
        assert!(env.find(&function_id, &ScopeKind::Global).is_some());
        assert!(env.find(&local_id, &function_scope).is_some());
        
        // Verify local symbol is not visible globally
        assert!(env.find(&local_id, &ScopeKind::Global).is_none());
        
        // Verify global symbols are visible from local scope
        assert!(env.find_in_scope_hierarchy(&global_id, &function_scope).is_some());
    }
}
