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

use ironplc_dsl::{
    common::{FunctionBlockDeclaration, FunctionDeclaration, Library, ProgramDeclaration},
    core::{Id, Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut hierarchy_visitor = HierarchyVisitor::new();
    hierarchy_visitor.walk(lib).map_err(|e| vec![e])?;

    if !hierarchy_visitor.problems.is_empty() {
        return Err(hierarchy_visitor.problems);
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
        if let Some(existing) = self
            .pou_types
            .insert(node.name.clone(), (PouKind::Function, node.name.span()))
        {
            self.problems.push(
                Diagnostic::problem(
                    Problem::PouDeclNameDuplicated,
                    Label::span(node.name.span(), "POU"),
                )
                .with_secondary(Label::span(existing.1, "POU")),
            );
        }
        self.context_type = Some((PouKind::Function, node.name.span()));

        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self.pou_types.insert(
            node.name.name.clone(),
            (PouKind::FunctionBlock, node.name.span()),
        ) {
            self.problems.push(
                Diagnostic::problem(
                    Problem::PouDeclNameDuplicated,
                    Label::span(node.name.span(), "POU"),
                )
                .with_secondary(Label::span(existing.1, "POU")),
            );
        }
        self.context_type = Some((PouKind::FunctionBlock, node.name.span()));

        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self
            .pou_types
            .insert(node.name.clone(), (PouKind::Program, node.name.span()))
        {
            self.problems.push(
                Diagnostic::problem(
                    Problem::PouDeclNameDuplicated,
                    Label::span(node.name.span(), "POU"),
                )
                .with_secondary(Label::span(existing.1, "POU")),
            );
        }
        self.context_type = Some((PouKind::Program, node.name.span()));

        node.recurse_visit(self)
    }

    fn visit_configuration_declaration(
        &mut self,
        node: &ironplc_dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        if let Some(existing) = self
            .pou_types
            .insert(node.name.clone(), (PouKind::Config, node.name.span()))
        {
            self.problems.push(
                Diagnostic::problem(
                    Problem::PouDeclNameDuplicated,
                    Label::span(node.name.span(), "POU"),
                )
                .with_secondary(Label::span(existing.1, "POU")),
            );
        }
        self.context_type = Some((PouKind::Config, node.name.span()));

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        rule_pou_hierarchy::apply,
        semantic_context::SemanticContextBuilder,
        test_helpers::{parse_and_resolve_types, parse_only},
    };
    use ironplc_parser::options::CompilerOptions;

    #[test]
    fn apply_when_duplicate_function_name_then_error() {
        let program = "
        FUNCTION Foo : BOOL
            Foo := FALSE;
        END_FUNCTION

        FUNCTION Foo : BOOL
            Foo := TRUE;
        END_FUNCTION";

        let library = parse_only(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_duplicate_function_block_name_then_error() {
        let program = "
        FUNCTION_BLOCK Bar
            VAR
                X : BOOL;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Bar
            VAR
                Y : BOOL;
            END_VAR
        END_FUNCTION_BLOCK";

        let library = parse_only(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_duplicate_program_name_then_error() {
        let program = "
        PROGRAM Baz
            VAR
                X : BOOL;
            END_VAR
        END_PROGRAM

        PROGRAM Baz
            VAR
                Y : BOOL;
            END_VAR
        END_PROGRAM";

        let library = parse_only(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_duplicate_configuration_name_then_error() {
        let program = "
        FUNCTION_BLOCK Fb1
            VAR
                X : BOOL;
            END_VAR
        END_FUNCTION_BLOCK

        PROGRAM Prg1
            VAR
                inst : Fb1;
            END_VAR
        END_PROGRAM

        CONFIGURATION Cfg1
            RESOURCE Res1 ON PLC
                TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
                PROGRAM P1 WITH Main : Prg1;
            END_RESOURCE
        END_CONFIGURATION

        CONFIGURATION Cfg1
            RESOURCE Res2 ON PLC
                TASK Main(INTERVAL := T#20ms, PRIORITY := 1);
                PROGRAM P2 WITH Main : Prg1;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse_only(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_program_uses_function_block_instance_then_ok() {
        let program = "
        FUNCTION_BLOCK COUNTER
            VAR
                count : INT;
            END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        PROGRAM main
            VAR
                c : COUNTER;
            END_VAR
            c();
        END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_function_block_uses_function_block_instance_then_ok() {
        let program = "
        FUNCTION_BLOCK Callee
            VAR
                IN1 : BOOL;
            END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Caller
            VAR
                CalleeInstance : Callee;
            END_VAR
        END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let _ = apply(&library, &context, &CompilerOptions::default());
        // TODO
        // assert!(result.is_ok());
    }
}
