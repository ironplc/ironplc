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

mod bits;
mod call;
mod call_order;
mod control;
mod expr;
mod fb_call;
mod function;
mod hoist;
mod place;
mod statement;
mod string;
mod string_result;
mod time;
mod types;
mod variables;

#[cfg(test)]
mod call_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod fb_call_tests;
#[cfg(test)]
mod function_tests;
#[cfg(test)]
mod hoist_tests;
#[cfg(test)]
mod method_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod string_result_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod time_tests;

use std::collections::HashMap;

use ironplc_analyzer::symbol_environment::DeclRef;
use ironplc_analyzer::{CleanAnalysis, MethodOwners};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, LibraryElementKind, ProgramDeclaration,
};
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::oop::MethodDeclaration;
use ironplc_dsl::textual::StmtKind;
use ironplc_ir::lowered::{
    LoopScope, ParsePolicy, Place, PouId, PouTable, ProgramBuilder, Stmt, TypeTable, VarId,
    VariableTable,
};

/// The body a statement belongs to.
#[derive(Clone, Copy, Debug)]
pub enum Body<'a> {
    Program(&'a ProgramDeclaration),
    FunctionBlock(&'a FunctionBlockDeclaration),
    Function(&'a FunctionDeclaration),
    /// A method of the function block `block` declares.
    Method {
        block: &'a FunctionBlockDeclaration,
        method: &'a MethodDeclaration,
    },
}

/// What lowering made of one statement.
#[derive(Clone, Debug)]
pub enum StatementLowering {
    /// The statement, with everything inside it, lowered to the statements
    /// it runs, in order.
    Lowered(Vec<Stmt>),
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
    /// How a `STRING_TO_*` conversion reads its string: the behaviour
    /// policy the compiler's options select (REQ-LOW-lowering-073).
    parse: ParsePolicy,
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
    /// Where the call whose outputs are being lowered leaves them, which
    /// its call outputs read.
    call_outputs: Option<CallOutputs>,
    /// The parts of the calls with outputs of the assignment being lowered
    /// (`hoist.rs`); `None` outside an assignment.
    hoisting: Option<hoist::Hoisting>,
    /// The block that declares each method a call can reach, as the
    /// analyzer resolves it (ADR-0041).
    method_owners: MethodOwners,
    /// The calls with a string result moved out of the statement being
    /// lowered (`string_result.rs`); `None` outside a statement.
    moved: Option<string_result::MovedCalls>,
}

/// Where a call leaves its outputs for the assignments of its outputs to
/// read.
#[derive(Clone, Debug)]
enum CallOutputs {
    /// In the fields of the function block instance it called.
    Instance(Place),
    /// In the temporary it passed for each output of a function or method,
    /// by the output's name (REQ-LOW-lowering-092).
    Temporaries(Vec<(Id, Place)>),
}

impl<'a> Lowering<'a> {
    /// Starts lowering the library `analysis` holds, reading a string as a
    /// number as `parse` says.
    pub fn new(analysis: &'a CleanAnalysis<'a>, parse: ParsePolicy) -> Self {
        Self {
            analysis,
            parse,
            program: ProgramBuilder::new(),
            type_ids: HashMap::new(),
            declared: HashMap::new(),
            pou_ids: HashMap::new(),
            bodies: HashMap::new(),
            loops: Vec::new(),
            call_outputs: None,
            hoisting: None,
            method_owners: MethodOwners::of_library(analysis.library(), analysis.context().types()),
            moved: None,
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
            Some(lowered) => StatementLowering::Lowered(lowered),
            None => StatementLowering::Unsupported,
        })
    }

    /// Lowers `stmt`, a top-level statement of the body of the program,
    /// function block or function named `pou` in the library the analysis
    /// holds. A method's statement is lowered by
    /// [`Self::lower_method_statement_of`].
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

    /// Lowers `stmt`, a top-level statement of the body of the method
    /// `method` of the function block `block`, as [`Self::lower_statement`]
    /// does.
    pub fn lower_method_statement_of(
        &mut self,
        block: &Id,
        method: &Id,
        stmt: &StmtKind,
    ) -> Result<StatementLowering, Diagnostic> {
        let Some(Body::FunctionBlock(block)) = self.body_named(block) else {
            return Ok(StatementLowering::Unsupported);
        };
        match block
            .methods
            .iter()
            .find(|declared| declared.name == *method)
        {
            Some(method) => self.lower_statement(Body::Method { block, method }, stmt),
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

    /// The POUs the lowered statements refer to.
    pub fn pous(&self) -> &PouTable {
        self.program.pous()
    }
}
