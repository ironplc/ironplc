//! Semantic rule that rejects a character string literal containing a
//! character its string type cannot represent.
//!
//! STRING is Latin-1, one byte per character, so a STRING literal may only
//! contain characters up to U+00FF. WSTRING is UTF-16LE with the UCS-2
//! semantics of IEC 61131-3, one code unit per character, so a WSTRING
//! literal may only contain characters of the Basic Multilingual Plane, up
//! to U+FFFF (ADR-0016). Codegen narrows each character to the type's code
//! unit; without this rule, `'等'` (U+7B49) would be stored as `'I'` (0x49)
//! and two different literals could compare equal. The rule rejects the
//! literal at analysis with P4052 so that `check`, `compile` and the
//! language server all report it at the literal.
//!
//! A literal's type is the one its delimiter spells (single quotes for
//! STRING, double quotes for WSTRING), wherever the literal appears: an
//! expression, a variable initializer, an array or structure initializer.
//! The one exception is the default of a string type declaration, where the
//! grammar accepts either delimiter and the parser records the declared
//! width on the literal.
//!
//! The parser does not decode `$` escapes, so a literal's characters are the
//! source text as written. An escape is spelled in ASCII, and decodes to a
//! character the type can hold (`$XX` at most U+00FF, `$XXXX` at most
//! U+FFFF), so checking the characters as written is right both before and
//! after decoding.
//!
//! ## Fails
//!
//! ```ignore
//! VAR
//!     s : STRING[10] := '等';   (* P4052: U+7B49 is above U+00FF *)
//!     w : WSTRING[10] := "😀";  (* P4052: U+1F600 is above U+FFFF *)
//! END_VAR
//! ```

use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::result::SemanticResult;
use crate::rule_support::{run_rule, DiagnosticVisitor};
use crate::semantic_context::SemanticContext;
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleStringLiteralCharRange {
            diagnostics: vec![],
        },
        lib,
    )
}

struct RuleStringLiteralCharRange {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleStringLiteralCharRange {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The largest code point a literal of the given type can hold (ADR-0016).
fn max_code_point(width: &StringType) -> u32 {
    match width {
        StringType::String => 0xFF,
        StringType::WString => 0xFFFF,
    }
}

/// How to make the literal fit, which depends on the type: a STRING literal
/// has a wider type to move to, a WSTRING literal does not.
fn help(width: &StringType) -> &'static str {
    match width {
        StringType::String => {
            "Write the literal with double quotes as a WSTRING literal, and declare the variable as WSTRING"
        }
        StringType::WString => {
            "Characters above U+FFFF (outside the Basic Multilingual Plane) cannot be stored in a WSTRING"
        }
    }
}

impl Visitor<Infallible> for RuleStringLiteralCharRange {
    type Value = ();

    fn visit_character_string_literal(
        &mut self,
        node: &CharacterStringLiteral,
    ) -> Result<Self::Value, Infallible> {
        let max = max_code_point(&node.width);
        // One diagnostic per literal: the first character that does not fit
        // is enough to say what is wrong, and a literal in another script
        // would otherwise produce one diagnostic per character.
        if let Some(ch) = node.value.iter().find(|ch| u32::from(**ch) > max) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::StringLiteralCharOutOfRange,
                    Label::span(
                        node.span.clone(),
                        format!("Character not representable in {}", node.width.keyword()),
                    ),
                )
                .with_context("character", &format!("{ch} (U+{:04X})", u32::from(*ch)))
                .with_context("type", &node.width.keyword().to_string())
                .with_help(help(&node.width)),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_parser::options::CompilerOptions;

    rule_ok!(
        apply_when_string_literal_latin1_then_ok,
        "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := 'café';
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_string_literal_above_latin1_then_p4052_at_literal,
        "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := '等';
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "'等'"
    );

    rule_ok!(
        apply_when_wstring_literal_bmp_then_ok,
        "PROGRAM main
VAR
    w : WSTRING[10];
END_VAR
    w := \"等\";
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_wstring_literal_above_bmp_then_p4052_at_literal,
        "PROGRAM main
VAR
    w : WSTRING[10];
END_VAR
    w := \"😀\";
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "\"😀\""
    );

    rule_err1_at!(
        apply_when_string_initializer_above_latin1_then_p4052_at_literal,
        "PROGRAM main
VAR
    tag : STRING[10] := '等';
END_VAR
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "'等'"
    );

    rule_err1_at!(
        apply_when_string_type_declaration_default_above_latin1_then_p4052_at_literal,
        "TYPE
    T : STRING[5] := '等';
END_TYPE",
        Problem::StringLiteralCharOutOfRange,
        "'等'"
    );

    // The grammar accepts either delimiter for a type declaration's default
    // and the declared width governs, so a WSTRING default written with
    // single quotes is checked against the WSTRING range.
    rule_ok!(
        apply_when_wstring_type_declaration_default_written_narrow_then_ok,
        "TYPE
    T : WSTRING[5] := '等';
END_TYPE"
    );

    rule_err1_at!(
        apply_when_array_of_string_initializer_element_above_latin1_then_p4052_at_literal,
        "PROGRAM main
VAR
    names : ARRAY[1..2] OF STRING[10] := ['ok', '等'];
END_VAR
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "'等'"
    );

    rule_err1_at!(
        apply_when_string_literal_in_comparison_then_p4052_at_literal,
        "PROGRAM main
VAR
    s : STRING[10];
    r : BOOL;
END_VAR
    r := s = '等';
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "'等'"
    );

    #[test]
    fn apply_when_both_comparison_literals_above_latin1_then_one_p4052_each() {
        let program = "PROGRAM main
VAR
    r : BOOL;
END_VAR
    r := '最终检测' = '一终检测';
END_PROGRAM";
        let opts = CompilerOptions::default();
        let (library, context) = crate::test_helpers::resolve_fresh_with(program, &opts);
        let errors = apply(&library, &context, &opts).unwrap_err();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors
            .iter()
            .all(|d| d.code == Problem::StringLiteralCharOutOfRange.code()));
    }

    rule_ok!(
        apply_when_string_literal_dollar_escape_then_ok,
        "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := '$41$$';
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_string_literal_has_several_bad_chars_then_one_diagnostic,
        "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := '等等';
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "'等等'"
    );

    rule_err1_at!(
        apply_when_prefixed_string_literal_above_latin1_then_label_covers_prefix,
        "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := STRING#'等';
END_PROGRAM",
        Problem::StringLiteralCharOutOfRange,
        "STRING#'等'"
    );

    #[test]
    fn apply_when_string_literal_above_latin1_then_diagnostic_names_character_and_fix() {
        let program = "PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := '等';
END_PROGRAM";
        let opts = CompilerOptions::default();
        let (library, context) = crate::test_helpers::resolve_fresh_with(program, &opts);
        let errors = apply(&library, &context, &opts).unwrap_err();
        let rendered = format!("{:?}", errors[0]);
        assert!(rendered.contains("U+7B49"), "{rendered}");
        assert!(errors[0].help().iter().any(|h| h.contains("WSTRING")));
    }

    #[test]
    fn analyze_when_string_literal_above_latin1_then_pipeline_reports_p4052() {
        // The rule is wired into the full `analyze` pipeline, which collects
        // semantic diagnostics into the context rather than returning Err.
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(
            "PROGRAM main
VAR
    tag : STRING[10] := '等';
    face : WSTRING[10] := \"😀\";
END_VAR
END_PROGRAM",
        );
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        let count = context
            .diagnostics()
            .iter()
            .filter(|d| d.code == Problem::StringLiteralCharOutOfRange.code())
            .count();
        assert_eq!(count, 2);
    }
}
