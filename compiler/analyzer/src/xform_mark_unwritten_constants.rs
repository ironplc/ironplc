//! Transform that marks variables the program never writes as `CONSTANT`.
//!
//! A variable with an initializer and no write anywhere in the library holds
//! its initial value for the whole run. Making that explicit lets every later
//! stage -- the semantic rules and, above all, code generation -- treat
//! "declared `CONSTANT`" and "never written" as one thing, so a constant fold
//! (such as `LEN` of a string that never changes) has a single case to
//! recognize.
//!
//! The transform is conservative: it may leave a variable unmarked that is in
//! fact never written, but it never marks one that is. A write is resolved to
//! the declaration it reaches -- through the symbol environment for the
//! scopes a name is visible in, the `EXTENDS` chain for an inherited field,
//! and the instance's type for a member -- and only that declaration is
//! blocked. A write through a path the transform cannot resolve to one
//! declaration (a member of an array element, a `VAR_CONFIG` path) blocks
//! every declaration of that name instead.
//!
//! See `specs/design/constant-variable-inference.md`.
//!
//! ## Before
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     greeting : STRING := 'Hello';
//!     count : INT := 0;
//! END_VAR
//!     count := LEN(greeting);
//! END_PROGRAM
//! ```
//!
//! ## After
//!
//! ```ignore
//! PROGRAM main
//! VAR CONSTANT
//!     greeting : STRING := 'Hello';
//! END_VAR
//! VAR
//!     count : INT := 0;
//! END_VAR
//!     count := LEN(greeting);
//! END_PROGRAM
//! ```
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;

use crate::function_environment::FunctionEnvironment;
use crate::symbol_environment::{ScopeKind, ScopePath, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;
use crate::write_collector::{collect, may_be_constant, Writes};

/// Marks every never-written variable in `lib` as `CONSTANT`.
///
/// Infallible: the transform only ever adds a qualifier, and it adds one only
/// where the semantic rules for `CONSTANT` declarations are known to pass.
pub fn apply(
    lib: Library,
    type_environment: &TypeEnvironment,
    function_environment: &FunctionEnvironment,
    symbol_environment: &SymbolEnvironment,
) -> Library {
    let collected = collect(
        &lib,
        type_environment,
        function_environment,
        symbol_environment,
    );
    let mut marker = Marker::new(collected.written, &collected.globals);
    let Ok(lib) = marker.fold_library(lib);
    lib
}
/// Applies the verdicts: which declarations become `CONSTANT`.
struct Marker {
    written: Writes,
    /// Global names whose `VAR_GLOBAL` and `VAR_EXTERNAL` declarations are
    /// all marked together.
    constant_globals: HashSet<Id>,
    /// The declarations the fold is inside, outermost first.
    scope: Vec<Id>,
}

impl Marker {
    fn new(written: Writes, globals: &HashMap<Id, bool>) -> Self {
        let constant_globals = globals
            .iter()
            .filter(|(name, qualifies)| **qualifies && !written.contains(&ScopeKind::Global, name))
            .map(|(name, _)| name.clone())
            .collect();
        Marker {
            written,
            constant_globals,
            scope: Vec::new(),
        }
    }

    fn should_mark(&self, decl: &VarDecl) -> bool {
        if decl.qualifier != DeclarationQualifier::Unspecified {
            return false;
        }
        let VariableIdentifier::Symbol(name) = &decl.identifier else {
            return false;
        };
        match decl.var_type {
            VariableType::Var | VariableType::VarTemp => {
                let scope = match self.scope.first() {
                    None => ScopeKind::Global,
                    Some(_) => ScopeKind::Named(ScopePath::new(self.scope.clone())),
                };
                may_be_constant(&decl.initializer) && !self.written.contains(&scope, name)
            }
            VariableType::Global | VariableType::External => self.constant_globals.contains(name),
            VariableType::Input
            | VariableType::Output
            | VariableType::InOut
            | VariableType::Access => false,
        }
    }
}

impl Fold<Infallible> for Marker {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.push(match node {
            ScopeNode::Function(node) => node.name.clone(),
            ScopeNode::FunctionBlock(node) => node.name.name.clone(),
            ScopeNode::Program(node) => node.name.clone(),
            ScopeNode::Method(node) => node.name.clone(),
        });
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.pop();
    }

    fn fold_var_decl(&mut self, mut node: VarDecl) -> Result<VarDecl, Infallible> {
        if self.should_mark(&node) {
            node.qualifier = DeclarationQualifier::Constant;
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests;
