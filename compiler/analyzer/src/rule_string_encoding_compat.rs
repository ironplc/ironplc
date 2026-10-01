//! Semantic rule that rejects mixing STRING and WSTRING values.
//!
//! STRING (Latin-1, one byte per character) and WSTRING (UTF-16LE, two bytes
//! per code unit) are distinct types with incompatible runtime encodings
//! (ADR-0016, ADR-0034). Assigning or comparing one against the other has no
//! implicit conversion, so the compiler rejects it at analysis time with
//! P4034. The VM also traps such a mix at runtime as defense-in-depth, but the
//! compile-time check is the primary guard.
//!
//! The rule reasons about the **declared** encoding of named string variables.
//! It does not flag string literals, which carry their encoding in their
//! delimiter and so are already typed `STRING` or `WSTRING` -- the ordinary
//! type checks (P4035 for an assignment, P4026 for a call argument) reject a
//! literal that does not match its destination. Nor does it flag the results
//! of string functions, whose encoding the analyzer collapses to a single
//! `STRING` type name; codegen resolves one encoding per operation and reports
//! P4034 there. Whether the characters of a literal fit its encoding is
//! `rule_string_literal_char_range` (P4052).
//!
//! ## Fails
//!
//! ```ignore
//! VAR
//!     s : STRING[10];
//!     w : WSTRING[10];
//! END_VAR
//!     s := w;        (* P4034: STRING := WSTRING *)
//! ```

use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::intermediate_type::IntermediateType;
use crate::result::SemanticResult;
use crate::rule_support::{run_rule, DiagnosticVisitor};
use crate::semantic_context::SemanticContext;
use crate::symbol_environment::ScopeTracker;
use ironplc_container::CharWidth;
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleStringEncodingCompat {
            context,
            scope: ScopeTracker::default(),
            diagnostics: vec![],
        },
        lib,
    )
}

struct RuleStringEncodingCompat<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look variables up in the symbol
    /// environment. A method's scope nests inside its function block's,
    /// so a method body sees its own variables as well as the instance's
    /// fields.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleStringEncodingCompat<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleStringEncodingCompat<'_> {
    /// Returns the declared string encoding of a simple named variable, if
    /// it is a string variable in scope.
    fn named_variable_encoding(&self, var: &Variable) -> Option<StringType> {
        let Variable::Symbolic(SymbolicVariableKind::Named(named)) = var else {
            return None;
        };
        let type_id = self
            .context
            .symbols()
            .find(&named.name, &self.scope.current())?
            .type_id?;
        match self.context.types().get_by_id(type_id)?.representation {
            IntermediateType::String {
                char_width: CharWidth::Narrow,
                ..
            } => Some(StringType::String),
            IntermediateType::String {
                char_width: CharWidth::Wide,
                ..
            } => Some(StringType::WString),
            _ => None,
        }
    }

    /// Returns the declared string encoding of an expression when it is a
    /// simple named string variable. Literals and complex expressions
    /// return `None`.
    fn expr_string_encoding(&self, expr: &Expr) -> Option<StringType> {
        match &expr.kind {
            ExprKind::Variable(var) => self.named_variable_encoding(var),
            _ => None,
        }
    }

    fn report(
        &mut self,
        span: ironplc_dsl::core::SourceSpan,
        left: &StringType,
        right: &StringType,
    ) {
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::StringEncodingMismatch,
                Label::span(span, "Incompatible string encodings"),
            )
            .with_context("left", &left.keyword().to_string())
            .with_context("right", &right.keyword().to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleStringEncodingCompat<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Infallible> {
        let target_enc = self.named_variable_encoding(&node.target);
        let value_enc = self.expr_string_encoding(&node.value);
        if let (Some(target_enc), Some(value_enc)) = (target_enc, value_enc) {
            if target_enc != value_enc {
                self.report(node.span(), &target_enc, &value_enc);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<Self::Value, Infallible> {
        let left_enc = self.expr_string_encoding(&node.left);
        let right_enc = self.expr_string_encoding(&node.right);
        if let (Some(left_enc), Some(right_enc)) = (left_enc, right_enc) {
            if left_enc != right_enc {
                self.report(node.left.span(), &left_enc, &right_enc);
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        parse_and_resolve_types_with_context, parse_and_resolve_types_with_options,
    };
    use ironplc_parser::options::CompilerOptions;

    fn check(source: &str) -> SemanticResult {
        let (library, context) = parse_and_resolve_types_with_context(source);
        apply(&library, &context, &CompilerOptions::default())
    }

    #[test]
    fn apply_when_string_assigned_wstring_then_p4034() {
        let result = check(
            "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  s := w;
END_PROGRAM
",
        );
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors[0].code, Problem::StringEncodingMismatch.code());
    }

    #[test]
    fn apply_when_wstring_assigned_string_then_p4034() {
        let result = check(
            "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  w := s;
END_PROGRAM
",
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err()[0].code,
            Problem::StringEncodingMismatch.code()
        );
    }

    #[test]
    fn apply_when_string_assigned_string_then_ok() {
        let result = check(
            "
PROGRAM main
  VAR
    a : STRING[10];
    b : STRING[10];
  END_VAR
  a := b;
END_PROGRAM
",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_wstring_assigned_wstring_then_ok() {
        let result = check(
            "
PROGRAM main
  VAR
    a : WSTRING[10];
    b : WSTRING[10];
  END_VAR
  a := b;
END_PROGRAM
",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_cross_encoding_comparison_then_p4034() {
        let result = check(
            "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
    r : BOOL;
  END_VAR
  r := s = w;
END_PROGRAM
",
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err()[0].code,
            Problem::StringEncodingMismatch.code()
        );
    }

    #[test]
    fn apply_when_wstring_assigned_literal_then_ok() {
        // A wide literal matches a wide target; nothing to flag.
        let result = check(
            "
PROGRAM main
  VAR
    w : WSTRING[10];
  END_VAR
  w := \"hi\";
END_PROGRAM
",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn analyze_when_string_assigned_wstring_then_pipeline_reports_p4034() {
        // The rule is wired into the full `analyze` pipeline, which collects
        // semantic diagnostics into the context rather than returning Err.
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(
            "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  s := w;
END_PROGRAM
",
        );
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        assert!(context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::StringEncodingMismatch.code()));
    }

    fn check_ed3(source: &str) -> SemanticResult {
        let options =
            CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Iec61131_3Ed3);
        let (library, context) = parse_and_resolve_types_with_options(source, &options);
        apply(&library, &context, &options)
    }

    #[test]
    fn apply_when_sibling_method_declares_wstring_then_field_encoding_used() {
        let result = check_ed3(
            "
FUNCTION_BLOCK FB
  VAR
    s : STRING[10];
    t : STRING[10];
  END_VAR
  METHOD A
    VAR
      t : WSTRING[10];
    END_VAR
  END_METHOD
  METHOD B
    s := t;
  END_METHOD
END_FUNCTION_BLOCK
",
        );
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    fn apply_when_method_local_wstring_assigned_to_field_string_then_p4034() {
        let result = check_ed3(
            "
FUNCTION_BLOCK FB
  VAR
    s : STRING[10];
  END_VAR
  METHOD A
    VAR
      w : WSTRING[10];
    END_VAR
    s := w;
  END_METHOD
END_FUNCTION_BLOCK
",
        );
        assert_eq!(
            result.unwrap_err()[0].code,
            Problem::StringEncodingMismatch.code()
        );
    }

    #[test]
    fn apply_when_method_local_not_string_shadows_wstring_field_then_ok() {
        let result = check_ed3(
            "
FUNCTION_BLOCK FB
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  METHOD A
    VAR
      w : INT;
    END_VAR
    s := w;
  END_METHOD
END_FUNCTION_BLOCK
",
        );
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    fn apply_when_string_assigned_wstring_alias_then_p4034() {
        let result = check(
            "
TYPE WName : WSTRING[10]; END_TYPE
PROGRAM main
  VAR
    s : STRING[10];
    w : WName;
  END_VAR
  s := w;
END_PROGRAM
",
        );
        assert_eq!(
            result.unwrap_err()[0].code,
            Problem::StringEncodingMismatch.code()
        );
    }
}
