use ironplc_dsl::common::{TypeName, VariableType};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use std::collections::HashMap;

/// A scope's position in the nesting tree: the chain of declaration
/// names from the library root.
///
/// A function block is `⟨FB_Motor⟩`; a method declared on it is
/// `⟨FB_Motor, GetSpeed⟩`. Scopes are nameable, which is why this is a
/// path of names rather than an id assigned to an AST node.
///
/// Never empty: an empty path would be the global scope, which
/// [`ScopeKind::Global`] already represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopePath(Vec<Id>);

impl ScopePath {
    pub fn new(segments: Vec<Id>) -> Self {
        debug_assert!(
            !segments.is_empty(),
            "a scope path is never empty; the empty scope is ScopeKind::Global"
        );
        Self(segments)
    }

    pub fn segments(&self) -> &[Id] {
        &self.0
    }
}

impl From<Id> for ScopePath {
    /// A scope directly inside the library, such as a function block.
    fn from(name: Id) -> Self {
        Self::new(vec![name])
    }
}

/// Represents the kind of scope a symbol belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Global scope (library level)
    Global,
    /// Named scope (function, function block, program, method, etc.)
    Named(ScopePath),
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
    /// Function block declaration
    FunctionBlock,
    /// Program declaration
    Program,
    /// Type declaration
    Type,
    /// Constant declaration
    #[allow(unused)]
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
    /// For structure fields, the type name of the structure
    pub struct_type: Option<TypeName>,
    /// The variable type qualifier (VAR, VAR_INPUT, VAR_OUTPUT, etc.)
    pub variable_type: Option<VariableType>,
    /// Formatted hardware address (e.g. "%IX0.0") for direct variables
    pub address: Option<String>,
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
            struct_type: None,
            variable_type: None,
            address: None,
            span,
        }
    }

    pub fn with_external(mut self, is_external: bool) -> Self {
        self.is_external = is_external;
        self
    }

    /// Set the enumeration type for enumeration value symbols
    pub fn with_enum_type(mut self, enum_type: TypeName) -> Self {
        self.enum_type = Some(enum_type);
        self
    }

    /// Set the structure type for structure field symbols
    pub fn with_struct_type(mut self, struct_type: TypeName) -> Self {
        self.struct_type = Some(struct_type);
        self
    }

    pub fn with_variable_type(mut self, vt: VariableType) -> Self {
        self.variable_type = Some(vt);
        self
    }

    pub fn with_address(mut self, addr: String) -> Self {
        self.address = Some(addr);
        self
    }
}

/// The main symbol environment that tracks all symbols across the library
pub struct SymbolEnvironment {
    /// Global symbols (types, functions, function blocks, programs)
    global_symbols: HashMap<Id, SymbolInfo>,
    /// Scoped symbols (variables within functions, function blocks, etc.)
    scoped_symbols: HashMap<ScopeKind, HashMap<Id, SymbolInfo>>,
}

impl SymbolEnvironment {
    pub fn new() -> Self {
        Self {
            global_symbols: HashMap::new(),
            scoped_symbols: HashMap::new(),
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
            ScopeKind::Named(_) => {
                let scope_symbols = self.scoped_symbols.entry(scope.clone()).or_default();

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

    /// Insert a variable with direction and optional hardware address.
    pub fn insert_variable(
        &mut self,
        name: &Id,
        kind: SymbolKind,
        scope: &ScopeKind,
        variable_type: VariableType,
        address: Option<String>,
    ) -> Result<(), Diagnostic> {
        let mut symbol_info = SymbolInfo::new(kind, scope.clone(), name.span())
            .with_variable_type(variable_type.clone());
        if let Some(addr) = address {
            symbol_info = symbol_info.with_address(addr);
        }
        if variable_type == VariableType::External {
            symbol_info = symbol_info.with_external(true);
        }

        match scope {
            ScopeKind::Global => {
                self.global_symbols.insert(name.clone(), symbol_info);
            }
            ScopeKind::Named(_) => {
                let scope_symbols = self.scoped_symbols.entry(scope.clone()).or_default();
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
            ScopeKind::Named(_) => {
                let scope_symbols = self.scoped_symbols.entry(scope.clone()).or_default();

                scope_symbols.insert(name.clone(), symbol_info);
            }
        }

        Ok(())
    }

    /// Insert a structure field with its type information
    pub fn insert_structure_field(
        &mut self,
        name: &Id,
        struct_type: &TypeName,
        scope: &ScopeKind,
    ) -> Result<(), Diagnostic> {
        let symbol_info = SymbolInfo::new(SymbolKind::StructureElement, scope.clone(), name.span())
            .with_struct_type(struct_type.clone());

        match scope {
            ScopeKind::Global => {
                self.global_symbols.insert(name.clone(), symbol_info);
            }
            ScopeKind::Named(_) => {
                let scope_symbols = self.scoped_symbols.entry(scope.clone()).or_default();

                scope_symbols.insert(name.clone(), symbol_info);
            }
        }

        Ok(())
    }

    /// Duplicate enumeration values from one type to another (for aliases)
    pub fn duplicate_enumeration_values_for_alias(
        &mut self,
        source_type: &TypeName,
        alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        // Find all enumeration values for the source type and collect them
        let source_values: Vec<Id> = self
            .get_enumeration_values_for_type(source_type)
            .iter()
            .map(|id| (*id).clone())
            .collect();

        // Duplicate each value with the alias type
        for value_name in source_values {
            self.insert_enumeration_value(&value_name, alias_type, &ScopeKind::Global)?;
        }

        Ok(())
    }

    /// Duplicate structure field symbols from one type to another (for aliases)
    pub fn duplicate_structure_fields_for_alias(
        &mut self,
        source_type: &TypeName,
        alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        // Find all structure field symbols for the source type and collect them
        let source_fields: Vec<Id> = self
            .get_structure_fields_for_type(source_type)
            .iter()
            .map(|id| (*id).clone())
            .collect();

        // Duplicate each field with the alias type
        for field_name in source_fields {
            self.insert_structure_field(&field_name, alias_type, &ScopeKind::Global)?;
        }

        Ok(())
    }

    /// Duplicate array element type information from one type to another (for aliases)
    pub fn duplicate_array_elements_for_alias(
        &mut self,
        _source_type: &TypeName,
        _alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        // For arrays, we don't need to duplicate symbols like we do for enumerations
        // and structures, since arrays don't have named elements that need to be
        // accessible through the alias. The array type itself is what gets aliased.
        // Array elements are accessed by index, not by name.
        Ok(())
    }

    /// Finds a symbol visible from the given scope.
    ///
    /// Walks outward through the enclosing scopes and then the global
    /// scope, so a method body sees its function block's fields and an
    /// inner declaration shadows an outer one of the same name.
    pub fn find(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        if let ScopeKind::Named(path) = scope {
            let segments = path.segments();
            for depth in (1..=segments.len()).rev() {
                let enclosing = ScopeKind::Named(ScopePath::new(segments[..depth].to_vec()));
                if let Some(symbol) = self
                    .scoped_symbols
                    .get(&enclosing)
                    .and_then(|symbols| symbols.get(name))
                {
                    return Some(symbol);
                }
            }
        }

        // Fall back to global scope
        self.global_symbols.get(name)
    }

    /// Get a symbol by name and scope (alias for find)
    #[allow(dead_code)]
    pub fn get(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        self.find(name, scope)
    }

    /// Returns all program declarations from the global scope.
    pub fn get_programs(&self) -> Vec<(&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .filter(|(_, info)| info.kind == SymbolKind::Program)
            .collect()
    }

    /// Returns all function block declarations from the global scope.
    pub fn get_function_blocks(&self) -> Vec<(&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .filter(|(_, info)| info.kind == SymbolKind::FunctionBlock)
            .collect()
    }

    /// Returns all variable-like symbols in the given scope (variables,
    /// parameters, output parameters, and in-out parameters).
    pub fn get_variables_in_scope(&self, scope: &ScopeKind) -> Vec<(&Id, &SymbolInfo)> {
        let Some(scope_symbols) = self.scoped_symbols.get(scope) else {
            return vec![];
        };
        scope_symbols
            .iter()
            .filter(|(_, info)| {
                matches!(
                    info.kind,
                    SymbolKind::Variable
                        | SymbolKind::Parameter
                        | SymbolKind::OutputParameter
                        | SymbolKind::InOutParameter
                )
            })
            .collect()
    }

    /// Iterate over every symbol in the environment: the global symbols
    /// first, followed by every scoped symbol across all named scopes.
    ///
    /// This is the shared traversal used by the read-only lookups that need
    /// to consider both global and scoped declarations.
    fn all_symbols(&self) -> impl Iterator<Item = (&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .chain(self.scoped_symbols.values().flat_map(|scope| scope.iter()))
    }

    /// Get all enumeration values for a specific enumeration type
    pub fn get_enumeration_values_for_type(&self, enum_type: &TypeName) -> Vec<&Id> {
        self.all_symbols()
            .filter(|(_, symbol)| {
                matches!(symbol.kind, SymbolKind::EnumerationValue)
                    && symbol.enum_type.as_ref() == Some(enum_type)
            })
            .map(|(name, _)| name)
            .collect()
    }

    /// Get all structure fields for a specific structure type
    pub fn get_structure_fields_for_type(&self, struct_type: &TypeName) -> Vec<&Id> {
        self.all_symbols()
            .filter(|(_, symbol)| {
                matches!(symbol.kind, SymbolKind::StructureElement)
                    && symbol.struct_type.as_ref() == Some(struct_type)
            })
            .map(|(name, _)| name)
            .collect()
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
        env.insert(&id2, SymbolKind::Program, &ScopeKind::Global)
            .unwrap();

        // Test finding symbols
        let symbol1 = env.find(&id1, &ScopeKind::Global).unwrap();
        assert_eq!(symbol1.kind, SymbolKind::Variable);

        let symbol2 = env.find(&id2, &ScopeKind::Global).unwrap();
        assert_eq!(symbol2.kind, SymbolKind::Program);

        // Test scoped symbols
        let scope = ScopeKind::Named(Id::from("FUNCTION_BLOCK").into());
        let id3 = Id::from("LOCAL_VAR");

        env.insert(&id3, SymbolKind::Variable, &scope).unwrap();

        let symbol3 = env.find(&id3, &scope).unwrap();
        assert_eq!(symbol3.kind, SymbolKind::Variable);

        // Test scope hierarchy (local scope should find global symbols)
        let symbol1_in_scope = env.find(&id1, &scope).unwrap();
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
        env.insert(&global_id, SymbolKind::Program, &ScopeKind::Global)
            .unwrap();

        // Insert function symbol
        env.insert(&function_id, SymbolKind::Program, &ScopeKind::Global)
            .unwrap();

        // Insert local symbol in function scope
        let function_scope = ScopeKind::Named(function_id.clone().into());
        env.insert(&local_id, SymbolKind::Variable, &function_scope)
            .unwrap();

        // Verify symbols are in correct scopes
        assert!(env.find(&global_id, &ScopeKind::Global).is_some());
        assert!(env.find(&function_id, &ScopeKind::Global).is_some());
        assert!(env.find(&local_id, &function_scope).is_some());

        // Verify local symbol is not visible globally
        assert!(env.find(&local_id, &ScopeKind::Global).is_none());

        // Verify global symbols are visible from local scope
        assert!(env.find(&global_id, &function_scope).is_some());
    }

    #[test]
    fn get_when_checking_symbol_existence_then_returns_correct_results() {
        let mut env = SymbolEnvironment::new();

        let id1 = Id::from("GLOBAL_VAR");
        let id2 = Id::from("LOCAL_VAR");

        // Insert global symbol
        env.insert(&id1, SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        // Test get method (alias for find)
        let symbol1 = env.get(&id1, &ScopeKind::Global).unwrap();
        assert_eq!(symbol1.kind, SymbolKind::Variable);

        let symbol2 = env.get(&id2, &ScopeKind::Global);
        assert!(symbol2.is_none());
    }

    #[test]
    fn default_implementation_when_creating_default_then_creates_empty_environment() {
        let env = SymbolEnvironment::default();

        // Default should create an empty environment
        assert_eq!(env.all_symbols().count(), 0);

        // Should be equivalent to new()
        let env2 = SymbolEnvironment::new();
        assert_eq!(env.all_symbols().count(), env2.all_symbols().count());
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
        let named_scope = ScopeKind::Named(function_id.clone().into());
        assert_eq!(named_scope, ScopeKind::Named(function_id.into()));

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

        let wrong_scope = ScopeKind::Named(Id::from("WRONG_FUNCTION").into());
        let found = env.find(&global_id, &wrong_scope);
        // Global symbols are accessible from any scope, so this should find the symbol
        assert!(found.is_some());

        // Test scope hierarchy with non-existent scope
        let non_existent_scope = ScopeKind::Named(Id::from("NON_EXISTENT").into());
        let found = env.find(&global_id, &non_existent_scope);
        assert!(found.is_some()); // Should find in global scope
    }

    #[test]
    fn get_enumeration_values_for_type_when_values_in_global_and_scoped_then_returns_matching_only()
    {
        let mut env = SymbolEnvironment::new();
        let enum_type = TypeName::from("COLOR");
        let other_type = TypeName::from("SIZE");

        // Global enumeration value of the requested type.
        env.insert_enumeration_value(&Id::from("RED"), &enum_type, &ScopeKind::Global)
            .unwrap();
        // Scoped enumeration value of the requested type.
        let scope = ScopeKind::Named(Id::from("FB").into());
        env.insert_enumeration_value(&Id::from("GREEN"), &enum_type, &scope)
            .unwrap();
        // Enumeration value of a different type (should be excluded).
        env.insert_enumeration_value(&Id::from("SMALL"), &other_type, &ScopeKind::Global)
            .unwrap();
        // Non-enumeration symbol whose enum_type is None (should be excluded).
        env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let values = env.get_enumeration_values_for_type(&enum_type);
        assert_eq!(values.len(), 2);
        assert!(values.iter().any(|id| **id == Id::from("RED")));
        assert!(values.iter().any(|id| **id == Id::from("GREEN")));
    }

    #[test]
    fn get_enumeration_values_for_type_when_no_matching_values_then_returns_empty() {
        let mut env = SymbolEnvironment::new();
        env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let values = env.get_enumeration_values_for_type(&TypeName::from("COLOR"));
        assert!(values.is_empty());
    }

    #[test]
    fn get_structure_fields_for_type_when_fields_in_global_and_scoped_then_returns_matching_only() {
        let mut env = SymbolEnvironment::new();
        let struct_type = TypeName::from("POINT");
        let other_type = TypeName::from("LINE");

        // Global structure field of the requested type.
        env.insert_structure_field(&Id::from("X"), &struct_type, &ScopeKind::Global)
            .unwrap();
        // Scoped structure field of the requested type.
        let scope = ScopeKind::Named(Id::from("FB").into());
        env.insert_structure_field(&Id::from("Y"), &struct_type, &scope)
            .unwrap();
        // Structure field of a different type (should be excluded).
        env.insert_structure_field(&Id::from("START"), &other_type, &ScopeKind::Global)
            .unwrap();
        // Non-structure symbol whose struct_type is None (should be excluded).
        env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let fields = env.get_structure_fields_for_type(&struct_type);
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|id| **id == Id::from("X")));
        assert!(fields.iter().any(|id| **id == Id::from("Y")));
    }

    #[test]
    fn get_structure_fields_for_type_when_no_matching_fields_then_returns_empty() {
        let mut env = SymbolEnvironment::new();
        env.insert(&Id::from("PLAIN"), SymbolKind::Variable, &ScopeKind::Global)
            .unwrap();

        let fields = env.get_structure_fields_for_type(&TypeName::from("POINT"));
        assert!(fields.is_empty());
    }

    #[test]
    fn symbol_info_span_and_scope_when_creating_symbol_info_then_has_correct_span_and_scope() {
        let span = ironplc_dsl::core::SourceSpan::default();
        let scope = ScopeKind::Named(Id::from("TEST_FUNCTION").into());

        let symbol_info = SymbolInfo::new(SymbolKind::Variable, scope.clone(), span);

        // Test that scope and visibility_scope are set correctly
        assert_eq!(symbol_info.scope, scope);
        assert_eq!(symbol_info.visibility_scope, scope);
        assert_eq!(symbol_info.span, ironplc_dsl::core::SourceSpan::default());
        assert!(!symbol_info.is_external);
        assert!(symbol_info.data_type.is_none());
    }

    /// A scope path nests, so a symbol declared in an enclosing scope is
    /// visible from an inner one -- how a method body sees the fields of
    /// the function block it is declared on.
    #[test]
    fn find_when_symbol_is_in_enclosing_scope_then_found_from_inner_scope() {
        let mut env = SymbolEnvironment::new();

        let outer = ScopeKind::Named(Id::from("FB_Motor").into());
        let inner = ScopeKind::Named(ScopePath::new(vec![
            Id::from("FB_Motor"),
            Id::from("GetSpeed"),
        ]));

        let field = Id::from("speed");
        env.insert(&field, SymbolKind::Variable, &outer).unwrap();

        assert!(
            env.find(&field, &inner).is_some(),
            "an enclosing scope's symbol should be visible from the inner scope"
        );
    }

    /// The innermost declaration of a name wins, so a method local
    /// shadows a function block field of the same name.
    #[test]
    fn find_when_inner_scope_redeclares_name_then_inner_symbol_shadows_outer() {
        let mut env = SymbolEnvironment::new();

        let outer = ScopeKind::Named(Id::from("FB_Motor").into());
        let inner = ScopeKind::Named(ScopePath::new(vec![
            Id::from("FB_Motor"),
            Id::from("GetSpeed"),
        ]));

        let name = Id::from("v");
        env.insert(&name, SymbolKind::Variable, &outer).unwrap();
        env.insert(&name, SymbolKind::Parameter, &inner).unwrap();

        assert_eq!(env.find(&name, &inner).unwrap().kind, SymbolKind::Parameter);
        assert_eq!(env.find(&name, &outer).unwrap().kind, SymbolKind::Variable);
    }

    /// A name declared only in an inner scope does not leak outward.
    #[test]
    fn find_when_symbol_is_in_inner_scope_then_not_found_from_enclosing_scope() {
        let mut env = SymbolEnvironment::new();

        let outer = ScopeKind::Named(Id::from("FB_Motor").into());
        let inner = ScopeKind::Named(ScopePath::new(vec![
            Id::from("FB_Motor"),
            Id::from("GetSpeed"),
        ]));

        let local = Id::from("q");
        env.insert(&local, SymbolKind::Variable, &inner).unwrap();

        assert!(env.find(&local, &outer).is_none());
    }
}
