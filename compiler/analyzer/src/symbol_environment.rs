use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use std::collections::HashMap;

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
    /// For enumeration values, the type name of the enumeration
    /// TODO this should probably be a new struct that is a TypeRef
    /// so that we can distinguish between the actual place of the declaration
    /// and a reference to the declaration.
    pub enum_type: Option<TypeName>,
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
            enum_type: None,
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

    /// Set the enumeration type for enumeration value symbols
    pub fn with_enum_type(mut self, enum_type: TypeName) -> Self {
        self.enum_type = Some(enum_type);
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
                let scope_symbols = self
                    .scoped_symbols
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

    /// Insert an enumeration value with its type information
    pub fn insert_enumeration_value(
        &mut self,
        name: &Id,
        enum_type: &TypeName,
        scope: &ScopeKind,
    ) -> Result<(), Diagnostic> {
        let symbol_info = SymbolInfo::new(SymbolKind::EnumerationValue, scope.clone(), name.span())
            .with_enum_type(enum_type.clone());

        match scope {
            ScopeKind::Global => {
                self.global_symbols.insert(name.clone(), symbol_info);
            }
            ScopeKind::Named(scope_name) => {
                let scope_symbols = self
                    .scoped_symbols
                    .entry(ScopeKind::Named(scope_name.clone()))
                    .or_default();

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
    #[allow(dead_code)]
    pub fn find_in_scope_hierarchy(
        &self,
        name: &Id,
        current_scope: &ScopeKind,
    ) -> Option<&SymbolInfo> {
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
        summary.push_str(&format!(
            "📚 Global Symbols ({}):\n",
            self.global_symbols.len()
        ));
        for (name, symbol) in &self.global_symbols {
            summary.push_str(&format!(
                "  • {}: {:?} at {:?}\n",
                name.original(),
                symbol.kind,
                symbol.span
            ));
        }

        // Scoped symbols summary
        summary.push_str(&format!(
            "\n🔧 Scoped Symbols ({} scopes):\n",
            self.scoped_symbols.len()
        ));
        for (scope, symbols) in &self.scoped_symbols {
            let scope_name = match scope {
                ScopeKind::Global => "Global".to_string(),
                ScopeKind::Named(id) => format!("Named({})", id.original()),
            };
            summary.push_str(&format!(
                "  📍 {} ({} symbols):\n",
                scope_name,
                symbols.len()
            ));
            for (name, symbol) in symbols {
                summary.push_str(&format!(
                    "    • {}: {:?} at {:?}\n",
                    name.original(),
                    symbol.kind,
                    symbol.span
                ));
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
        summary.push_str(&format!(
            "  • Number of scopes: {}\n",
            self.scoped_symbols.len()
        ));

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

    /// Get all enumeration values for a specific enumeration type
    pub fn get_enumeration_values_for_type(&self, enum_type: &TypeName) -> Vec<&Id> {
        let mut values = Vec::new();

        // Check global symbols
        for (name, symbol) in &self.global_symbols {
            if matches!(symbol.kind, SymbolKind::EnumerationValue) {
                if let Some(ref symbol_enum_type) = symbol.enum_type {
                    if symbol_enum_type == enum_type {
                        values.push(name);
                    }
                }
            }
        }

        // Check scoped symbols
        for scope_symbols in self.scoped_symbols.values() {
            for (name, symbol) in scope_symbols {
                if matches!(symbol.kind, SymbolKind::EnumerationValue) {
                    if let Some(ref symbol_enum_type) = symbol.enum_type {
                        if symbol_enum_type == enum_type {
                            values.push(name);
                        }
                    }
                }
            }
        }

        values
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
    fn symbol_environment_basic_operations_when_inserting_and_finding_symbols_then_works_correctly()
    {
        let mut env = SymbolEnvironment::new();

        // Test inserting global symbols
        let id1 = Id::from("GLOBAL_VAR");
        let id2 = Id::from("FUNCTION_NAME");

        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&id2, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

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
    fn symbol_environment_scope_management_when_managing_scopes_then_symbols_are_in_correct_scopes()
    {
        let mut env = SymbolEnvironment::new();

        let global_id = Id::from("GLOBAL");
        let function_id = Id::from("FUNCTION");
        let local_id = Id::from("LOCAL");

        // Insert global symbol
        env.insert(&global_id, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        // Insert function symbol
        env.insert(&function_id, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        // Insert local symbol in function scope
        let function_scope = ScopeKind::Named(function_id.clone());
        env.insert(&local_id, SymbolKind::Variable, &function_scope)
            .unwrap();

        // Verify symbols are in correct scopes
        assert!(env.find(&global_id, &ScopeKind::Global).is_some());
        assert!(env.find(&function_id, &ScopeKind::Global).is_some());
        assert!(env.find(&local_id, &function_scope).is_some());

        // Verify local symbol is not visible globally
        assert!(env.find(&local_id, &ScopeKind::Global).is_none());

        // Verify global symbols are visible from local scope
        assert!(env
            .find_in_scope_hierarchy(&global_id, &function_scope)
            .is_some());
    }

    #[test]
    fn symbol_info_builder_methods_when_using_builder_pattern_then_creates_correct_symbol_info() {
        let span = ironplc_dsl::core::SourceSpan::default();
        let mut symbol_info = SymbolInfo::new(SymbolKind::Variable, ScopeKind::Global, span);

        // Test with_data_type
        symbol_info = symbol_info.with_data_type("INT".to_string());
        assert_eq!(symbol_info.data_type, Some("INT".to_string()));

        // Test with_external
        symbol_info = symbol_info.with_external(true);
        assert!(symbol_info.is_external);

        // Test with_visibility_scope
        let named_scope = ScopeKind::Named(Id::from("FUNCTION"));
        symbol_info = symbol_info.with_visibility_scope(named_scope.clone());
        assert_eq!(symbol_info.visibility_scope, named_scope);
    }

    #[test]
    fn total_symbols_count_when_counting_symbols_then_returns_correct_total() {
        let mut env = SymbolEnvironment::new();

        // Initially empty
        assert_eq!(env.total_symbols(), 0);

        // Add global symbols
        let id1 = Id::from("GLOBAL1");
        let id2 = Id::from("GLOBAL2");
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&id2, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();
        assert_eq!(env.total_symbols(), 2);

        // Add scoped symbols
        let scope = ScopeKind::Named(Id::from("FUNCTION"));
        let id3 = Id::from("LOCAL1");
        let id4 = Id::from("LOCAL2");
        env.insert(&id3, SymbolKind::Variable, &scope).unwrap();
        env.insert(&id4, SymbolKind::Parameter, &scope).unwrap();
        assert_eq!(env.total_symbols(), 4);
    }

    #[test]
    fn get_scope_symbols_when_getting_symbols_from_scope_then_returns_correct_symbols() {
        let mut env = SymbolEnvironment::new();

        // Test global scope
        let global_symbols = env.get_scope_symbols(&ScopeKind::Global).unwrap();
        assert_eq!(global_symbols.len(), 0); // Initially empty

        // Add global symbols
        let id1 = Id::from("GLOBAL1");
        let id2 = Id::from("GLOBAL2");
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&id2, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        let global_symbols = env.get_scope_symbols(&ScopeKind::Global).unwrap();
        assert_eq!(global_symbols.len(), 2);
        assert!(global_symbols.contains_key(&id1));
        assert!(global_symbols.contains_key(&id2));

        // Test named scope
        let scope = ScopeKind::Named(Id::from("FUNCTION"));
        let named_symbols = env.get_scope_symbols(&scope);
        assert!(named_symbols.is_none()); // Scope doesn't exist yet

        // Add symbols to named scope
        let id3 = Id::from("LOCAL1");
        env.insert(&id3, SymbolKind::Variable, &scope).unwrap();

        let named_symbols = env.get_scope_symbols(&scope).unwrap();
        assert_eq!(named_symbols.len(), 1);
        assert!(named_symbols.contains_key(&id3));
    }

    #[test]
    fn contains_and_get_methods_when_checking_symbol_existence_then_returns_correct_results() {
        let mut env = SymbolEnvironment::new();

        let id1 = Id::from("GLOBAL_VAR");
        let id2 = Id::from("LOCAL_VAR");

        // Insert global symbol
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        // Test contains method
        assert!(env.contains(&id1, &ScopeKind::Global));
        // Global symbols are accessible from any scope, so this should be true
        assert!(env.contains(&id1, &ScopeKind::Named(Id::from("FUNCTION"))));
        assert!(!env.contains(&id2, &ScopeKind::Global));

        // Test get method (alias for find)
        let symbol1 = env.get(&id1, &ScopeKind::Global).unwrap();
        assert_eq!(symbol1.kind, SymbolKind::Variable);

        let symbol2 = env.get(&id2, &ScopeKind::Global);
        assert!(symbol2.is_none());
    }

    #[test]
    fn get_global_and_scoped_symbols_when_getting_all_symbols_then_returns_correct_symbols() {
        let mut env = SymbolEnvironment::new();

        // Initially empty
        let global_symbols = env.get_global_symbols();
        assert_eq!(global_symbols.len(), 0);

        let scoped_symbols = env.get_scoped_symbols();
        assert_eq!(scoped_symbols.len(), 0);

        // Add global symbols
        let id1 = Id::from("GLOBAL1");
        let id2 = Id::from("GLOBAL2");
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&id2, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        let global_symbols = env.get_global_symbols();
        assert_eq!(global_symbols.len(), 2);
        assert!(global_symbols.contains_key(&id1));
        assert!(global_symbols.contains_key(&id2));

        // Add scoped symbols
        let scope1 = ScopeKind::Named(Id::from("FUNCTION1"));
        let scope2 = ScopeKind::Named(Id::from("FUNCTION2"));

        let id3 = Id::from("LOCAL1");
        let id4 = Id::from("LOCAL2");
        env.insert(&id3, SymbolKind::Variable, &scope1).unwrap();
        env.insert(&id4, SymbolKind::Parameter, &scope2).unwrap();

        let scoped_symbols = env.get_scoped_symbols();
        assert_eq!(scoped_symbols.len(), 2);
        assert!(scoped_symbols.contains_key(&scope1));
        assert!(scoped_symbols.contains_key(&scope2));

        let scope1_symbols = &scoped_symbols[&scope1];
        assert_eq!(scope1_symbols.len(), 1);
        assert!(scope1_symbols.contains_key(&id3));

        let scope2_symbols = &scoped_symbols[&scope2];
        assert_eq!(scope2_symbols.len(), 1);
        assert!(scope2_symbols.contains_key(&id4));
    }

    #[test]
    fn clear_cache_when_clearing_cache_then_symbols_are_removed() {
        let mut env = SymbolEnvironment::new();

        // clear_cache should not panic even when empty
        env.clear_cache();

        // Add some symbols and clear cache again
        let id = Id::from("TEST_VAR");
        env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.clear_cache();

        // Symbols should still be accessible after clearing cache
        assert!(env.contains(&id, &ScopeKind::Global));
    }

    #[test]
    fn default_implementation_when_creating_default_then_creates_empty_environment() {
        let env = SymbolEnvironment::default();

        // Default should create an empty environment
        assert_eq!(env.total_symbols(), 0);
        assert_eq!(env.get_global_symbols().len(), 0);
        assert_eq!(env.get_scoped_symbols().len(), 0);

        // Should be equivalent to new()
        let env2 = SymbolEnvironment::new();
        assert_eq!(env.total_symbols(), env2.total_symbols());
    }

    #[test]
    fn get_accessible_symbols_when_getting_accessible_symbols_then_returns_correct_symbols() {
        let mut env = SymbolEnvironment::new();

        // Add global symbols
        let global_id1 = Id::from("GLOBAL1");
        let global_id2 = Id::from("GLOBAL2");
        env.insert(&global_id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&global_id2, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        // Add scoped symbols
        let scope = ScopeKind::Named(Id::from("FUNCTION"));
        let local_id1 = Id::from("LOCAL1");
        let local_id2 = Id::from("LOCAL2");
        env.insert(&local_id1, SymbolKind::Variable, &scope)
            .unwrap();
        env.insert(&local_id2, SymbolKind::Parameter, &scope)
            .unwrap();

        // Test accessible symbols from global scope
        let global_accessible = env.get_accessible_symbols(&ScopeKind::Global);
        assert_eq!(global_accessible.len(), 2); // Only global symbols
        assert!(global_accessible.iter().any(|(id, _)| **id == global_id1));
        assert!(global_accessible.iter().any(|(id, _)| **id == global_id2));

        // Test accessible symbols from named scope
        let scope_accessible = env.get_accessible_symbols(&scope);
        assert_eq!(scope_accessible.len(), 4); // Global + local symbols
        assert!(scope_accessible.iter().any(|(id, _)| **id == global_id1));
        assert!(scope_accessible.iter().any(|(id, _)| **id == global_id2));
        assert!(scope_accessible.iter().any(|(id, _)| **id == local_id1));
        assert!(scope_accessible.iter().any(|(id, _)| **id == local_id2));
    }

    #[test]
    fn generate_summary_when_generating_summary_then_returns_correct_summary() {
        let mut env = SymbolEnvironment::new();

        // Test empty summary
        let empty_summary = env.generate_summary();
        assert!(empty_summary.contains("Global Symbols (0)"));
        assert!(empty_summary.contains("Scoped Symbols (0 scopes)"));
        assert!(empty_summary.contains("Total symbols: 0"));

        // Add some symbols and test populated summary
        let global_id = Id::from("GLOBAL_VAR");
        let function_id = Id::from("TEST_FUNCTION");
        let local_id = Id::from("LOCAL_VAR");

        env.insert(&global_id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&function_id, SymbolKind::Function, &ScopeKind::Global)
            .unwrap();

        let scope = ScopeKind::Named(function_id.clone());
        env.insert(&local_id, SymbolKind::Parameter, &scope)
            .unwrap();

        let summary = env.generate_summary();
        assert!(summary.contains("Global Symbols (2)"));
        assert!(summary.contains("Scoped Symbols (1 scopes)"));
        assert!(summary.contains("Total symbols: 3"));
        assert!(summary.contains("GLOBAL_VAR"));
        assert!(summary.contains("TEST_FUNCTION"));
        assert!(summary.contains("LOCAL_VAR"));
        assert!(summary.contains("Named(TEST_FUNCTION)"));
    }

    #[test]
    fn get_symbol_details_when_getting_symbol_details_then_returns_correct_details() {
        let mut env = SymbolEnvironment::new();

        // Test getting details for non-existent symbol
        let non_existent_id = Id::from("NON_EXISTENT");
        let details = env.get_symbol_details(&non_existent_id, &ScopeKind::Global);
        assert!(details.is_none());

        // Test getting details for existing global symbol
        let global_id = Id::from("GLOBAL_VAR");
        env.insert(&global_id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let details = env
            .get_symbol_details(&global_id, &ScopeKind::Global)
            .unwrap();
        assert!(details.contains("Symbol: GLOBAL_VAR"));
        assert!(details.contains("Kind: Variable"));
        assert!(details.contains("Scope: Global"));
        assert!(details.contains("Visibility: Global"));
        assert!(details.contains("External: false"));
        assert!(details.contains("Data Type: None"));

        // Test getting details for scoped symbol
        let function_id = Id::from("TEST_FUNCTION");
        let scope = ScopeKind::Named(function_id.clone());
        let local_id = Id::from("LOCAL_PARAM");

        env.insert(&local_id, SymbolKind::Parameter, &scope)
            .unwrap();

        let details = env.get_symbol_details(&local_id, &scope).unwrap();
        assert!(details.contains("Symbol: LOCAL_PARAM"));
        assert!(details.contains("Kind: Parameter"));
        assert!(details.contains("Scope: Named(TEST_FUNCTION)"));
        assert!(details.contains("Visibility: Named(TEST_FUNCTION)"));
    }

    #[test]
    fn debug_implementation_when_debugging_then_formats_correctly() {
        let mut env = SymbolEnvironment::new();

        // Test debug output for empty environment
        let debug_output = format!("{env:?}");
        assert!(debug_output.contains("SymbolEnvironment"));
        assert!(debug_output.contains("global_symbols"));
        assert!(debug_output.contains("scoped_symbols"));

        // Test debug output with symbols
        let id = Id::from("TEST_VAR");
        env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let debug_output = format!("{env:?}");
        assert!(debug_output.contains("SymbolEnvironment"));
        assert!(debug_output.contains("global_symbols"));
        assert!(debug_output.contains("scoped_symbols"));
    }

    #[test]
    fn scope_kind_variants_when_creating_scope_kinds_then_creates_correct_variants() {
        // Test Global scope
        let global_scope = ScopeKind::Global;
        assert_eq!(global_scope, ScopeKind::Global);

        // Test Named scope
        let function_id = Id::from("TEST_FUNCTION");
        let named_scope = ScopeKind::Named(function_id.clone());
        assert_eq!(named_scope, ScopeKind::Named(function_id));

        // Test scope comparison
        assert_ne!(global_scope, named_scope);

        // Test scope cloning
        let cloned_scope = named_scope.clone();
        assert_eq!(named_scope, cloned_scope);
    }

    #[test]
    fn edge_cases_and_error_conditions_when_handling_edge_cases_then_handles_correctly() {
        let mut env = SymbolEnvironment::new();

        // Test inserting same symbol multiple times (should not panic)
        let id = Id::from("DUPLICATE_VAR");
        env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();
        env.insert(&id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap(); // Should not panic

        // Test finding symbol in wrong scope
        let global_id = Id::from("GLOBAL_ONLY");
        env.insert(&global_id, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let wrong_scope = ScopeKind::Named(Id::from("WRONG_FUNCTION"));
        let found = env.find(&global_id, &wrong_scope);
        // Global symbols are accessible from any scope, so this should find the symbol
        assert!(found.is_some());

        // Test scope hierarchy with non-existent scope
        let non_existent_scope = ScopeKind::Named(Id::from("NON_EXISTENT"));
        let found = env.find_in_scope_hierarchy(&global_id, &non_existent_scope);
        assert!(found.is_some()); // Should find in global scope

        // Test empty scopes
        let empty_scope = ScopeKind::Named(Id::from("EMPTY_FUNCTION"));
        let scope_symbols = env.get_scope_symbols(&empty_scope);
        assert!(scope_symbols.is_none());

        let accessible = env.get_accessible_symbols(&empty_scope);
        assert_eq!(accessible.len(), 2); // Both global symbols (DUPLICATE_VAR and GLOBAL_ONLY)
    }

    #[test]
    fn symbol_info_span_and_scope_when_creating_symbol_info_then_has_correct_span_and_scope() {
        let span = ironplc_dsl::core::SourceSpan::default();
        let scope = ScopeKind::Named(Id::from("TEST_FUNCTION"));

        let symbol_info = SymbolInfo::new(SymbolKind::Variable, scope.clone(), span);

        // Test that scope and visibility_scope are set correctly
        assert_eq!(symbol_info.scope, scope);
        assert_eq!(symbol_info.visibility_scope, scope);
        assert_eq!(symbol_info.span, ironplc_dsl::core::SourceSpan::default());
        assert!(!symbol_info.is_external);
        assert!(symbol_info.data_type.is_none());
    }
}
