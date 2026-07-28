//! Semantic rule that gates AT-located variables declared inside an
//! otherwise plain `VAR`/`VAR_INPUT`/`VAR_OUTPUT` block behind
//! `--allow-mixed-located-var-declarations`.
//!
//! The IEC 61131-3 standard requires located variables (complete address
//! like `AT %IX0.0`, or incomplete/wildcard address like `AT %I*`) to live
//! in their own dedicated `VAR ... END_VAR` block, separate from ordinary
//! symbolic variables. Real CODESYS/TwinCAT code commonly mixes them in one
//! block; this is a vendor extension.
//!
//! The parser always accepts the mixed form; this rule is what actually
//! enforces the flag, by deriving "is this declaration mixed with a plain
//! sibling" from each declaration's [`BlockId`] rather than a precomputed
//! flag -- see [`mixed_located_var_decls`].
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Example
//! VAR
//!     tempSensor AT%I*: INT;
//!     fbComm     : BOOL;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_mixed_located_var_declarations {
        return Ok(());
    }

    let mut visitor = RuleMixedLocatedVarDeclarations {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleMixedLocatedVarDeclarations {
    diagnostics: Vec<Diagnostic>,
}

impl RuleMixedLocatedVarDeclarations {
    fn check(&mut self, owner: &dyn HasVariables) {
        for decl in mixed_located_var_decls(owner.variables()) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::MixedLocatedVarDeclarationNotAllowed,
                Label::span(decl.identifier.span(), "Located variable"),
            ));
        }
    }
}

impl Visitor<Diagnostic> for RuleMixedLocatedVarDeclarations {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.check(node);
        node.recurse_visit(self)
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.check(node);
        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.check(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    fn opts_with_flag() -> CompilerOptions {
        CompilerOptions {
            allow_mixed_located_var_declarations: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_mixed_block_and_flag_disabled_then_error() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    tempSensor AT%I*: INT;
    fbComm     : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_err());
    }

    #[test]
    fn apply_when_mixed_block_and_flag_enabled_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    tempSensor AT%I*: INT;
    fbComm     : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &opts_with_flag());

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_dedicated_incompl_located_block_then_never_flagged() {
        // A block containing ONLY located variables uses the pre-existing,
        // always-allowed incompl_located_var_declarations() grammar path,
        // not the new mixed-block extension -- must never be flagged
        // regardless of the option, proving mixed_located_var_decls
        // correctly distinguishes the two (no Symbol sibling in this
        // block's BlockId group).
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    tempSensor1 AT%I*: INT;
    tempSensor2 AT%I*: INT;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_plain_block_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : BOOL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_ok());
    }
}
