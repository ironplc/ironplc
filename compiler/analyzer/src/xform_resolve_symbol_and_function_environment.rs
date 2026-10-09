//! Transform that builds the symbol table and function environment.
//!
//! This transform populates:
//! - `SymbolEnvironment`: tracks declarations and scoping (variables, parameters, types, POUs)
//! - `FunctionEnvironment`: tracks function signatures for call validation
//!
//! Function signatures store type names (not resolved types) to allow building
//! complete signatures even when type resolution fails. Types are resolved
//! on-demand during validation via TypeEnvironment.
//!
//! A repeated declaration name is diagnosed here, by the environments: a
//! function repeating a function is `P4016` from the function environment, a
//! type repeating a type is `P2007` and any other pair of global declarations
//! is `P4013` from the symbol environment. A function and a symbol of one
//! name live in different environments, so this transform checks that pair
//! itself. The first declaration is kept and analysis continues on it; the
//! diagnostics are collected rather than aborting the walk.

use ironplc_dsl::{
    common::{
        AddressAssignment, InitialValueAssignmentKind, Library, LocationPrefix, SizePrefix,
        TypeName, TypeReference, VariableType,
    },
    core::{Id, Located},
    diagnostic::Diagnostic,
    scope::{ScopeBearing, ScopeNode},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use log::debug;
use std::convert::Infallible;

use crate::{
    function_environment::{FunctionEnvironment, FunctionSignature},
    semantic_type::SemanticFunctionParameter,
    symbol_environment::{
        duplicate_declaration, ScopeKind, ScopeTracker, SymbolEnvironment, SymbolInfo, SymbolKind,
    },
    type_environment::TypeEnvironment,
};

/// Populates the environments from `lib`. Always keeps the library: the
/// diagnostics are the repeated declaration names, and analysis continues
/// on the first declaration of each.
///
/// A function's or method's result variable records the id of its return
/// type from `type_environment`.
pub fn apply(
    lib: Library,
    symbol_environment: &mut SymbolEnvironment,
    function_environment: &mut FunctionEnvironment,
    type_environment: &TypeEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let diagnostics = resolve(
        &lib,
        symbol_environment,
        function_environment,
        Some(type_environment),
    );
    Ok((lib, diagnostics))
}

/// Populates the environments from `lib` without a type environment, so
/// result variables record no type id.
#[cfg(test)]
pub fn apply_impl(
    lib: &Library,
    symbol_env: &mut SymbolEnvironment,
    function_env: &mut FunctionEnvironment,
) -> Vec<Diagnostic> {
    resolve(lib, symbol_env, function_env, None)
}

fn resolve(
    lib: &Library,
    symbol_env: &mut SymbolEnvironment,
    function_env: &mut FunctionEnvironment,
    type_env: Option<&TypeEnvironment>,
) -> Vec<Diagnostic> {
    let mut resolver = EnvironmentResolver {
        symbol_env,
        function_env,
        type_env,
        scope: ScopeTracker::default(),
        diagnostics: Vec::new(),
    };
    let Ok(()) = resolver.walk(lib);

    debug!("{:?}", resolver.symbol_env);

    resolver.diagnostics
}

struct EnvironmentResolver<'a> {
    symbol_env: &'a mut SymbolEnvironment,
    function_env: &'a mut FunctionEnvironment,
    /// Resolves the return type of a result variable, when given.
    type_env: Option<&'a TypeEnvironment>,
    /// The declaration the traversal is currently inside.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> EnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        self.scope.current()
    }

    /// Keeps the diagnostic an environment returned for a repeated name.
    fn record(&mut self, result: Result<(), Diagnostic>) {
        if let Err(diagnostic) = result {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Declares the implicit result variable `name` of the function or
    /// method `node` that the traversal is about to enter, in that
    /// declaration's own scope. A variable the declaration declares with
    /// the same name replaces it.
    fn declare_result_variable(&mut self, node: ScopeNode<'_>, name: &Id, return_type: &TypeName) {
        let scope = self.scope.scope_of(&node);
        let type_id = self.type_env.and_then(|types| types.id_of(return_type));
        let info =
            SymbolInfo::new(SymbolKind::ResultVariable, scope, name.span()).with_type_id(type_id);
        let result = self.symbol_env.insert_info(name, info);
        self.record(result);
    }

    /// Declares `name` in the global scope. A function already holds the
    /// name in the other environment, so that pair is checked here; every
    /// other repeat is the symbol environment's to report.
    fn declare_global(&mut self, name: &Id, kind: SymbolKind) {
        self.declare_global_symbol(name, SymbolInfo::new(kind, ScopeKind::Global, name.span()));
    }

    /// Declares `name` in the global scope as [`Self::declare_global`]
    /// does, with the symbol `info` describes.
    fn declare_global_symbol(&mut self, name: &Id, info: SymbolInfo) {
        if let Some(function) = self.function_env.get(name) {
            self.diagnostics.push(duplicate_declaration(
                Problem::PouDeclNameDuplicated,
                name,
                function.span.clone(),
            ));
            return;
        }
        let result = self.symbol_env.insert_info(name, info);
        self.record(result);
    }
}

impl<'a> Visitor<Infallible> for EnvironmentResolver<'a> {
    type Value = ();

    /// Pushes the declaration the traversal is entering onto the scope
    /// stack, so the variables it declares are recorded against its own
    /// path rather than the enclosing declaration's.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    // TODO fn visit_program_access_decl

    fn visit_var_decl(
        &mut self,
        node: &ironplc_dsl::common::VarDecl,
    ) -> Result<Self::Value, Infallible> {
        match &node.identifier {
            ironplc_dsl::common::VariableIdentifier::Symbol(id) => {
                let result = self.symbol_env.insert_variable(
                    id,
                    &self.current_scope(),
                    node.var_type.clone(),
                    node.qualifier.clone(),
                    node.type_id,
                    None,
                );
                self.record(result);
            }
            ironplc_dsl::common::VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    let address = format_address(&direct.address_assignment);
                    let result = self.symbol_env.insert_variable(
                        name,
                        &self.current_scope(),
                        node.var_type.clone(),
                        node.qualifier.clone(),
                        node.type_id,
                        Some(address),
                    );
                    self.record(result);
                }
            }
        }
        node.recurse_visit(self)
    }

    fn visit_edge_var_decl(
        &mut self,
        node: &ironplc_dsl::common::EdgeVarDecl,
    ) -> Result<Self::Value, Infallible> {
        let result = self.symbol_env.insert(
            &node.identifier,
            SymbolKind::EdgeVariable,
            &self.current_scope(),
        );
        self.record(result);
        node.recurse_visit(self)
    }

    fn visit_function_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_result_variable(
            node.as_scope_node(),
            &node.name,
            &node.return_type.to_type_name(),
        );

        // Build function signature for function environment
        // (Functions are tracked in FunctionEnvironment, not SymbolEnvironment)
        // Collect parameters (INPUT, OUTPUT, INOUT variables)
        //
        // Note: We store TypeName references, not resolved types. This allows
        // building complete signatures even when type resolution fails. Types
        // are resolved on-demand during validation via TypeEnvironment.
        let mut parameters = Vec::new();
        for var_decl in &node.variables {
            if !var_decl.var_type.is_parameter() {
                continue;
            }

            // Get parameter name
            let param_name = match &var_decl.identifier {
                ironplc_dsl::common::VariableIdentifier::Symbol(id) => id.clone(),
                ironplc_dsl::common::VariableIdentifier::Direct(_) => continue,
            };

            // Get parameter type name (store as TypeName, resolve later).
            // REF_TO parameters report TypeReference::Inline from type_name(),
            // so we check the initializer directly to extract the referenced type.
            let (param_type, is_reference) = match var_decl.type_name() {
                TypeReference::Named(type_name) => (type_name, false),
                TypeReference::Inline => match &var_decl.initializer {
                    InitialValueAssignmentKind::Reference(ref_init) => {
                        match &ref_init.target {
                            crate::ironplc_dsl::common::ReferenceTarget::Named(tn) => {
                                (tn.clone(), true)
                            }
                            crate::ironplc_dsl::common::ReferenceTarget::Array(subranges) => {
                                // REF_TO ARRAY[...] OF T — use the element type name
                                // so the parameter is registered in the function signature.
                                (subranges.type_name.to_type_name(), true)
                            }
                        }
                    }
                    _ => continue,
                },
                _ => continue,
            };

            parameters.push(SemanticFunctionParameter {
                name: param_name,
                param_type,
                is_input: var_decl.var_type == VariableType::Input,
                is_output: var_decl.var_type == VariableType::Output,
                is_inout: var_decl.var_type == VariableType::InOut,
                is_reference,
            });
        }

        // Store return type as TypeName (resolve later during validation)
        let return_type = Some(node.return_type.clone());

        // A type, function block, program or configuration already holds
        // the name in the symbol environment: the function repeats it, and
        // that earlier declaration is kept. A function repeating a function
        // is the function environment's own P4016.
        if let Some(existing) = self.symbol_env.find(&node.name, &ScopeKind::Global) {
            if existing.scope == ScopeKind::Global
                && matches!(
                    existing.kind,
                    SymbolKind::Type
                        | SymbolKind::FunctionBlock
                        | SymbolKind::Program
                        | SymbolKind::Configuration
                )
            {
                self.diagnostics.push(duplicate_declaration(
                    Problem::PouDeclNameDuplicated,
                    &node.name,
                    existing.span.clone(),
                ));
                return node.recurse_visit(self);
            }
        }

        // Build and insert function signature
        let signature =
            FunctionSignature::new(node.name.clone(), return_type, parameters, node.name.span());
        let result = self.function_env.insert(signature);
        self.record(result);

        node.recurse_visit(self)
    }

    fn visit_method_declaration(
        &mut self,
        node: &ironplc_dsl::oop::MethodDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // A method without a return type is a procedure: it has no result
        // to assign.
        if let Some(return_type) = &node.return_type {
            self.declare_result_variable(
                node.as_scope_node(),
                &node.name,
                &return_type.to_type_name(),
            );
        }
        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let name = &node.name.name;
        self.declare_global_symbol(
            name,
            SymbolInfo::new(SymbolKind::FunctionBlock, ScopeKind::Global, name.span())
                .with_abstract(node.is_abstract())
                .with_extends(node.oop.as_ref().and_then(|oop| oop.base.clone())),
        );
        let result = node.recurse_visit(self);
        // A property is declared in the block's scope after its variables,
        // so that a property named like a variable is the repeat, as
        // TwinCAT reports it.
        let block_scope = self.scope.scope_of(&node.as_scope_node());
        for property in &node.properties {
            let result = self
                .symbol_env
                .insert(&property.name, SymbolKind::Property, &block_scope);
            self.record(result);
        }
        result
    }

    fn visit_program_declaration(
        &mut self,
        node: &ironplc_dsl::common::ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Program);
        node.recurse_visit(self)
    }

    fn visit_configuration_declaration(
        &mut self,
        node: &ironplc_dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Configuration);
        node.recurse_visit(self)
    }

    fn visit_interface_declaration(
        &mut self,
        node: &ironplc_dsl::common::InterfaceDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.declare_global(&node.name, SymbolKind::Type);
        node.recurse_visit(self)
    }

    fn visit_data_type_declaration_kind(
        &mut self,
        node: &ironplc_dsl::common::DataTypeDeclarationKind,
    ) -> Result<Self::Value, Infallible> {
        match node {
            ironplc_dsl::common::DataTypeDeclarationKind::Simple(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Structure(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Enumeration(_) => {
                // Declared by `visit_enumeration_declaration`, which the
                // recursion below reaches and which also records the values.
                // Declaring it here too would report the type as its own
                // repeat.
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Array(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Subrange(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::String(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
            ironplc_dsl::common::DataTypeDeclarationKind::LateBound(_) => {
                // Skip late-bound types for now
            }
            ironplc_dsl::common::DataTypeDeclarationKind::StructureInitialization(_) => {
                // Skip structure initializations for now
            }
            ironplc_dsl::common::DataTypeDeclarationKind::Reference(decl) => {
                self.declare_global(&decl.type_name.name, SymbolKind::Type);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_structure_element_declaration(
        &mut self,
        node: &ironplc_dsl::common::StructureElementDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let result = self.symbol_env.insert(
            &node.name,
            SymbolKind::StructureElement,
            &self.current_scope(),
        );
        self.record(result);
        node.recurse_visit(self)
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &ironplc_dsl::common::EnumerationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // Add the enumeration type itself
        self.declare_global(&node.type_name.name, SymbolKind::Type);

        // Add each enumeration value
        if let ironplc_dsl::common::SpecificationKind::Inline(values) = &node.spec_init.spec {
            for value in &values.values {
                self.symbol_env
                    .insert_enumeration_value(&value.value, &node.type_name);
            }
        }

        node.recurse_visit(self)
    }

    // TODO should this handle parameters?
}

fn format_address(addr: &AddressAssignment) -> String {
    let loc = match addr.location {
        LocationPrefix::I => "I",
        LocationPrefix::Q => "Q",
        LocationPrefix::M => "M",
    };
    let size = match addr.size {
        SizePrefix::X => "X",
        SizePrefix::B => "B",
        SizePrefix::W => "W",
        SizePrefix::D => "D",
        SizePrefix::L => "L",
        SizePrefix::Nil | SizePrefix::Unspecified => "",
    };
    let parts: Vec<String> = addr.address.iter().map(|a| a.to_string()).collect();
    format!("%{loc}{size}{}", parts.join("."))
}

#[cfg(test)]
mod tests;
