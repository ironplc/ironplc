//! Semantic rule that rejects `VAR_GLOBAL` declarations at the top level of a
//! library (outside any `CONFIGURATION`/`RESOURCE` block) unless the
//! `--allow-top-level-var-global` extension is enabled.
//!
//! In strict IEC 61131-3, `VAR_GLOBAL` is only permitted inside
//! `CONFIGURATION` and `RESOURCE`. The parser represents a top-level block as a
//! distinct [`LibraryElementKind::GlobalVarDeclarations`] element, so this rule
//! is simply "reject that element kind when the flag is off" — config/resource
//! globals live on the [`ConfigurationDeclaration`] and are never seen here.
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL CONSTANT
//!     X : INT := 250;
//!   END_VAR
//!   ...
//! END_CONFIGURATION
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! VAR_GLOBAL CONSTANT
//!   X : INT := 250;
//! END_VAR
//! ```
use ironplc_dsl::{
    common::{Library, LibraryElementKind},
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_top_level_var_global {
        return Ok(());
    }

    let diagnostics: Vec<Diagnostic> = lib
        .elements
        .iter()
        .filter_map(|element| match element {
            LibraryElementKind::GlobalVarDeclarations(decls) => Some(decls),
            _ => None,
        })
        .map(|decls| {
            // Point at the first declaration; the whole block is the offending
            // construct but the AST does not carry the `VAR_GLOBAL` keyword span.
            let span = decls
                .first()
                .map_or_else(SourceSpan::default, Located::span);
            Diagnostic::problem(
                Problem::TopLevelVarGlobalNotAllowed,
                Label::span(span, "Top-level VAR_GLOBAL"),
            )
            .with_help(
                "Move the VAR_GLOBAL block inside a CONFIGURATION (or RESOURCE) block, \
                 or select a dialect that supports top-level VAR_GLOBAL.",
            )
        })
        .collect();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod test {
    use crate::test_helpers::{diagnostic_codes, rule_diagnostics_with};
    use ironplc_parser::options::CompilerOptions;

    use super::*;

    const TOP_LEVEL_GLOBAL: &str =
        "VAR_GLOBAL CONSTANT\nX : INT := 250;\nEND_VAR\nPROGRAM p\nEND_PROGRAM";

    /// The parser accepts a top-level `VAR_GLOBAL` only with this option on.
    fn allowed() -> CompilerOptions {
        CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_top_level_var_global_and_not_allowed_then_error_with_help() {
        let diagnostics = rule_diagnostics_with(
            apply,
            TOP_LEVEL_GLOBAL,
            &allowed(),
            &CompilerOptions::default(),
        );

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::TopLevelVarGlobalNotAllowed.code()]
        );
        assert!(!diagnostics[0].help().is_empty());
        let location = &diagnostics[0].primary.location;
        assert_eq!(&TOP_LEVEL_GLOBAL[location.start..location.end], "X");
    }

    rule_ok!(
        apply_when_top_level_var_global_and_allowed_then_ok,
        TOP_LEVEL_GLOBAL,
        allowed()
    );

    // Even with the option off, a global inside a configuration is allowed.
    rule_ok!(
        apply_when_var_global_inside_configuration_then_ok,
        "CONFIGURATION config
    VAR_GLOBAL CONSTANT
        X : INT := 250;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : p;
    END_RESOURCE
END_CONFIGURATION
PROGRAM p
END_PROGRAM"
    );

    rule_ok!(apply_when_no_var_global_then_ok, "PROGRAM p\nEND_PROGRAM");
}
