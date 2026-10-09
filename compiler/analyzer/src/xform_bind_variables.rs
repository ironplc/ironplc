//! Transformation pass that binds every variable reference to the
//! declaration it names.
//!
//! The symbol environment already answers "which declaration does this name
//! mean from here" (`SymbolEnvironment::find`: the enclosing scopes innermost
//! first, then the function blocks the outermost one `EXTENDS`, then the
//! global scope). This pass records that answer, as a [`DeclId`], on every
//! reference: a [`NamedVariable`] -- including the record of `s.f` and the
//! array of `a[i]` -- the control variable of a `FOR`, the instance of a
//! function block call and the instance a method is called on.
//!
//! A back end then keys storage by the declaration and never resolves a name
//! (ADR-0058, `specs/design/variable-binding.md`). Deciding what a name means
//! in two places is how the two came to disagree.
//!
//! A reference through `VAR_EXTERNAL` binds to the global the external names,
//! so every reference to one global variable carries one id: aliasing is
//! decided here, not by a back end. A name that declares no variable is left
//! unbound; the rules that check references report it.
use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::decl_id::DeclId;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;

use crate::symbol_environment::{ScopeKind, ScopeTracker, SymbolEnvironment, SymbolKind};

/// Binds every variable reference in `lib` from `symbols`. Cannot fail.
pub fn apply(lib: Library, symbols: &SymbolEnvironment) -> Library {
    let mut binder = VariableBinder {
        symbols,
        scope: ScopeTracker::default(),
    };
    let Ok(lib) = binder.fold_library(lib);
    lib
}

struct VariableBinder<'a> {
    symbols: &'a SymbolEnvironment,
    scope: ScopeTracker,
}

impl VariableBinder<'_> {
    /// The declaration `name` refers to from the current scope, or `None`
    /// when it names no variable.
    fn bind(&self, name: &Id) -> Option<DeclId> {
        let info = self.symbols.find(name, &self.scope.current())?;
        if !is_variable(&info.kind) {
            return None;
        }
        if info.is_external {
            // The external names a global of the same name. Without one the
            // reference keeps the external's own declaration, which nothing
            // gives storage to.
            let global = self
                .symbols
                .find(name, &ScopeKind::Global)
                .filter(|global| is_variable(&global.kind) && !global.is_external);
            return global.and_then(|global| global.decl_id).or(info.decl_id);
        }
        info.decl_id
    }
}

/// Whether a symbol of `kind` is a variable a reference can name.
fn is_variable(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Variable
            | SymbolKind::Parameter
            | SymbolKind::OutputParameter
            | SymbolKind::InOutParameter
            | SymbolKind::EdgeVariable
            | SymbolKind::ResultVariable
    )
}

impl Fold<Infallible> for VariableBinder<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_named_variable(&mut self, node: NamedVariable) -> Result<NamedVariable, Infallible> {
        let decl_id = self.bind(&node.name);
        Ok(NamedVariable { decl_id, ..node })
    }

    fn fold_for(&mut self, node: For) -> Result<For, Infallible> {
        let control_decl_id = self.bind(&node.control);
        let node = node.recurse_fold(self)?;
        Ok(For {
            control_decl_id,
            ..node
        })
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Infallible> {
        let instance_decl_id = self.bind(&node.var_name);
        let node = node.recurse_fold(self)?;
        Ok(FbCall {
            instance_decl_id,
            ..node
        })
    }

    fn fold_method_call(&mut self, node: MethodCall) -> Result<MethodCall, Infallible> {
        let receiver_decl_id = match &node.receiver {
            MethodReceiver::Instance(name) => self.bind(name),
            MethodReceiver::SelfRef(_) => None,
        };
        let node = node.recurse_fold(self)?;
        Ok(MethodCall {
            receiver_decl_id,
            ..node
        })
    }
}

#[cfg(test)]
mod tests;
