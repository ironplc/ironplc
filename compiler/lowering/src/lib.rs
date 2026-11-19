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
//! [`lower`] gives the whole lowered program a backend compiles, or a P9999
//! at each construct lowering does not express yet. It completes every
//! initializer on a copy of the library, then [`Lowering::lower_program`]
//! lowers the copy in one walk: every declaration of every reachable POU and
//! every global first, then each top-level statement of each body. A
//! [`PartialProgram`] records what did not lower, and is the whole lowered
//! program once everything did.
//!
//! Inside the walk, every function that lowers a node returns [`Lowered`]:
//! the node, or [`Unlowered`], which says why not. A construct lowering does
//! not express yet is [`Unlowered::NotYet`], which the walk reports as a
//! P9999 at the statement or declaration that holds the construct and goes
//! on with the next; a broken invariant is [`Unlowered::Broken`], a P9998,
//! which ends the walk.
//!
//! Lowering takes a [`CleanAnalysis`] and has no entry point that takes a
//! library without one (REQ-LOW-lowering-011).

// A match over a lowered enum names every variant (REQ-LOW-lowering-082).
#![deny(clippy::wildcard_enum_match_arm)]
#![allow(clippy::result_large_err)]

mod bits;
mod call;
mod control;
mod execution_model;
mod expr;
mod fb_call;
mod function;
mod init;
pub mod initial_value;
mod interface;
mod place;
mod program;
mod selection;
mod statement;
mod string;
mod string_result;
mod structure_result;
mod time;
mod typed;
mod types;
mod variables;
mod whole;

#[cfg(test)]
mod call_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod declaration_tests;
#[cfg(test)]
mod fb_call_tests;
#[cfg(test)]
mod function_tests;
#[cfg(test)]
mod in_place_tests;
#[cfg(test)]
mod init_tests;
#[cfg(test)]
mod method_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod program_tests;
#[cfg(test)]
mod prologue_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod string_result_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod structure_result_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod time_tests;
#[cfg(test)]
mod types_tests;

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;

use std::collections::HashMap;

use ironplc_analyzer::symbol_environment::DeclRef;
use ironplc_analyzer::{CleanAnalysis, MethodOwners};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, LibraryElementKind, ProgramDeclaration,
};
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::oop::MethodDeclaration;
pub use program::PartialProgram;
pub use whole::{lower, parse_policy};

use ironplc_ir::lowered::{ParsePolicy, PouId, ProgramBuilder, VarId};

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

/// Why lowering did not make a node (REQ-LOW-lowering-109).
#[derive(Clone, Debug)]
pub(crate) enum Unlowered {
    /// The node, or something inside it, is a construct lowering does not
    /// express yet. Not a defect: the walk reports it as a P9999 at the
    /// statement or declaration that holds the construct, and goes on.
    NotYet,
    /// The analyzed library breaks an invariant lowering relies on: a P9998,
    /// which ends the walk. Boxed so that the common case, a node that
    /// lowered, carries no more than a pointer for the uncommon one.
    Broken(Box<Diagnostic>),
}

impl From<Diagnostic> for Unlowered {
    fn from(diagnostic: Diagnostic) -> Self {
        Unlowered::Broken(Box::new(diagnostic))
    }
}

impl Unlowered {
    /// What the walk records for a node that did not lower: for a construct
    /// lowering does not express yet, the P9999 at `span`, the statement or
    /// declaration that holds the construct; for a broken invariant, its
    /// P9998, as the error that ends the walk.
    #[track_caller]
    pub(crate) fn not_yet_at(self, span: SourceSpan) -> Result<Diagnostic, Diagnostic> {
        match self {
            Unlowered::NotYet => Ok(Diagnostic::todo_with_span(span)),
            Unlowered::Broken(diagnostic) => Err(*diagnostic),
        }
    }
}

/// A node lowering made, or why it did not.
pub(crate) type Lowered<T> = Result<T, Unlowered>;

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
    /// The lowered type of each analyzer type, or why it did not lower.
    type_ids: HashMap<ironplc_dsl::type_id::TypeId, Lowered<ironplc_ir::lowered::TypeId>>,
    /// The analyzer types being lowered, each with the lowered type declared
    /// for it if a reference reached it again (`types::reference_target`).
    types_in_progress: HashMap<ironplc_dsl::type_id::TypeId, Option<ironplc_ir::lowered::TypeId>>,
    /// The variable of each declaration that is not a field.
    declared: HashMap<DeclRef, VarId>,
    pou_ids: HashMap<String, PouId>,
    /// The frame of each body it has lowered a statement of, enclosed by
    /// nothing, or why it could not make the body's frame.
    bodies: HashMap<PouId, Lowered<variables::BodyFrame<'static>>>,
    /// The block that declares each method a call can reach, as the
    /// analyzer resolves it (ADR-0041).
    method_owners: MethodOwners,
    /// The variable of the body of each method whose result is an array or
    /// a structure, which holds the result (REQ-LOW-lowering-187).
    aggregate_results: HashMap<PouId, VarId>,
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
            types_in_progress: HashMap::new(),
            declared: HashMap::new(),
            pou_ids: HashMap::new(),
            bodies: HashMap::new(),
            method_owners: MethodOwners::of_library(analysis.library(), analysis.context().types()),
            aggregate_results: HashMap::new(),
        }
    }

    /// Lowers `stmt`, a top-level statement of `body`, with every statement
    /// and expression inside it, to the statements it runs.
    #[cfg(test)]
    pub(crate) fn lower_statement(
        &mut self,
        body: Body<'a>,
        stmt: &ironplc_dsl::textual::StmtKind,
    ) -> Lowered<Vec<ironplc_ir::lowered::Stmt>> {
        let frame = self.frame(body)?;
        self.statement(&frame, stmt)
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
                LibraryElementKind::FunctionBlockDeclaration(block) if block.name.id == *pou => {
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
    #[cfg(test)]
    pub(crate) fn types(&self) -> &ironplc_ir::lowered::TypeTable {
        self.program.types()
    }

    /// The variables the lowered statements refer to.
    #[cfg(test)]
    pub(crate) fn variables(&self) -> &ironplc_ir::lowered::VariableTable {
        self.program.variables()
    }
}
