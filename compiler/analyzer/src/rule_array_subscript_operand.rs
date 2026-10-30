//! Semantic rule that a subscript list subscripts an array.
//!
//! Only an array has elements for a subscript to select, so a list on
//! anything else -- an elementary value, a `STRING`, a structure, a function
//! block instance, a field of one of those, or an element of an array that is
//! not itself an array (`a[1][2]` on a one-dimensional `a`) -- is an error.
//! A list on a reference is judged against what the reference refers to, as
//! the other subscript rules judge it ([`variable_type::subscripted_type_id`]):
//! `pa[1]` on a reference to an array is accepted, and `pd[1]` on a reference
//! to a `DINT` is reported.
//!
//! CODESYS and TwinCAT give a subscript of a `STRING`, a `WSTRING` and a
//! `POINTER TO` a meaning of their own (a character code, and pointer
//! arithmetic). IronPLC does not, so the rule reports them in every dialect,
//! as it reports any other subscript of something that is not an array. See
//! <https://github.com/ironplc/ironplc/issues/2532>.
//!
//! A list whose subscripted variable has no type is not this rule's to judge:
//! an undeclared name, a field the record does not have, and a list that
//! another list before it left without a type (`m[1][2]` on a
//! two-dimensional `m`, reported by `rule_array_subscript_count`) are each
//! reported where the type was lost.
//!
//! See section 2.4.1.2.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       values : ARRAY[1..3] OF DINT;
//!       x : DINT;
//!    END_VAR
//!    x := values[1];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       values : ARRAY[1..3] OF DINT;
//!       x : DINT;
//!    END_VAR
//!    x := x[1];          (* x is a DINT *)
//!    x := values[1][2];  (* values[1] is a DINT *)
//! END_PROGRAM
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{ArrayVariable, SymbolicVariableKind},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    symbol_environment::ScopeTracker,
    value_type, variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArraySubscriptOperand {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Checks that the variable each subscript list subscripts is an array, which
/// the type environment answers for.
struct RuleArraySubscriptOperand<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to type the subscripted variable from.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl RuleArraySubscriptOperand<'_> {
    /// The type `node` subscripts, as a diagnostic shows it, when it is not
    /// an array; `None` when it is one or its type is not known.
    ///
    /// A bare `THIS^` or `SUPER^` has no type of its own, since it is an
    /// instance rather than a value, but it names a function block, which is
    /// not an array. Where it names none, `rule_self_reference_context`
    /// reports it.
    fn not_array(&self, node: &ArrayVariable) -> Option<String> {
        let scope = self.scope.current();
        if let SymbolicVariableKind::SelfRef(self_ref) = node.subscripted_variable.as_ref() {
            let block = self.context.symbols().self_type(&scope, self_ref.kind)?;
            return Some(block.to_string());
        }
        let types = self.context.types();
        let subscripted =
            variable_type::subscripted_type_id(node, self.context.symbols(), types, &scope)?;
        if let SemanticType::Array { .. } = types.get_by_id(subscripted)?.representation {
            return None;
        }
        Some(value_type::describe(types, subscripted))
    }
}

impl DiagnosticVisitor for RuleArraySubscriptOperand<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleArraySubscriptOperand<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<(), Infallible> {
        // The subscripted variable and the subscripts first, so that the
        // lists of one reference are reported in the order they are written.
        node.recurse_visit(self)?;
        if let Some(actual) = self.not_array(node) {
            // The list, not the variable: `a[1][2]` subscripts `a[1]`, whose
            // span is the array `a`.
            let span = match (node.subscripts.first(), node.subscripts.last()) {
                (Some(first), Some(last)) => SourceSpan::join2(first, last),
                _ => node.span(),
            };
            self.diagnostics.push(
                Diagnostic::problem(Problem::SubscriptNotArray, Label::span(span, "Subscripts"))
                    .with_context("variable", &node.subscripted_variable.to_string())
                    .with_context("type", &actual),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        codes, configuration_with_globals, diagnostic_codes, edition3_options, rule_codes,
        rule_diagnostics,
    };
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// No problems: every list subscripts an array.
    const OK: &[Problem] = &[];
    /// The one problem a list on something that is not an array reports.
    const NOT_ARRAY: &[Problem] = &[Problem::SubscriptNotArray];

    /// A program that declares a variable of each kind a list may be written
    /// on, and whose body reads `access` into `x`.
    fn reading(access: &str) -> String {
        format!(
            "TYPE
    Row : ARRAY[1..3] OF DINT;
    Grid : ARRAY[1..2, 1..3] OF DINT;
    Level : DINT;
    Rec : STRUCT n : DINT; v : Row; END_STRUCT;
END_TYPE

FUNCTION_BLOCK Fb
VAR_OUTPUT
    o : DINT;
    v : Row;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    x : DINT;
    w : WORD;
    level : Level;
    q : STRING;
    wq : WSTRING;
    rec : Rec;
    fb : Fb;
    a : ARRAY[1..3] OF DINT;
    m : Grid;
    row : Row;
    rows : ARRAY[1..2] OF Row;
    recs : ARRAY[1..2] OF Rec;
    pa : REF_TO Row;
    pd : REF_TO DINT;
    ra : REFERENCE TO Row;
    rd : REFERENCE TO DINT;
END_VAR
    x := {access};
END_PROGRAM"
        )
    }

    /// The options [`reading`] parses under: edition 3 for `REF_TO`, with
    /// the TwinCAT `REFERENCE TO`.
    fn options() -> CompilerOptions {
        CompilerOptions {
            allow_reference_to: true,
            ..edition3_options()
        }
    }

    /// REQ-ETR-analyzer-011: a subscript list on something that is not an
    /// array, nor a reference to one, is reported.
    #[spec_test(REQ_ETR_analyzer_011)]
    #[rstest]
    #[case::array("a[1]", OK)]
    #[case::named_array_type("row[1]", OK)]
    #[case::two_dimensions("m[1, 2]", OK)]
    #[case::array_of_arrays("rows[1][2]", OK)]
    #[case::structure_field("rec.v[2]", OK)]
    #[case::function_block_field("fb.v[2]", OK)]
    #[case::array_of_structures_field("recs[1].v[2]", OK)]
    #[case::reference("pa[2]", OK)]
    #[case::dereference("pa^[2]", OK)]
    #[case::reference_to("ra[2]", OK)]
    #[case::elementary("x[1]", NOT_ARRAY)]
    #[case::bit_string("w[1]", NOT_ARRAY)]
    #[case::alias_of_elementary("level[1]", NOT_ARRAY)]
    #[case::string("q[1]", NOT_ARRAY)]
    #[case::wide_string("wq[1]", NOT_ARRAY)]
    #[case::structure("rec[1].n", NOT_ARRAY)]
    #[case::function_block_instance("fb[1].o", NOT_ARRAY)]
    #[case::structure_field_not_array("rec.n[1]", NOT_ARRAY)]
    #[case::function_block_field_not_array("fb.o[1]", NOT_ARRAY)]
    #[case::element_not_array("a[1][2]", NOT_ARRAY)]
    #[case::element_of_array_of_arrays_not_array("rows[1][2][3]", NOT_ARRAY)]
    #[case::array_of_structures_element("recs[1][2]", NOT_ARRAY)]
    #[case::array_of_structures_field_not_array("recs[1].n[2]", NOT_ARRAY)]
    #[case::reference_to_elementary("pd[1]", NOT_ARRAY)]
    #[case::dereference_of_elementary("pd^[1]", NOT_ARRAY)]
    #[case::reference_to_to_elementary("rd[1]", NOT_ARRAY)]
    #[case::inside_subscript("a[x[1]]", NOT_ARRAY)]
    fn apply_when_subscript_list_then_subscripted_variable_is_array(
        #[case] access: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &reading(access), &options()),
            codes(expected)
        );
    }

    rule_ok!(
        apply_when_subscripted_variable_undeclared_then_ok,
        "PROGRAM main
VAR
    x : DINT;
END_VAR
    x := nope[1];
END_PROGRAM"
    );

    // `m[1]` gives too few subscripts, so it has no type to subscript again;
    // `rule_array_subscript_count` reports it.
    rule_ok!(
        apply_when_subscripted_list_has_no_type_then_ok,
        "PROGRAM main
VAR
    m : ARRAY[1..2, 1..3] OF DINT;
    x : DINT;
END_VAR
    x := m[1][2];
END_PROGRAM"
    );

    rule_err_at!(
        apply_when_element_subscripted_again_then_labels_subscripts,
        "PROGRAM main
VAR
    a : ARRAY[1..3] OF DINT;
    i : DINT;
END_VAR
    a[i][i + 1] := 5;
END_PROGRAM",
        Problem::SubscriptNotArray,
        "i + 1"
    );

    rule_err!(
        apply_when_this_subscripted_then_error,
        "FUNCTION_BLOCK Fb
VAR
    x : DINT;
END_VAR
    x := THIS^[1].x;
END_FUNCTION_BLOCK",
        [Problem::SubscriptNotArray],
        edition3_options()
    );

    // `THIS^` outside a function block names no instance;
    // `rule_self_reference_context` reports it.
    rule_ok!(
        apply_when_this_subscripted_outside_function_block_then_ok,
        "PROGRAM main
VAR
    x : DINT;
END_VAR
    x := THIS^[1].x;
END_PROGRAM",
        edition3_options()
    );

    #[rstest]
    #[case::array("r := g[1];", OK)]
    #[case::not_array("r := k[1];", NOT_ARRAY)]
    fn apply_when_external_subscripted_then_checked_as_declared(
        #[case] statement: &str,
        #[case] expected: &[Problem],
    ) {
        let program = format!(
            "PROGRAM main
VAR_EXTERNAL
    g : ARRAY[1..3] OF DINT;
    k : DINT;
END_VAR
VAR
    r : DINT;
END_VAR
    {statement}
END_PROGRAM
{}",
            configuration_with_globals("g : ARRAY[1..3] OF DINT; k : DINT;")
        );
        assert_eq!(
            rule_codes(apply, &program, &CompilerOptions::default()),
            codes(expected)
        );
    }

    #[rstest]
    #[case::array("r := io[1];", OK)]
    #[case::not_array("r := r[1];", NOT_ARRAY)]
    fn apply_when_function_in_out_subscripted_then_checked(
        #[case] statement: &str,
        #[case] expected: &[Problem],
    ) {
        let program = format!(
            "FUNCTION f : DINT
VAR_IN_OUT
    io : ARRAY[1..3] OF DINT;
END_VAR
VAR
    r : DINT;
END_VAR
    {statement}
    f := r;
END_FUNCTION"
        );
        assert_eq!(
            rule_codes(apply, &program, &CompilerOptions::default()),
            codes(expected)
        );
    }

    #[test]
    fn apply_when_not_array_then_names_variable_and_type() {
        let diagnostics = rule_diagnostics(apply, &reading("rec.n[1]"), &options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::SubscriptNotArray.code()]
        );
        assert!(diagnostics[0]
            .described
            .contains(&"variable=rec.n".to_owned()));
        assert!(diagnostics[0].described.contains(&"type=dint".to_owned()));
    }

    // The examples of `docs/reference/compiler/problems/P4096.rst`.
    const DOCUMENTED_PROGRAM: &str = "
TYPE
  Reading : STRUCT
    level : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    count : DINT;
    sensor : Reading;
  END_VAR
  count[1] := 5;
  count := sensor.level[1];
END_PROGRAM";

    rule_err!(
        apply_when_documented_failing_example_then_error,
        DOCUMENTED_PROGRAM,
        [Problem::SubscriptNotArray; 2]
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        &DOCUMENTED_PROGRAM
            .replace("count : DINT", "counts : ARRAY[1..4] OF DINT")
            .replace("level : DINT", "level : ARRAY[1..4] OF DINT")
            .replace("count[1] := 5", "counts[1] := 5")
            .replace("count := sensor", "counts[2] := sensor")
    );
}
