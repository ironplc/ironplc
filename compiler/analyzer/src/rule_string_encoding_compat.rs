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
//! It does not flag string literals (whose encoding adapts to the assignment
//! target) or the results of string functions (whose encoding the analyzer
//! collapses to a single `STRING` type name); those narrower cases rely on the
//! runtime trap.
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

use std::collections::HashMap;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::result::SemanticResult;
use crate::semantic_context::SemanticContext;
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleStringEncodingCompat {
        diagnostics: vec![],
        string_vars: HashMap::new(),
    };

    visitor.walk(lib).map_err(|e| vec![e])?;

    if visitor.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(visitor.diagnostics)
    }
}

struct RuleStringEncodingCompat {
    diagnostics: Vec<Diagnostic>,
    /// Declared encoding of each named string variable in the current POU.
    string_vars: HashMap<Id, StringType>,
}

/// Returns the declared string encoding of a simple named variable, if it is a
/// string variable tracked in `string_vars`.
fn named_variable_encoding<'a>(
    var: &Variable,
    string_vars: &'a HashMap<Id, StringType>,
) -> Option<&'a StringType> {
    match var {
        Variable::Symbolic(SymbolicVariableKind::Named(named)) => string_vars.get(&named.name),
        _ => None,
    }
}

/// Returns the declared string encoding of an expression when it is a simple
/// named string variable. Literals and complex expressions return `None`.
fn expr_string_encoding<'a>(
    expr: &Expr,
    string_vars: &'a HashMap<Id, StringType>,
) -> Option<&'a StringType> {
    match &expr.kind {
        ExprKind::Variable(var) => named_variable_encoding(var, string_vars),
        _ => None,
    }
}

impl RuleStringEncodingCompat {
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

impl Visitor<Diagnostic> for RuleStringEncodingCompat {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.string_vars.clear();
        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.string_vars.clear();
        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.string_vars.clear();
        node.recurse_visit(self)
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if let VariableIdentifier::Symbol(ref id) = node.identifier {
            if let InitialValueAssignmentKind::String(ref string_init) = node.initializer {
                self.string_vars
                    .insert(id.clone(), string_init.width.clone());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Diagnostic> {
        let target_enc = named_variable_encoding(&node.target, &self.string_vars).cloned();
        let value_enc = expr_string_encoding(&node.value, &self.string_vars).cloned();
        if let (Some(target_enc), Some(value_enc)) = (target_enc, value_enc) {
            if target_enc != value_enc {
                self.report(node.span(), &target_enc, &value_enc);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<Self::Value, Diagnostic> {
        let left_enc = expr_string_encoding(&node.left, &self.string_vars).cloned();
        let right_enc = expr_string_encoding(&node.right, &self.string_vars).cloned();
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
    use crate::test_helpers::parse_and_resolve_types_with_context;
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
        // A literal's encoding adapts to the target; not flagged here.
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
}
