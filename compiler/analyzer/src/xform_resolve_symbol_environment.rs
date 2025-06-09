//! "Rule" that builds the symbol table.

use ironplc_dsl::{common::{Library, VariableType}, core::Id, diagnostic::Diagnostic, visitor::Visitor};
use log::debug;

use crate::{result::SemanticResult, symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind}, type_environment::TypeEnvironment};

pub fn apply(lib: Library, _type_environment: &TypeEnvironment, symbol_environment: &mut SymbolEnvironment) -> Result<Library, Vec<Diagnostic>> {
    apply_impl(&lib, symbol_environment)?;

    Ok(lib)
}

pub fn apply_impl(lib: &Library, env: &mut SymbolEnvironment) -> SemanticResult {
    let mut resolver = SymbolEnvironmentResolver {
        env,
        scope: None,
    };
    let result = resolver.walk(lib).map_err(|e| vec![e]);

    debug!("{:?}", resolver.env);

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

struct SymbolEnvironmentResolver<'a>{
    env: &'a mut SymbolEnvironment,
    scope: Option<Id>,
}

impl<'a> SymbolEnvironmentResolver<'a> {
    fn current_scope(&self) -> ScopeKind {
        match &self.scope {
            Some(name) => ScopeKind::Named(name.clone()),
            None => ScopeKind::Global,
        }
    }
}

impl<'a> Visitor<Diagnostic> for SymbolEnvironmentResolver<'a> {
    type Value = ();

    // TODO fn visit_program_access_decl

    fn visit_var_decl(&mut self,node: &ironplc_dsl::common::VarDecl) -> Result<Self::Value,Diagnostic> {
        // Some types of variables are references to other variables.
        // TODO Stage these so that we can update references to them but not actually declare them here
        // (or otherwise distinguish between a declaration and a reference)
        if node.var_type != VariableType::External {
            match &node.identifier {
                ironplc_dsl::common::VariableIdentifier::Symbol(id) => {
                    self.env.insert(id, SymbolKind::SymbolicVariable, &self.current_scope())?;
                },
                ironplc_dsl::common::VariableIdentifier::Direct(_) => {
                    // TODO
                },
            }
        }
        node.recurse_visit(self)
    }

    fn visit_edge_var_decl(
        &mut self,
        node: &ironplc_dsl::common::EdgeVarDecl,
    ) -> Result<Self::Value, Diagnostic> {
        self.env.insert(&node.identifier, SymbolKind::SymbolicVariable, &self.current_scope())?;
        node.recurse_visit(self)
    }

    fn visit_function_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.clone());

        self.env.insert(&node.name, SymbolKind::Function, &ScopeKind::Global)?;

        let result = node.recurse_visit(self);
        self.scope = None;

        result
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &ironplc_dsl::common::FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.name.clone());
        self.env.insert(&node.name.name, SymbolKind::FunctionBlock, &ScopeKind::Global)?;
        let result = node.recurse_visit(self);
        self.scope = None;
        
        result
    }

    fn visit_program_declaration(
        &mut self,
        node: &ironplc_dsl::common::ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.scope = Some(node.name.clone());
        self.env.insert(&node.name, SymbolKind::Program, &ScopeKind::Global)?;
        let result = node.recurse_visit(self);
        self.scope = None;
        
        result
    }

    // TODO should this handle parameters?
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::Id;

    use crate::{xform_resolve_symbol_environment::apply_impl, symbol_environment::{ScopeKind, SymbolEnvironment, SymbolKind}, test_helpers::parse_and_resolve_types};

    #[test]
    fn apply_when_var_init_valid_enum_value_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let mut env = SymbolEnvironment::new();
        let result = apply_impl(&library, &mut env);

        assert!(result.is_ok());
        let attributes = env.get(&Id::from("LEVEL"), &ScopeKind::Named(Id::from("LOGGER"))).unwrap();
        assert_eq!(attributes.kind, SymbolKind::SymbolicVariable);

        let attributes = env.get(&Id::from("LOGGER"), &ScopeKind::Global).unwrap();
        assert_eq!(attributes.kind, SymbolKind::FunctionBlock);
    }
}