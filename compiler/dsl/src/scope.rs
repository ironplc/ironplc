//! Declarations that open a lexical scope.
//!
//! A scope-bearing declaration is one whose variable declarations are
//! visible only inside its own body: a `FUNCTION`, a `FUNCTION_BLOCK`, a
//! `PROGRAM`, a `METHOD`. Rather than each analysis pass deciding for
//! itself which node kinds those are — and silently getting no scope for
//! the kinds it forgot — the traversal opens and closes the scope, and a
//! pass that cares implements
//! [`Visitor::enter_scope`]/[`Visitor::exit_scope`] (or the [`Fold`]
//! equivalents) exactly once.
//!
//! Marking a declaration `#[recurse(scope)]` is what makes the derived
//! `recurse_visit`/`recurse_fold` call those hooks. The derive refuses to
//! compile a declaration that holds `variables: Vec<VarDecl>` and says
//! neither `#[recurse(scope)]` nor `#[recurse(no_scope)]`, so a new
//! POU-like construct cannot quietly arrive without the question being
//! answered.
//!
//! [`Visitor::enter_scope`]: crate::visitor::Visitor::enter_scope
//! [`Visitor::exit_scope`]: crate::visitor::Visitor::exit_scope
//! [`Fold`]: crate::fold::Fold

use crate::common::{
    FunctionBlockDeclaration, FunctionDeclaration, MethodDeclaration, ProgramDeclaration,
};

/// The declaration that opened the scope the traversal is entering.
///
/// Passes match on this without a wildcard arm: the kinds do not agree on
/// what a scope contains — a function seeds its own name as the implicit
/// result variable, a function block additionally seeds the fields it
/// inherits through `EXTENDS`, a method seeds its own name only when it
/// has a return type — so a new variant must be a compile error
/// everywhere that discriminates, not a silently skipped case.
#[derive(Debug)]
pub enum ScopeNode<'a> {
    Function(&'a FunctionDeclaration),
    FunctionBlock(&'a FunctionBlockDeclaration),
    Program(&'a ProgramDeclaration),
    Method(&'a MethodDeclaration),
}

/// Implemented by every declaration marked `#[recurse(scope)]`.
///
/// The derived traversal calls this to describe the scope it is opening.
/// The trait supplies the content; the enforcement that a pass handles
/// every kind is [`ScopeNode`]'s exhaustiveness.
pub trait ScopeBearing {
    fn as_scope_node(&self) -> ScopeNode<'_>;
}

impl ScopeBearing for FunctionDeclaration {
    fn as_scope_node(&self) -> ScopeNode<'_> {
        ScopeNode::Function(self)
    }
}

impl ScopeBearing for FunctionBlockDeclaration {
    fn as_scope_node(&self) -> ScopeNode<'_> {
        ScopeNode::FunctionBlock(self)
    }
}

impl ScopeBearing for ProgramDeclaration {
    fn as_scope_node(&self) -> ScopeNode<'_> {
        ScopeNode::Program(self)
    }
}

impl ScopeBearing for MethodDeclaration {
    fn as_scope_node(&self) -> ScopeNode<'_> {
        ScopeNode::Method(self)
    }
}
