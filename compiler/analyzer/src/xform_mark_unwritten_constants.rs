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
//! The transform also records, in the symbol environment, the value each
//! constant holds for the whole run: the literal of its initializer, for a
//! declared and an inferred constant alike
//! ([`SymbolEnvironment::constant_value`]). A rule that judges a value the
//! program fixes before it runs reads it there, rather than finding the
//! declaration itself.
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
use crate::symbol_environment::{ScopeKind, ScopeTracker, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;
use crate::write_collector::{collect, may_be_constant, Writes};

/// Marks every never-written variable in `lib` as `CONSTANT`, and records
/// in `symbol_environment` the value each constant holds.
///
/// Infallible: the transform only ever adds a qualifier, and it adds one only
/// where the semantic rules for `CONSTANT` declarations are known to pass.
pub fn apply(
    lib: Library,
    type_environment: &TypeEnvironment,
    function_environment: &FunctionEnvironment,
    symbol_environment: &mut SymbolEnvironment,
) -> Library {
    let collected = collect(
        &lib,
        type_environment,
        function_environment,
        symbol_environment,
    );
    let mut marker = Marker::new(collected.written, &collected.globals, symbol_environment);
    let Ok(lib) = marker.fold_library(lib);
    lib
}

/// The literal `init` initializes a variable with, when it is one literal
/// of an elementary type: the value a constant with that initializer holds.
fn literal_initial_value(init: &InitialValueAssignmentKind) -> Option<&ConstantKind> {
    match init {
        InitialValueAssignmentKind::Simple(simple) => simple.initial_value.as_ref(),
        _ => None,
    }
}

/// Applies the verdicts: which declarations become `CONSTANT`.
struct Marker<'a> {
    written: Writes,
    /// Global names whose `VAR_GLOBAL` and `VAR_EXTERNAL` declarations are
    /// all marked together.
    constant_globals: HashSet<Id>,
    /// The declaration the fold is inside.
    scope: ScopeTracker,
    /// Where the value each constant holds is recorded.
    symbols: &'a mut SymbolEnvironment,
}

impl<'a> Marker<'a> {
    fn new(
        written: Writes,
        globals: &HashMap<Id, bool>,
        symbols: &'a mut SymbolEnvironment,
    ) -> Self {
        let constant_globals = globals
            .iter()
            .filter(|(name, qualifies)| **qualifies && !written.contains(&ScopeKind::Global, name))
            .map(|(name, _)| name.clone())
            .collect();
        Marker {
            written,
            constant_globals,
            scope: ScopeTracker::default(),
            symbols,
        }
    }

    /// Records the value `decl` holds for the whole run: its initial value,
    /// when `decl` is `CONSTANT`, declared or just marked, and that value
    /// is one literal.
    fn record_value(&mut self, decl: &VarDecl) {
        if decl.qualifier != DeclarationQualifier::Constant {
            return;
        }
        let VariableIdentifier::Symbol(name) = &decl.identifier else {
            return;
        };
        if let Some(value) = literal_initial_value(&decl.initializer) {
            self.symbols
                .record_constant_value(name, &self.scope.current(), value.clone());
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
                may_be_constant(&decl.initializer)
                    && !self.written.contains(&self.scope.current(), name)
            }
            VariableType::Global | VariableType::External => self.constant_globals.contains(name),
            VariableType::Input
            | VariableType::Output
            | VariableType::InOut
            | VariableType::Access => false,
        }
    }
}

impl Fold<Infallible> for Marker<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_var_decl(&mut self, mut node: VarDecl) -> Result<VarDecl, Infallible> {
        if self.should_mark(&node) {
            node.qualifier = DeclarationQualifier::Constant;
        }
        self.record_value(&node);
        Ok(node)
    }
}

#[cfg(test)]
mod tests;
