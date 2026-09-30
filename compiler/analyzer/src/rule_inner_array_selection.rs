//! Semantic rule that flags a selection of a whole inner array of an array
//! of arrays as not implemented (P9004).
//!
//! An element of an array of arrays is itself an array: on
//! `rows : ARRAY[1..2] OF Row`, `rows[1]` is a whole `Row`. Reading, writing
//! or passing it is valid IEC 61131-3, but code generation lays an array of
//! arrays out as one multi-dimensional array and only addresses its
//! innermost elements (`rows[1][2]`), so it cannot copy a whole inner array
//! yet. This rule reports the selection so that `check` rejects it too.
//!
//! A selection that gives only some of the subscripts of a multi-dimensional
//! array (`m[1]` on an `ARRAY[1..2, 1..3]`) selects no element; this rule
//! does not report it.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
//! PROGRAM main
//!    VAR
//!       rows : ARRAY[1..2] OF Row;
//!       x : DINT;
//!    END_VAR
//!    x := rows[1][2];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
//! PROGRAM main
//!    VAR
//!       rows : ARRAY[1..2] OF Row;
//!       row : Row;
//!    END_VAR
//!    rows[1] := row;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    selection_type::{ArrayDeclarations, Selections},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
    variable_type::{Declarations, Declared},
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleInnerArraySelection {
            types: context.types(),
            arrays: ArrayDeclarations::from_library(lib),
            declarations: Declarations::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleInnerArraySelection<'a> {
    types: &'a TypeEnvironment,
    arrays: ArrayDeclarations,
    /// The declared type of every variable in scope.
    declarations: Declarations<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleInnerArraySelection<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleInnerArraySelection<'_> {
    /// Reports the subscript chain `node` is the outermost bracket of when
    /// it selects a whole array.
    fn check_chain(&mut self, node: &ArrayVariable) {
        let selections = Selections {
            declarations: &self.declarations,
            types: self.types,
            arrays: &self.arrays,
        };
        let Some(view) = selections.view_of(&SymbolicVariableKind::Array(node.clone())) else {
            return;
        };
        if !selections.is_whole_array(view.clone()) {
            return;
        }
        let described = selections
            .type_name(&view)
            .map(|name| format!("a whole {name}"))
            .unwrap_or_else(|| "a whole array".to_string());
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::InnerArrayAccessNotImplemented,
                Label::span(
                    node.span(),
                    format!(
                        "'{node}' is {described}, an element of an array of arrays; select one of its elements instead"
                    ),
                ),
            )
            .with_context("variable", &node.to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleInnerArraySelection<'_> {
    type Value = ();

    /// Opens a declaration's scope.
    ///
    /// The match stays exhaustive so that a new kind of scope has to say so
    /// rather than silently sharing the enclosing declaration's frame.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        match node {
            ScopeNode::Function(_)
            | ScopeNode::FunctionBlock(_)
            | ScopeNode::Program(_)
            | ScopeNode::Method(_) => self.declarations.enter(),
        }
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.declarations.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        self.declarations
            .add_if(node.identifier.symbolic_id(), Declared::of(node));
        node.recurse_visit(self)
    }

    /// Checks a subscript chain once, from its outermost bracket, then visits
    /// what the chain contains without visiting its inner brackets as chains
    /// of their own: the `rows[1]` in `rows[1][2]` is not a selection.
    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<(), Infallible> {
        self.check_chain(node);
        let mut bracket = node;
        loop {
            for subscript in &bracket.subscripts {
                self.visit_expr(subscript)?;
            }
            match bracket.subscripted_variable.as_ref() {
                SymbolicVariableKind::Array(inner) => bracket = inner,
                base => return self.visit_symbolic_variable_kind(base),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{
        options::{CompilerOptions, Dialect},
        parse_program,
    };
    use ironplc_problems::Problem;
    use rstest::rstest;

    /// The analyzer's diagnostics for `program`, as problem codes.
    fn problems_in(program: &str) -> Vec<String> {
        let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&library], &options).unwrap();
        context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    /// The analyzer's diagnostics for `statement` in a program declaring
    /// arrays of arrays.
    fn problems_of(statement: &str) -> Vec<String> {
        problems_in(&format!(
            "TYPE
    Row : ARRAY[1..3] OF DINT;
    Rows : ARRAY[1..2] OF Row;
    Rec : STRUCT
        t : ARRAY[1..2] OF Row;
        v : ARRAY[1..3] OF DINT;
    END_STRUCT;
END_TYPE
FUNCTION first : DINT
VAR_INPUT
    r : Row;
END_VAR
    first := r[1];
END_FUNCTION
PROGRAM main
VAR
    x : DINT;
    b : BOOL;
    row : Row;
    rows : ARRAY[1..2] OF Row;
    named_rows : Rows;
    grid : ARRAY[1..2, 1..2] OF Row;
    m : ARRAY[1..2, 1..3] OF DINT;
    rec : Rec;
    recs : ARRAY[1..2] OF Rec;
END_VAR
    {statement}
END_PROGRAM"
        ))
    }

    fn count_of(problems: &[String], problem: Problem) -> usize {
        problems
            .iter()
            .filter(|code| *code == problem.code())
            .count()
    }

    #[rstest]
    #[case::target("rows[1] := row;")]
    #[case::named_array_of_arrays_target("named_rows[2] := row;")]
    #[case::two_dimensions_target("grid[1, 2] := row;")]
    #[case::two_dimensions_one_bracket_each_target("grid[1][2] := row;")]
    #[case::argument("x := first(rows[1]);")]
    #[case::comparison("b := rows[1] = rows[2];")]
    #[case::structure_field_target("rec.t[1] := row;")]
    #[case::array_of_structures_field_target("recs[1].t[2] := row;")]
    fn apply_when_inner_array_selected_then_inner_array_access_not_implemented(
        #[case] statement: &str,
    ) {
        assert!(
            count_of(
                &problems_of(statement),
                Problem::InnerArrayAccessNotImplemented
            ) > 0
        );
    }

    #[rstest]
    #[case::element("x := rows[1][2];")]
    #[case::element_target("rows[1][2] := x;")]
    #[case::named_array_of_arrays_element("x := named_rows[2][3];")]
    #[case::two_dimensions_element("x := grid[1, 2][3];")]
    #[case::structure_field_element("x := rec.t[1][2];")]
    #[case::array_of_structures_field_element("x := recs[1].t[2][3];")]
    #[case::array_element("x := row[1];")]
    #[case::whole_array("row := row;")]
    #[case::structure_array_field("x := first(rec.v);")]
    #[case::multi_dimensional_element("x := m[1][2];")]
    #[case::subscript_in_subscript("x := row[rows[1][2]];")]
    fn apply_when_no_inner_array_selected_then_ok(#[case] statement: &str) {
        assert_eq!(
            count_of(
                &problems_of(statement),
                Problem::InnerArrayAccessNotImplemented
            ),
            0
        );
    }

    #[test]
    fn apply_when_inner_array_selected_in_subscript_then_reported() {
        let problems = problems_of("x := row[first(rows[1])];");
        assert_eq!(
            count_of(&problems, Problem::InnerArrayAccessNotImplemented),
            1
        );
    }

    /// The failing example on the P9004 page.
    #[test]
    fn apply_when_p9004_documented_example_then_inner_array_access_not_implemented() {
        let program = "TYPE
    Row : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
    VAR
        rows : ARRAY[1..2] OF Row;
        row  : Row;
    END_VAR
    rows[1] := row;
END_PROGRAM";
        assert_eq!(
            problems_in(program),
            vec![Problem::InnerArrayAccessNotImplemented.code().to_string()]
        );
    }

    /// The corrected example on the P9004 page.
    #[test]
    fn apply_when_p9004_documented_fix_then_ok() {
        let program = "TYPE
    Row : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
    VAR
        rows : ARRAY[1..2] OF Row;
        row  : Row;
        i    : DINT;
    END_VAR
    FOR i := 1 TO 3 DO
        rows[1][i] := row[i];
    END_FOR;
END_PROGRAM";
        assert!(problems_in(program).is_empty());
    }

    #[test]
    fn apply_when_inner_array_selected_then_label_names_selection_and_type() {
        let options = CompilerOptions::default();
        let program = "TYPE
    Row : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
    VAR
        rows : ARRAY[1..2] OF Row;
        row  : Row;
    END_VAR
    rows[1] := row;
END_PROGRAM";
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&library], &options).unwrap();
        assert_eq!(
            context.diagnostics()[0].primary.message,
            "'rows[1]' is a whole Row, an element of an array of arrays; select one of its elements instead"
        );
    }
}
