//! Semantic rule that flags the CODESYS/TwinCAT call-style function-block
//! instance initializer (`name : FB_Type(args)`) as recognized but not yet
//! supported in code generation.
//!
//! The parser accepts this construct and stores it as a distinct AST node
//! ([`FunctionBlockCallInitializer`]) so that source files using it are not
//! rejected outright. In CODESYS the arguments are passed to the function
//! block's constructor (the `FB_init` method), which the compiler does not
//! yet model. Rather than silently discard the arguments, this rule emits a
//! P9999 (`NotImplemented`) diagnostic until code generation supports the
//! construct.
//!
//! This mirrors [`crate::rule_unsupported_stdlib_type`] (P9001), which flags
//! recognized-but-unimplemented standard library types.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Example
//!    VAR
//!       comm : FB_Comm(retries := 3);  // call-style initializer
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleFunctionBlockCallUnsupported {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleFunctionBlockCallUnsupported {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleFunctionBlockCallUnsupported {
    type Value = ();

    fn visit_function_block_call_initializer(
        &mut self,
        node: &FunctionBlockCallInitializer,
    ) -> Result<(), Diagnostic> {
        self.diagnostics
            .push(Diagnostic::not_implemented(Label::span(
                node.type_name.span(),
                "Call-style function block initializer",
            )));
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    #[test]
    fn apply_when_fb_call_style_init_then_reports_not_implemented() {
        let program = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3);
END_VAR
END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context, &CompilerOptions::default());

        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        // (must be constructed via Diagnostic::not_implemented), so assert on
        // the stable code string rather than referencing the variant.
        assert_eq!(diagnostics[0].code, "P9999");
    }

    rule_ok!(
        apply_when_fb_member_init_then_ok,
        "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm := (retries := 3);
END_VAR
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_bare_fb_decl_then_ok,
        "
FUNCTION_BLOCK FB_Comm
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm;
END_VAR
END_FUNCTION_BLOCK"
    );
}
