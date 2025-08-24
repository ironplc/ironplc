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
#[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn with_data_type(mut self, data_type: String) -> Self {
        self.data_type = Some(data_type);
        self
    }

    pub fn with_external(mut self, is_external: bool) -> Self {
        self.is_external = is_external;
        self
    }

    #[allow(dead_code)]
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
                    .or_default();
                
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
    #[allow(dead_code)]
    pub fn get_scope_symbols(&self, scope: &ScopeKind) -> Option<&HashMap<Id, SymbolInfo>> {
        match scope {
            ScopeKind::Global => Some(&self.global_symbols),
            ScopeKind::Named(_) => self.scoped_symbols.get(scope),
        }
    }

    /// Check if a symbol exists in the given scope
    #[allow(dead_code)]
    pub fn contains(&self, name: &Id, scope: &ScopeKind) -> bool {
        self.find(name, scope).is_some()
    }

    /// Get a symbol by name and scope (alias for find)
    #[allow(dead_code)]
    pub fn get(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        self.find(name, scope)
    }

    /// Get all global symbols
    #[allow(dead_code)]
    pub fn get_global_symbols(&self) -> &HashMap<Id, SymbolInfo> {
        &self.global_symbols
    }

    /// Get all scoped symbols
    #[allow(dead_code)]
    pub fn get_scoped_symbols(&self) -> &HashMap<ScopeKind, HashMap<Id, SymbolInfo>> {
        &self.scoped_symbols
    }

    /// Clear the resolution cache (useful after modifications)
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.resolution_cache.clear();
    }

    /// Get the total number of symbols in the environment
    #[allow(dead_code)]
    pub fn total_symbols(&self) -> usize {
        let global_count = self.global_symbols.len();
        let scoped_count: usize = self.scoped_symbols.values().map(|scope| scope.len()).sum();
        global_count + scoped_count
    }

    /// Generate a comprehensive summary of the symbol table
    #[allow(dead_code)]
    pub fn generate_summary(&self) -> String {
        let mut summary = String::new();
        
        // Global symbols summary
        summary.push_str(&format!("📚 Global Symbols ({}):\n", self.global_symbols.len()));
        for (name, symbol) in &self.global_symbols {
            summary.push_str(&format!("  • {}: {:?} at {:?}\n", 
                name.original(), symbol.kind, symbol.span));
        }
        
        // Scoped symbols summary
        summary.push_str(&format!("\n🔧 Scoped Symbols ({} scopes):\n", self.scoped_symbols.len()));
        for (scope, symbols) in &self.scoped_symbols {
            let scope_name = match scope {
                ScopeKind::Global => "Global".to_string(),
                ScopeKind::Named(id) => format!("Named({})", id.original()),
            };
            summary.push_str(&format!("  📍 {} ({} symbols):\n", scope_name, symbols.len()));
            for (name, symbol) in symbols {
                summary.push_str(&format!("    • {}: {:?} at {:?}\n", 
                    name.original(), symbol.kind, symbol.span));
            }
        }
        
        // Statistics
        let total_symbols = self.total_symbols();
        let global_count = self.global_symbols.len();
        let scoped_count: usize = self.scoped_symbols.values().map(|scope| scope.len()).sum();
        
        summary.push_str("\n📊 Summary Statistics:\n");
        summary.push_str(&format!("  • Total symbols: {total_symbols}\n"));
        summary.push_str(&format!("  • Global symbols: {global_count}\n"));
        summary.push_str(&format!("  • Scoped symbols: {scoped_count}\n"));
        summary.push_str(&format!("  • Number of scopes: {}\n", self.scoped_symbols.len()));
        
        summary
    }

    /// Get detailed information about a specific symbol
    #[allow(dead_code)]
    pub fn get_symbol_details(&self, name: &Id, scope: &ScopeKind) -> Option<String> {
        if let Some(symbol) = self.find(name, scope) {
            let scope_name = match &symbol.scope {
                ScopeKind::Global => "Global".to_string(),
                ScopeKind::Named(id) => format!("Named({})", id.original()),
            };
            
            let visibility_name = match &symbol.visibility_scope {
                ScopeKind::Global => "Global".to_string(),
                ScopeKind::Named(id) => format!("Named({})", id.original()),
            };
            
            Some(format!(
                "Symbol: {}\n  Kind: {:?}\n  Scope: {}\n  Visibility: {}\n  External: {}\n  Data Type: {:?}\n  Location: {:?}",
                name.original(),
                symbol.kind,
                scope_name,
                visibility_name,
                symbol.is_external,
                symbol.data_type,
                symbol.span
            ))
        } else {
            None
        }
    }

    /// Validate that a symbol reference is valid in the given scope
    #[allow(dead_code)]
    #[allow(unused_variables)]
    pub fn validate_symbol_reference(&self, name: &Id, scope: &ScopeKind) -> Result<(), String> {
        if let Some(symbol) = self.find_in_scope_hierarchy(name, scope) {
            // Symbol exists and is accessible from this scope
            Ok(())
        } else {
            // Symbol not found - generate helpful error message
            let scope_name = match scope {
                ScopeKind::Global => "global scope".to_string(),
                ScopeKind::Named(id) => format!("scope '{}'", id.original()),
            };
            
            // Check if symbol exists in other scopes for better error messages
            if let Some(global_symbol) = self.global_symbols.get(name) {
                Err(format!("Symbol '{}' exists in global scope but is not accessible from {}", 
                    name.original(), scope_name))
            } else {
                // Check if symbol exists in any named scope
                for (named_scope, symbols) in &self.scoped_symbols {
                    if let Some(symbol) = symbols.get(name) {
                        let named_scope_name = match named_scope {
                            ScopeKind::Global => "global scope".to_string(),
                            ScopeKind::Named(id) => format!("scope '{}'", id.original()),
                        };
                        return Err(format!("Symbol '{}' exists in {} but is not accessible from {}", 
                            name.original(), named_scope_name, scope_name));
                    }
                }
                
                Err(format!("Symbol '{}' is not declared in {}", name.original(), scope_name))
            }
        }
    }

    /// Get all symbols that are accessible from a given scope
    #[allow(dead_code)]
    #[allow(unused_variables)]
    pub fn get_accessible_symbols(&self, scope: &ScopeKind) -> Vec<(&Id, &SymbolInfo)> {
        let mut accessible = Vec::new();
        
        // Add global symbols (always accessible)
        for (name, symbol) in &self.global_symbols {
            accessible.push((name, symbol));
        }
        
        // Add symbols from the current scope
        if let Some(scope_symbols) = self.scoped_symbols.get(scope) {
            for (name, symbol) in scope_symbols {
                accessible.push((name, symbol));
            }
        }
        
        accessible
    }

    /// Check for duplicate symbol declarations and return detailed information
    #[allow(dead_code)]
    pub fn check_for_duplicates(&self) -> Vec<String> {
        let mut duplicates = Vec::new();
        
        // Check for duplicate global symbols
        let mut global_names = std::collections::HashSet::new();
        for (name, symbol) in &self.global_symbols {
            if !global_names.insert(name.original()) {
                duplicates.push(format!("Duplicate global symbol '{}' declared at {:?}", 
                    name.original(), symbol.span));
            }
        }
        
        // Check for duplicate symbols within each scope
        for (scope, symbols) in &self.scoped_symbols {
            let scope_name = match scope {
                ScopeKind::Global => "global scope".to_string(),
                ScopeKind::Named(id) => format!("scope '{}'", id.original()),
            };
            
            let mut scope_names = std::collections::HashSet::new();
            for (name, symbol) in symbols {
                if !scope_names.insert(name.original()) {
                    duplicates.push(format!("Duplicate symbol '{}' in {} declared at {:?}", 
                        name.original(), scope_name, symbol.span));
                }
            }
        }
        
        duplicates
    }

    /// Get symbol statistics for analysis and debugging
    #[allow(dead_code)]
    pub fn get_statistics(&self) -> std::collections::HashMap<String, usize> {
        let mut stats = std::collections::HashMap::new();
        
        // Count symbols by kind
        for symbol in self.global_symbols.values() {
            let kind_name = format!("{:?}", symbol.kind);
            *stats.entry(kind_name).or_insert(0) += 1;
        }
        
        for symbols in self.scoped_symbols.values() {
            for symbol in symbols.values() {
                let kind_name = format!("{:?}", symbol.kind);
                *stats.entry(kind_name).or_insert(0) += 1;
            }
        }
        
        stats
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
