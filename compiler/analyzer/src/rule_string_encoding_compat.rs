//! Semantic rule that rejects mixing STRING and WSTRING values.
//!
//! STRING (Latin-1, one byte per character) and WSTRING (UTF-16LE, two bytes
//! per code unit) are distinct types with incompatible runtime encodings
//! (ADR-0016, ADR-0034). There is no implicit conversion between them, so the
//! compiler rejects every place one meets the other with P4034. The VM also
//! traps such a mix at runtime as defense-in-depth, but the compile-time check
//! is the primary guard.
//!
//! A value's encoding is the one its type states: a literal's delimiter
//! (`'abc'` is a `STRING`, `"abc"` a `WSTRING`), a variable's declaration --
//! an array element or a structure field included -- and a string function's
//! result, which the analyzer types by the argument that binds its generic
//! return type. The rule checks each place two encodings meet:
//!
//! * the two sides of a comparison;
//! * the string arguments of one call, those bound to `ANY_STRING` inputs
//!   such as the two of `CONCAT` or `FIND`, which share one encoding;
//! * a value stored into a string variable, element or field.
//!
//! A literal or a function result stored into a named variable is the
//! exception: the assignment and return-type checks already report that
//! mismatch (P4035, P4027), so this rule leaves it to them, and they leave the
//! cases this rule reports to it through [`mixes_encodings`]. Whether the
//! characters of a literal fit its encoding is `rule_string_literal_char_range`
//! (P4052).
//!
//! ## Passes
//!
//! ```ignore
//! VAR
//!     s : STRING[10];
//!     names : ARRAY[1..2] OF STRING[10];
//!     found : INT;
//! END_VAR
//!     names[1] := s;
//!     found := FIND(s, 'a');
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! VAR
//!     s : STRING[10];
//!     w : WSTRING[10];
//!     names : ARRAY[1..2] OF STRING[10];
//!     found : INT;
//!     same : BOOL;
//! END_VAR
//!     s := w;                 (* STRING := WSTRING *)
//!     names[1] := w;          (* the same, into an element *)
//!     found := FIND(w, 'a');  (* a WSTRING searched for a STRING *)
//!     same := w = 'a';        (* a WSTRING compared with a STRING *)
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
use crate::symbol_environment::{ScopeKind, ScopeTracker};
use crate::variable_type;
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

/// The encoding of a string type, or `None` for any other type.
fn encoding_of(representation: &IntermediateType) -> Option<StringType> {
    match representation {
        IntermediateType::String { char_width, .. } => Some(match char_width {
            CharWidth::Narrow => StringType::String,
            CharWidth::Wide => StringType::WString,
        }),
        _ => None,
    }
}

/// The declared encoding of the string variable, array element or structure
/// field that `var` names from `scope`.
fn variable_encoding(
    var: &Variable,
    context: &SemanticContext,
    scope: &ScopeKind,
) -> Option<StringType> {
    let Variable::Symbolic(kind) = var else {
        return None;
    };
    // A bit or partial access answers with the type of the variable it
    // selects from (see `variable_type::of`), not with the value it selects.
    if matches!(
        kind,
        SymbolicVariableKind::BitAccess(_) | SymbolicVariableKind::PartialAccess(_)
    ) {
        return None;
    }
    encoding_of(&variable_type::of(kind, context, scope)?)
}

/// The encoding of the string value `expr` produces, or `None` when it is
/// not a string or its type is not known.
fn expr_encoding(expr: &Expr, context: &SemanticContext, scope: &ScopeKind) -> Option<StringType> {
    match &expr.kind {
        ExprKind::Variable(var) => variable_encoding(var, context, scope),
        ExprKind::Expression(inner) => expr_encoding(inner, context, scope),
        _ => encoding_of(context.types().representation_of_expr(expr)?),
    }
}

/// The encodings of `target` and `value` when `target := value` stores a
/// value of one encoding into a place of the other, and this rule is the one
/// to report it.
///
/// A literal or a function result stored into a named variable is left to
/// the assignment and return-type checks (P4035, P4027).
fn mismatched_assignment(
    target: &Variable,
    value: &Expr,
    context: &SemanticContext,
    scope: &ScopeKind,
) -> Option<(StringType, StringType)> {
    let target_encoding = variable_encoding(target, context, scope)?;
    let value_encoding = match (target, &value.kind) {
        (Variable::Symbolic(SymbolicVariableKind::Named(_)), ExprKind::Variable(var)) => {
            variable_encoding(var, context, scope)?
        }
        (Variable::Symbolic(SymbolicVariableKind::Named(_)), _) => return None,
        _ => expr_encoding(value, context, scope)?,
    };
    (target_encoding != value_encoding).then_some((target_encoding, value_encoding))
}

/// Whether `target := value` stores a string of one encoding into a place of
/// the other: the assignment this rule reports as P4034. Other rules use it
/// to leave that assignment to this one.
pub(crate) fn mixes_encodings(
    target: &Variable,
    value: &Expr,
    context: &SemanticContext,
    scope: &ScopeKind,
) -> bool {
    mismatched_assignment(target, value, context, scope).is_some()
}

impl RuleStringEncodingCompat<'_> {
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

    /// Reports the first string argument of `node` whose encoding differs
    /// from the first one's.
    ///
    /// The arguments bound to a call's `ANY_STRING` inputs are one generic
    /// type, so they share one encoding. A call whose arguments disagree has
    /// one problem, however many arguments differ.
    fn check_string_arguments(&mut self, node: &Function) {
        let Some(signature) = self.context.functions().get(&node.name) else {
            return;
        };
        let any_string = TypeName::from("ANY_STRING");
        let scope = self.scope.current();
        let mut first: Option<StringType> = None;
        for (param, argument) in signature.bind_inputs(&node.param_assignment) {
            if param.param_type != any_string {
                continue;
            }
            let Some(encoding) = expr_encoding(argument, self.context, &scope) else {
                continue;
            };
            match &first {
                None => first = Some(encoding),
                Some(expected) if *expected != encoding => {
                    let expected = expected.clone();
                    self.report(argument.span(), &expected, &encoding);
                    return;
                }
                Some(_) => {}
            }
        }
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
        let scope = self.scope.current();
        if let Some((target_enc, value_enc)) =
            mismatched_assignment(&node.target, &node.value, self.context, &scope)
        {
            self.report(node.span(), &target_enc, &value_enc);
        }
        node.recurse_visit(self)
    }

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<Self::Value, Infallible> {
        let scope = self.scope.current();
        let left_enc = expr_encoding(&node.left, self.context, &scope);
        let right_enc = expr_encoding(&node.right, self.context, &scope);
        if let (Some(left_enc), Some(right_enc)) = (left_enc, right_enc) {
            if left_enc != right_enc {
                self.report(node.left.span(), &left_enc, &right_enc);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        self.check_string_arguments(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{codes, edition3_options, rule_codes};
    use ironplc_parser::options::CompilerOptions;
    use rstest::rstest;

    /// No problems: the encodings agree, or another rule reports the mix.
    const OK: &[Problem] = &[];
    /// The one problem a mix of encodings reports.
    const MIXED: &[Problem] = &[Problem::StringEncodingMismatch];

    /// A program whose body is `body`, with a variable of each encoding, an
    /// array of each, and a structure with a `STRING` field.
    fn program(body: &str) -> String {
        format!(
            "TYPE
    Rec : STRUCT
        f : STRING[10];
    END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
    names : ARRAY[1..2] OF STRING[20];
    wides : ARRAY[1..2] OF WSTRING[20];
    r : Rec;
    found : INT;
    same : BOOL;
  END_VAR
  {body}
END_PROGRAM"
        )
    }

    #[rstest]
    #[case::wstring_with_string_literal("same := w = 'abc';", MIXED)]
    #[case::string_with_wstring_literal("same := s = \"abc\";", MIXED)]
    #[case::string_function_result_with_literal("same := CONCAT(w, w) = 'abab';", MIXED)]
    #[case::array_element_with_variable("same := names[1] = w;", MIXED)]
    #[case::structure_field_with_variable("same := r.f = w;", MIXED)]
    #[case::wstring_with_wstring_literal("same := w = \"abc\";", OK)]
    #[case::array_element_with_literal("same := names[1] = 'abc';", OK)]
    fn apply_when_comparison_then_operands_share_encoding(
        #[case] body: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &program(body), &CompilerOptions::default()),
            codes(expected)
        );
    }

    #[rstest]
    #[case::concat("names[1] := CONCAT(s, w);", MIXED)]
    #[case::find("found := FIND(w, 'cd');", MIXED)]
    #[case::insert("wides[1] := INSERT(w, 'x', 1);", MIXED)]
    #[case::replace("wides[1] := REPLACE(w, 'x', 1, 1);", MIXED)]
    #[case::nested_call("wides[1] := CONCAT(CONCAT(w, w), s);", MIXED)]
    #[case::concat_same_encoding("wides[1] := CONCAT(w, \"x\");", OK)]
    #[case::find_same_encoding("found := FIND(s, 'a');", OK)]
    #[case::one_string_argument("found := LEN(w);", OK)]
    fn apply_when_string_function_then_string_arguments_share_encoding(
        #[case] body: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &program(body), &CompilerOptions::default()),
            codes(expected)
        );
    }

    #[rstest]
    #[case::variable_into_array_element("names[1] := w;", MIXED)]
    #[case::variable_into_structure_field("r.f := w;", MIXED)]
    #[case::function_result_into_array_element("names[1] := CONCAT(w, w);", MIXED)]
    #[case::literal_into_array_element("names[1] := \"abc\";", MIXED)]
    #[case::array_element_into_variable("s := wides[1];", MIXED)]
    #[case::same_encoding_into_array_element("names[1] := s;", OK)]
    #[case::same_encoding_literal_into_field("r.f := 'abc';", OK)]
    // A literal or a function result stored into a named variable is the
    // assignment and return-type checks' to report (P4035, P4027).
    #[case::literal_into_named_variable("s := \"abc\";", OK)]
    #[case::function_result_into_named_variable("s := CONCAT(w, w);", OK)]
    fn apply_when_value_stored_then_encoding_matches_place(
        #[case] body: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &program(body), &CompilerOptions::default()),
            codes(expected)
        );
    }

    rule_err_at!(
        apply_when_string_function_arguments_differ_then_labels_differing_argument,
        "
PROGRAM main
  VAR
    narrow : STRING[10];
    out : STRING[20];
  END_VAR
  out := CONCAT(narrow, \"tail\");
END_PROGRAM
",
        Problem::StringEncodingMismatch,
        "\"tail\""
    );

    /// Each mix is reported once across every rule: the assignment and
    /// return-type checks do not report the ones this rule does.
    #[rstest]
    #[case::array_element_into_variable("s := wides[1];")]
    #[case::variable_into_structure_field("r.f := w;")]
    #[case::literal_into_array_element("names[1] := \"abc\";")]
    #[case::comparison_with_literal("same := w = 'abc';")]
    #[case::string_function_arguments("names[1] := CONCAT(s, w);")]
    fn analyze_when_encodings_mixed_then_pipeline_reports_p4034_once(#[case] body: &str) {
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(&program(body));
        // rule-test-conventions: allow(pipeline) -- shows no other rule reports the same mix
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        let reported: Vec<_> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect();
        assert_eq!(reported, codes(MIXED));
    }

    fn check(source: &str) -> Vec<String> {
        rule_codes(apply, source, &CompilerOptions::default())
    }

    rule_err!(
        apply_when_string_assigned_wstring_then_p4034,
        "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  s := w;
END_PROGRAM
",
        [Problem::StringEncodingMismatch]
    );

    rule_err!(
        apply_when_wstring_assigned_string_then_p4034,
        "
PROGRAM main
  VAR
    s : STRING[10];
    w : WSTRING[10];
  END_VAR
  w := s;
END_PROGRAM
",
        [Problem::StringEncodingMismatch]
    );

    rule_ok!(
        apply_when_string_assigned_string_then_ok,
        "
PROGRAM main
  VAR
    a : STRING[10];
    b : STRING[10];
  END_VAR
  a := b;
END_PROGRAM
"
    );

    rule_ok!(
        apply_when_wstring_assigned_wstring_then_ok,
        "
PROGRAM main
  VAR
    a : WSTRING[10];
    b : WSTRING[10];
  END_VAR
  a := b;
END_PROGRAM
"
    );

    rule_err!(
        apply_when_cross_encoding_comparison_then_p4034,
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
        [Problem::StringEncodingMismatch]
    );

    #[test]
    fn apply_when_wstring_assigned_literal_then_ok() {
        // A wide literal matches a wide target; nothing to flag.
        let codes = check(
            "
PROGRAM main
  VAR
    w : WSTRING[10];
  END_VAR
  w := \"hi\";
END_PROGRAM
",
        );
        assert!(codes.is_empty(), "{codes:?}");
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
        // rule-test-conventions: allow(pipeline) -- shows the rule is wired into analyze
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        let codes: Vec<_> = context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect();
        assert_eq!(codes, [Problem::StringEncodingMismatch.code()]);
    }

    rule_ok!(
        apply_when_sibling_method_declares_wstring_then_field_encoding_used,
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
        edition3_options()
    );

    rule_err_at!(
        apply_when_method_local_wstring_assigned_to_field_string_then_p4034,
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
        Problem::StringEncodingMismatch,
        ":=",
        edition3_options()
    );

    rule_ok!(
        apply_when_method_local_not_string_shadows_wstring_field_then_ok,
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
        edition3_options()
    );

    rule_err!(
        apply_when_string_assigned_wstring_alias_then_p4034,
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
        [Problem::StringEncodingMismatch]
    );
}
