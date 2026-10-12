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

mod control;
mod expr;
mod place;
mod statement;
mod types;
mod variables;

#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use ironplc_analyzer::symbol_environment::DeclRef;
use ironplc_analyzer::CleanAnalysis;
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, LibraryElementKind, ProgramDeclaration,
};
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::StmtKind;
use ironplc_ir::lowered::{
    LoopScope, PouId, ProgramBuilder, Stmt, TypeTable, VarId, VariableTable,
};

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
    Lowered(Box<Stmt>),
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
    /// The lowered program's tables, which every node lowering builds
    /// refers into.
    program: ProgramBuilder,
    /// The lowered type of each analyzer type, or `None` for one lowering
    /// cannot express yet.
    type_ids: HashMap<ironplc_dsl::type_id::TypeId, Option<ironplc_ir::lowered::TypeId>>,
    /// The variable of each declaration that is not a field.
    declared: HashMap<DeclRef, VarId>,
    pou_ids: HashMap<String, PouId>,
    /// What lowering knows of each body it has lowered a statement of.
    bodies: HashMap<PouId, Option<variables::BodyFrame>>,
    /// The loops enclosing the statement being lowered, innermost last.
    loops: Vec<LoopScope>,
}

impl<'a> Lowering<'a> {
    /// Starts lowering the library `analysis` holds.
    pub fn new(analysis: &'a CleanAnalysis<'a>) -> Self {
        Self {
            analysis,
            program: ProgramBuilder::new(),
            type_ids: HashMap::new(),
            declared: HashMap::new(),
            pou_ids: HashMap::new(),
            bodies: HashMap::new(),
            loops: Vec::new(),
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
        let Some(frame) = self.frame(body)? else {
            return Ok(StatementLowering::Unsupported);
        };
        Ok(match self.statement(&frame, stmt)? {
            Some(lowered) => StatementLowering::Lowered(Box::new(lowered)),
            None => StatementLowering::Unsupported,
        })
    }

    /// Lowers `stmt`, a top-level statement of the body of the program,
    /// function block or function named `pou` in the library the analysis
    /// holds. A POU of another kind, such as a method, is unsupported.
    pub fn lower_statement_of(
        &mut self,
        pou: &Id,
        stmt: &StmtKind,
    ) -> Result<StatementLowering, Diagnostic> {
        match self.body_named(pou) {
            Some(body) => self.lower_statement(body, stmt),
            None => Ok(StatementLowering::Unsupported),
        }
    }

    /// The body of the program, function block or function named `pou`.
    fn body_named(&self, pou: &Id) -> Option<Body<'a>> {
        self.analysis
            .library()
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ProgramDeclaration(program) if program.name == *pou => {
                    Some(Body::Program(program))
                }
                LibraryElementKind::FunctionBlockDeclaration(block) if block.name.name == *pou => {
                    Some(Body::FunctionBlock(block))
                }
                LibraryElementKind::FunctionDeclaration(function) if function.name == *pou => {
                    Some(Body::Function(function))
                }
                LibraryElementKind::DataTypeDeclaration(_)
                | LibraryElementKind::FunctionDeclaration(_)
                | LibraryElementKind::FunctionBlockDeclaration(_)
                | LibraryElementKind::ProgramDeclaration(_)
                | LibraryElementKind::ConfigurationDeclaration(_)
                | LibraryElementKind::GlobalVarDeclarations(_)
                | LibraryElementKind::InterfaceDeclaration(_) => None,
            })
    }

    /// The types the lowered statements refer to.
    pub fn types(&self) -> &TypeTable {
        self.program.types()
    }

    /// The variables the lowered statements refer to.
    pub fn variables(&self) -> &VariableTable {
        self.program.variables()
    }
}
