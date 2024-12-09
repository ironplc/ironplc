//! Rule that checks the hierarchy of declarations.
//! 
//! This rule passes when:
//! 
//! 1. Programs only call function or function blocks
//! 2. Function blocks only call functions or function blocks.
//! 3. Functions call only other functions.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//!    VAR
//!       IN1: BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       CalleeInstance : Callee;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK SelfRecursive
//!    VAR
//!       SelfRecursiveInstance : SelfRecursive;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```

use std::collections::HashMap;

use ironplc_dsl::{common::{FunctionBlockDeclaration, FunctionDeclaration, Library, ProgramDeclaration}, core::{Id, Located, SourceSpan}, diagnostic::{Diagnostic, Label}, visitor::Visitor};
use ironplc_problems::Problem;

use crate::result::SemanticResult;

pub fn apply(lib: &Library) -> SemanticResult {
    let mut hierarchy_visitor = HierarchyVisitor::new();
    hierarchy_visitor.walk(lib).map_err(|e| vec![e])?;

    if hierarchy_visitor.problems.len() > 0 {
        return Err(hierarchy_visitor.problems)
    }
    Ok(())
}

#[derive(Debug)]
enum PouKind {
    Function,
    FunctionBlock, 
    Program,
    Config,
}

struct HierarchyVisitor {
    pou_types: HashMap<Id, (PouKind, SourceSpan)>,
    problems: Vec<Diagnostic>,
    context_type: Option<(PouKind, SourceSpan)>,
}

impl HierarchyVisitor {
    fn new() -> Self {
        Self {
            pou_types: HashMap::new(),
            problems: Vec::new(),
            context_type: None,
        }
    }
}

impl Visitor<Diagnostic> for HierarchyVisitor {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self.pou_types.insert(node.name.clone(), (PouKind::Function, node.name.span())) {
            self.problems.push(Diagnostic::problem(Problem::PouDeclNameDuplicated, Label::span(node.name.span(), "POU")).with_secondary(Label::span(existing.1, "POU")));
        }
        self.context_type = Some((PouKind::Function, node.name.span()));

        let res = node.recurse_visit(self);
        res
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self.pou_types.insert(node.name.clone(), (PouKind::FunctionBlock, node.name.span())) {
            self.problems.push(Diagnostic::problem(Problem::PouDeclNameDuplicated, Label::span(node.name.span(), "POU")).with_secondary(Label::span(existing.1, "POU")));
        }
        self.context_type = Some((PouKind::FunctionBlock, node.name.span()));

        let res = node.recurse_visit(self);
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self.pou_types.insert(node.name.clone(), (PouKind::Program, node.name.span())) {
            self.problems.push(Diagnostic::problem(Problem::PouDeclNameDuplicated, Label::span(node.name.span(), "POU")).with_secondary(Label::span(existing.1, "POU")));
        }
        self.context_type = Some((PouKind::Program, node.name.span()));

        let res = node.recurse_visit(self);
        res
    }

    fn visit_configuration_declaration(
        &mut self,
        node: &ironplc_dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self.pou_types.insert(node.name.clone(), (PouKind::Config, node.name.span())) {
            self.problems.push(Diagnostic::problem(Problem::PouDeclNameDuplicated, Label::span(node.name.span(), "POU")).with_secondary(Label::span(existing.1, "POU")));
        }
        self.context_type = Some((PouKind::Config, node.name.span()));

        let res = node.recurse_visit(self);
        res
    }

    fn visit_function_block_initial_value_assignment(&mut self,node: &ironplc_dsl::common::FunctionBlockInitialValueAssignment) -> Result<Self::Value,Diagnostic> {
        if let Some(existing) = self.pou_types.insert(node.type_name.name.clone(), (PouKind::FunctionBlock, node.type_name.name.span())) {
            self.problems.push(Diagnostic::problem(Problem::PouDeclNameDuplicated, Label::span(node.type_name.name.span(), "POU")).with_secondary(Label::span(existing.1, "POU")));
        }
        self.context_type = Some((PouKind::Config, node.type_name.name.span()));

        let res = node.recurse_visit(self);
        res
    }
}

#[cfg(test)]
mod tests {
    use crate::{rule_pou_hierarchy::apply, test_helpers::parse_and_resolve_types};

    #[test]
    fn apply_when_function_invokes_function_block_then_error() {
        let program = "
        FUNCTION_BLOCK Callee
            VAR
               IN1: BOOL;
            END_VAR

        END_FUNCTION_BLOCK
        
        FUNCTION Caller : BOOL
            VAR
                CalleeInstance : Callee;
            END_VAR

            Caller := FALSE;
        END_FUNCTION";

        let library = parse_and_resolve_types(program);
        let _ = apply(&library);
        // TODO
        // assert!(result.is_ok());
    }
}