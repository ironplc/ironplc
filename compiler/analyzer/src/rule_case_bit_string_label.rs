//! Semantic rule that gates hex/binary/octal bit-string literals used as
//! `CASE` labels (e.g. `16#D012:`, `2#1010:`) behind
//! `--allow-bit-string-case-labels`.
//!
//! The IEC 61131-3 standard grammar for a case label (Annex B) is
//! `case_list_element ::= subrange | signed_integer | enumerated_value`,
//! where `signed_integer` is a *decimal* digit sequence. Radix-prefixed
//! bit-string literals (`hex_integer` / `binary_integer` / `octal_integer`)
//! are deliberately not in `case_list_element`, so accepting them as a
//! label is a dialect extension (TwinCAT/CODESYS). The parser always accepts
//! the form; this rule is what enforces the flag.
//!
//! ## Fails (without the flag)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Example
//! VAR
//!     x : DINT;
//!     y : INT;
//! END_VAR
//! CASE x OF
//!     16#D012: y := 1;
//! END_CASE;
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{Case, CaseSelectionKind},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(
    lib: &ironplc_dsl::common::Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if options.allow_bit_string_case_labels {
        return Ok(());
    }

    let mut visitor = RuleCaseBitStringLabel {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleCaseBitStringLabel {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleCaseBitStringLabel {
    type Value = ();

    fn visit_case(&mut self, node: &Case) -> Result<Self::Value, Diagnostic> {
        for group in &node.statement_groups {
            for selector in &group.selectors {
                if let CaseSelectionKind::BitStringLiteral(lit) = selector {
                    self.diagnostics.push(Diagnostic::problem(
                        Problem::BitStringCaseLabelNotAllowed,
                        Label::span(lit.value.span(), "Bit-string literal CASE label"),
                    ));
                }
            }
        }
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
            allow_bit_string_case_labels: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn apply_when_hex_case_label_and_flag_disabled_then_error() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#D012: y := 1;
END_CASE;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_err());
    }

    #[test]
    fn apply_when_hex_case_label_and_flag_enabled_then_ok() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#D012: y := 1;
END_CASE;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &opts_with_flag());

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_binary_and_octal_case_labels_and_flag_disabled_then_error_per_label() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    2#1010: y := 1;
    8#17: y := 2;
END_CASE;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        let diagnostics = result.expect_err("both bit-string labels must be flagged");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn apply_when_plain_decimal_case_label_then_never_flagged() {
        // A plain decimal label parses as SignedInteger, not BitStringLiteral,
        // so it is standard syntax and must never be flagged regardless of the
        // option.
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    5: y := 1;
END_CASE;
END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context, &CompilerOptions::default());

        assert!(result.is_ok());
    }
}
