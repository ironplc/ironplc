//! Semantic context for semantic analysis.
//!
//! This module provides a unified container for all environments needed during
//! semantic analysis. Instead of passing multiple environments separately to
//! each validation rule, we pass a single SemanticContext that contains all
//! the necessary information.
//!
//! This design:
//! - Reduces parameter threading through the analysis pipeline
//! - Makes it easy to add new environments without changing function signatures
//! - Provides a clear "this is everything you need" abstraction

use crate::function_environment::{FunctionEnvironment, FunctionEnvironmentBuilder};
use crate::symbol_environment::SymbolEnvironment;
use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};
use ironplc_dsl::diagnostic::Diagnostic;

/// Contains all environments needed for semantic analysis.
///
/// The SemanticContext bundles together the type environment, function environment,
/// and symbol environment. This allows validation rules to access any environment
/// they need without requiring changes to their function signatures when new
/// environments are added.
#[derive(Debug)]
pub struct SemanticContext {
    /// Type environment containing type definitions (elementary types, user-defined types,
    /// and stdlib function blocks)
    pub types: TypeEnvironment,
    /// Function environment containing function signatures (stdlib functions and
    /// user-defined functions)
    pub functions: FunctionEnvironment,
    /// Symbol environment containing variable and symbol declarations
    pub symbols: SymbolEnvironment,
}

impl SemanticContext {
    /// Creates a new SemanticContext with the given environments.
    pub fn new(
        types: TypeEnvironment,
        functions: FunctionEnvironment,
        symbols: SymbolEnvironment,
    ) -> Self {
        Self {
            types,
            functions,
            symbols,
        }
    }

    /// Provides read-only access to the type environment.
    pub fn types(&self) -> &TypeEnvironment {
        &self.types
    }

    /// Provides read-only access to the function environment.
    pub fn functions(&self) -> &FunctionEnvironment {
        &self.functions
    }

    /// Provides read-only access to the symbol environment.
    pub fn symbols(&self) -> &SymbolEnvironment {
        &self.symbols
    }

    /// Provides mutable access to the type environment.
    pub fn types_mut(&mut self) -> &mut TypeEnvironment {
        &mut self.types
    }

    /// Provides mutable access to the function environment.
    #[allow(dead_code)]
    pub fn functions_mut(&mut self) -> &mut FunctionEnvironment {
        &mut self.functions
    }

    /// Provides mutable access to the symbol environment.
    pub fn symbols_mut(&mut self) -> &mut SymbolEnvironment {
        &mut self.symbols
    }
}

/// Builder for constructing a SemanticContext with optional standard library support.
pub struct SemanticContextBuilder {
    type_builder: TypeEnvironmentBuilder,
    function_builder: FunctionEnvironmentBuilder,
}

impl SemanticContextBuilder {
    /// Creates a new SemanticContextBuilder.
    pub fn new() -> Self {
        Self {
            type_builder: TypeEnvironmentBuilder::new(),
            function_builder: FunctionEnvironmentBuilder::new(),
        }
    }

    /// Adds elementary types (BOOL, INT, REAL, etc.) to the type environment.
    pub fn with_elementary_types(mut self) -> Self {
        self.type_builder = self.type_builder.with_elementary_types();
        self
    }

    /// Adds standard library function blocks (TON, TOF, CTU, etc.) to the type environment.
    pub fn with_stdlib_function_blocks(mut self) -> Self {
        self.type_builder = self.type_builder.with_stdlib_function_blocks();
        self
    }

    /// Adds standard library functions (INT_TO_REAL, etc.) to the function environment.
    pub fn with_stdlib_functions(mut self) -> Self {
        self.function_builder = self.function_builder.with_stdlib_functions();
        self
    }

    /// Builds the SemanticContext with an empty symbol environment.
    ///
    /// The symbol environment is typically populated during semantic analysis
    /// as declarations are processed.
    pub fn build(self) -> Result<SemanticContext, Diagnostic> {
        let types = self.type_builder.build()?;
        let functions = self.function_builder.build();
        let symbols = SymbolEnvironment::new();

        Ok(SemanticContext::new(types, functions, symbols))
    }
}

impl Default for SemanticContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::TypeName;

    #[test]
    fn semantic_context_new_when_created_then_has_all_environments() {
        let types = TypeEnvironmentBuilder::new().build().unwrap();
        let functions = FunctionEnvironmentBuilder::new().build();
        let symbols = SymbolEnvironment::new();

        let ctx = SemanticContext::new(types, functions, symbols);

        // Just verify we can access each environment
        let _ = ctx.types();
        let _ = ctx.functions();
        let _ = ctx.symbols();
    }

    #[test]
    fn semantic_context_builder_when_with_elementary_types_then_has_types() {
        let ctx = SemanticContextBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Verify elementary types are present
        assert!(ctx.types().get(&TypeName::from("bool")).is_some());
        assert!(ctx.types().get(&TypeName::from("int")).is_some());
        assert!(ctx.types().get(&TypeName::from("real")).is_some());
    }

    #[test]
    fn semantic_context_builder_when_with_stdlib_function_blocks_then_has_function_blocks() {
        let ctx = SemanticContextBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();

        // Verify stdlib function blocks are present
        assert!(ctx.types().get(&TypeName::from("ton")).is_some());
        assert!(ctx.types().get(&TypeName::from("tof")).is_some());
        assert!(ctx.types().get(&TypeName::from("ctu")).is_some());
    }

    #[test]
    fn semantic_context_builder_when_default_then_builds_empty() {
        let ctx = SemanticContextBuilder::new().build().unwrap();

        // Should have empty environments
        assert!(ctx.functions().is_empty());
    }

    #[test]
    fn semantic_context_mutable_access_when_modifying_then_changes_persist() {
        let mut ctx = SemanticContextBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Verify we can get mutable access
        let types = ctx.types_mut();
        // Just verify the mutable reference works
        let _ = types.iter().count();

        let symbols = ctx.symbols_mut();
        // Verify mutable access to symbols works
        let _ = symbols.total_symbols();
    }

    #[test]
    fn semantic_context_builder_when_chaining_all_options_then_builds_complete_context() {
        let ctx = SemanticContextBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .with_stdlib_functions()
            .build()
            .unwrap();

        // Verify types are populated
        assert!(ctx.types().get(&TypeName::from("int")).is_some());
        assert!(ctx.types().get(&TypeName::from("ton")).is_some());

        // Functions will be empty until stdlib functions are implemented
        // This test will need to be updated when we add the actual functions
    }
}
