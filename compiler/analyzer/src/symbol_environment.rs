//! The symbol table. When complete, has each item instantiated
//! by the application.
use std::collections::HashMap;

use ironplc_dsl::{core::{Id, Located, SourceSpan}, diagnostic::{Diagnostic, Label}};
use ironplc_problems::Problem;

#[derive(Clone, Debug, PartialEq)]
pub enum SymbolKind {
    SymbolicVariable,
    Function,
    FunctionBlock,
    Program,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScopeKind {
    Global,
    Named(Id)
}

#[derive(Debug)]
pub struct SymbolAttributes {
    pub span: SourceSpan,
    pub kind: SymbolKind,
    pub scope: ScopeKind,
}

#[derive(Debug)]
pub struct SymbolEnvironment {
    table: HashMap<String, SymbolAttributes>,
}

impl SymbolEnvironment {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn insert(&mut self, identifier: &Id, kind: SymbolKind, scope: &ScopeKind) -> Result<(), Diagnostic> {
        let name = self.mangle_name(identifier, scope);
        self.table.insert(name, SymbolAttributes {
            span: identifier.span(),
            kind,
            scope: scope.clone(),
        }).map_or_else(
            || Ok(()),
            |existing| {
                Err(Diagnostic::problem(
                    Problem::SymbolDeclDuplicated,
                    Label::span(identifier.span(), "Duplicate declaration"),
                )
                .with_secondary(Label::span(existing.span, "First declaration")))
            },
        )
    }

    pub fn get(&self, identifier: &Id, scope: &ScopeKind) -> Option<&SymbolAttributes> {
        let name = self.mangle_name(identifier, scope);
        self.table.get(&name)
    }

    fn mangle_name(&self, identifier: &Id, scope: &ScopeKind) -> String {
        match scope {
            ScopeKind::Global => {
                identifier.lower_case.clone()
            },
            ScopeKind::Named(scope_id) => {
                let mut name = scope_id.lower_case().clone();
                // '$' is an arbitrary character that is not a valid identifier
                name.push('$');
                name.push_str(identifier.lower_case());
                name
            },
        }
    }
}

pub(crate) struct SymbolEnvironmentBuilder {
}

impl SymbolEnvironmentBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self) -> Result<SymbolEnvironment, Diagnostic> {
        Ok(SymbolEnvironment::new())
    }
}
