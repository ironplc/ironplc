//! Lowering: from an analyzed library to the lowered program a backend
//! consumes (`specs/design/lowered-program.md`).
//!
//! The analyzer decides what a program means and records it in the analyzed
//! library. Lowering translates what the analyzer recorded into the nodes of
//! [`ironplc_ir::lowered`], desugars, and allocates ids. It decides nothing
//! whose outcome could make a program invalid, and reports no problem with
//! the program: only P9999 for a construct it cannot express yet, and P9998
//! for a compiler defect (REQ-LOW-lowering-109).
//!
//! While codegen still compiles most statements from the AST, lowering grows
//! one top-level statement at a time. [`Lowering::lower_statement`] answers
//! whether it supports a statement and every statement and expression inside
//! it. A statement it does not support yet is [`StatementLowering::Unsupported`]:
//! that is not a defect, and codegen compiles the statement from the AST.
//!
//! Lowering takes a [`CleanAnalysis`] and has no entry point that takes a
//! library without one (REQ-LOW-lowering-011).

// A match over a lowered enum names every variant (REQ-LOW-lowering-082).
#![deny(clippy::wildcard_enum_match_arm)]
#![allow(clippy::result_large_err)]

mod expr;
mod statement;
mod types;
mod variables;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use ironplc_analyzer::symbol_environment::DeclRef;
use ironplc_analyzer::CleanAnalysis;
use ironplc_dsl::common::{FunctionBlockDeclaration, FunctionDeclaration, ProgramDeclaration};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::StmtKind;
use ironplc_ir::lowered::{PouId, Stmt, TypeTable, VarId};

pub use variables::{Owner, Variable};

/// The body a statement belongs to.
#[derive(Clone, Copy, Debug)]
pub enum Body<'a> {
    Program(&'a ProgramDeclaration),
    FunctionBlock(&'a FunctionBlockDeclaration),
    Function(&'a FunctionDeclaration),
}

/// What lowering made of one statement.
#[derive(Clone, Debug)]
pub enum StatementLowering {
    /// The statement, with everything inside it, lowered.
    Lowered(Stmt),
    /// The statement, or something inside it, is not one lowering supports
    /// yet. Not a defect: the statement is compiled from the AST.
    Unsupported,
}

/// Lowers the statements of one analyzed library.
///
/// It keeps the tables a lowered program refers into, the types, variables
/// and POUs, and adds to them as the statements it lowers reach new ones, so
/// that every statement of one compilation shares one set of ids.
pub struct Lowering<'a> {
    analysis: &'a CleanAnalysis<'a>,
    types: TypeTable,
    /// The lowered type of each analyzer type, or `None` for one lowering
    /// cannot express yet.
    type_ids: HashMap<ironplc_dsl::type_id::TypeId, Option<ironplc_ir::lowered::TypeId>>,
    variables: Vec<Variable>,
    /// The variable of each declaration that is not a field.
    declared: HashMap<DeclRef, VarId>,
    pous: Vec<String>,
    pou_ids: HashMap<String, PouId>,
    /// What lowering knows of each body it has lowered a statement of.
    bodies: HashMap<PouId, Option<variables::BodyFrame>>,
}

impl<'a> Lowering<'a> {
    /// Starts lowering the library `analysis` holds.
    pub fn new(analysis: &'a CleanAnalysis<'a>) -> Self {
        Self {
            analysis,
            types: TypeTable::new(),
            type_ids: HashMap::new(),
            variables: Vec::new(),
            declared: HashMap::new(),
            pous: Vec::new(),
            pou_ids: HashMap::new(),
            bodies: HashMap::new(),
        }
    }

    /// Lowers `stmt`, a top-level statement of `body`, with every statement
    /// and expression inside it.
    ///
    /// Returns [`StatementLowering::Unsupported`] when lowering does not
    /// support the statement yet, and a P9998 when the analyzed library
    /// breaks an invariant lowering relies on.
    pub fn lower_statement(
        &mut self,
        body: Body<'a>,
        stmt: &StmtKind,
    ) -> Result<StatementLowering, Diagnostic> {
        let Some(frame) = self.frame(body) else {
            return Ok(StatementLowering::Unsupported);
        };
        Ok(match self.statement(&frame, stmt)? {
            Some(lowered) => StatementLowering::Lowered(lowered),
            None => StatementLowering::Unsupported,
        })
    }

    /// The types the lowered statements refer to.
    pub fn types(&self) -> &TypeTable {
        &self.types
    }

    /// The variable `id` names, or `None` if lowering allocated no `id`.
    pub fn variable(&self, id: VarId) -> Option<&Variable> {
        self.variables.get(id.index() as usize)
    }
}
