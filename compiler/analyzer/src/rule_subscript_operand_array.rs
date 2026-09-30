//! Semantic rule that a subscript selects an element of an array.
//!
//! Only an array has elements to select, so a subscript applied to anything
//! else -- a scalar, a `STRING`, a structure, a function block instance, or
//! a field of one of those -- is an error (P4070). An array takes one
//! subscript per dimension: more than it has, or a subscript chain that
//! stops before every dimension has one, is an error too (P4071).
//!
//! The subscripts of a chain count together, as code generation reads them:
//! `m[1][2]` on an `ARRAY[1..2, 1..3]` selects the same element as `m[1, 2]`.
//! A bracket may not reach past the array it starts in, though, so
//! `a[1, 2]` on an `ARRAY[1..2] OF Row` is rejected where `a[1][2]` is not.
//!
//! A reference to an array is subscripted as the array it references,
//! `pa[1]` as `pa^[1]`: code generation indexes the referenced array either
//! way, and a TwinCAT `REFERENCE TO` is always written without the `^`.
//!
//! See section 2.4.1.2.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       values : ARRAY[1..3] OF DINT;
//!       matrix : ARRAY[1..2, 1..3] OF DINT;
//!       x : DINT;
//!    END_VAR
//!    x := values[1];
//!    x := matrix[1, 2];
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
//!    x := x[1];          (* x is not an array *)
//!    x := values[1, 2];  (* values has one dimension *)
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
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::IntermediateType,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
    value_type,
    variable_type::{self, Declarations, Declared},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleSubscriptOperandArray {
            type_environment: context.types(),
            // `Declarations::new` opens the base scope, where declarations
            // made outside any POU land.
            declarations: Declarations::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleSubscriptOperandArray<'a> {
    type_environment: &'a TypeEnvironment,
    /// The declared type of every variable in scope.
    declarations: Declarations<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleSubscriptOperandArray<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The array a subscript chain is part way through.
struct OpenArray<'n> {
    /// The variable the array is, for diagnostics.
    operand: &'n SymbolicVariableKind,
    /// The array's type, for diagnostics.
    array_type: IntermediateType,
    /// What a subscript for every dimension selects.
    element_type: IntermediateType,
    /// The number of dimensions the array has.
    dimensions: usize,
    /// The number of subscripts given so far.
    given: usize,
}

impl RuleSubscriptOperandArray<'_> {
    /// Checks the subscript chain `node` is the outermost bracket of.
    ///
    /// Walks the brackets from the one next to the base variable outwards,
    /// carrying the type each selects. Stops at the first problem, since
    /// every bracket after it would select from a type that is not known.
    fn check_chain(&mut self, node: &ArrayVariable) {
        let mut brackets = vec![node];
        while let SymbolicVariableKind::Array(inner) =
            brackets[brackets.len() - 1].subscripted_variable.as_ref()
        {
            brackets.push(inner);
        }
        brackets.reverse();

        let base = brackets[0].subscripted_variable.as_ref();
        let Some(mut current) = self.operand_type(base) else {
            return;
        };
        let mut open: Option<OpenArray> = None;

        for bracket in brackets {
            let count = bracket.subscripts.len();
            let mut array = match open.take() {
                Some(array) => array,
                None => {
                    let operand = bracket.subscripted_variable.as_ref();
                    let IntermediateType::Array {
                        element_type,
                        dimensions,
                    } = &current
                    else {
                        self.report_not_array(operand, &current);
                        return;
                    };
                    if dimensions.is_empty() {
                        // The analyzer does not know how many dimensions
                        // this array has, so it cannot count the
                        // subscripts; the bracket selects an element.
                        current = element_type.as_ref().clone();
                        continue;
                    }
                    OpenArray {
                        operand,
                        array_type: current.clone(),
                        element_type: element_type.as_ref().clone(),
                        dimensions: dimensions.len(),
                        given: 0,
                    }
                }
            };

            array.given += count;
            if array.given > array.dimensions {
                self.report_count_mismatch(&array);
                return;
            }
            if array.given == array.dimensions {
                current = array.element_type;
            } else {
                open = Some(array);
            }
        }

        if let Some(array) = open {
            self.report_count_mismatch(&array);
        }
    }

    /// The type of the variable a subscript chain starts from, or `None`
    /// when it is not resolved (another rule reports an undeclared name).
    ///
    /// A reference to an array answers with the array; see the module
    /// documentation.
    fn operand_type(&self, base: &SymbolicVariableKind) -> Option<IntermediateType> {
        match variable_type::of(base, &self.declarations, self.type_environment)? {
            IntermediateType::Reference { target_type } if target_type.is_array() => {
                Some(*target_type)
            }
            other => Some(other),
        }
    }

    /// The type of `operand` as a diagnostic shows it: an elementary type by
    /// its keyword, a variable of a declared type by that type's name, and
    /// anything else by its shape.
    fn describe(&self, operand: &SymbolicVariableKind, operand_type: &IntermediateType) -> String {
        let elementary = self
            .type_environment
            .elementary_type_name_for(operand_type)
            .is_some();
        if let (false, SymbolicVariableKind::Named(named)) = (elementary, operand) {
            if let Some(id) = self
                .declarations
                .find(&named.name)
                .and_then(|declared: &Declared| declared.type_id(self.type_environment))
            {
                return value_type::describe(self.type_environment, id);
            }
        }
        value_type::describe_representation(self.type_environment, operand_type)
    }

    fn report_not_array(
        &mut self,
        operand: &SymbolicVariableKind,
        operand_type: &IntermediateType,
    ) {
        let described = self.describe(operand, operand_type);
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::SubscriptNotArray,
                Label::span(
                    operand.span(),
                    format!(
                        "'{operand}' is {described}, not an array, so it cannot be subscripted"
                    ),
                ),
            )
            .with_context("variable", &operand.to_string())
            .with_context("type", &described),
        );
    }

    fn report_count_mismatch(&mut self, array: &OpenArray) {
        let described = self.describe(array.operand, &array.array_type);
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ArraySubscriptCountMismatch,
                Label::span(
                    array.operand.span(),
                    format!(
                        "'{}' is {described}, which takes {} subscript(s), but is given {}",
                        array.operand, array.dimensions, array.given
                    ),
                ),
            )
            .with_context("variable", &array.operand.to_string())
            .with_context("dimensions", &array.dimensions.to_string())
            .with_context("subscripts", &array.given.to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleSubscriptOperandArray<'_> {
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
    /// of their own.
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

    /// The problem codes the analyzer reports for `program`.
    fn problems_in(program: &str, options: &CompilerOptions) -> Vec<String> {
        let library = parse_program(program, &FileId::default(), options).unwrap();
        let (_library, context) = analyze(&[&library], options).unwrap();
        context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    /// The problem codes the analyzer reports for `statement` in a program
    /// that declares a variable of each kind. Edition 3, for `REF_TO`.
    fn problems_of(statement: &str) -> Vec<String> {
        let program = format!(
            "TYPE
    Rec : STRUCT
        n : DINT;
        v : ARRAY[1..3] OF DINT;
        m : ARRAY[1..2, 1..3] OF DINT;
    END_STRUCT;
    Row : ARRAY[1..3] OF DINT;
END_TYPE
FUNCTION_BLOCK Fb
VAR_OUTPUT
    o : DINT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    x : DINT;
    q : STRING;
    rec : Rec;
    fb : Fb;
    a : ARRAY[1..3] OF DINT;
    m : ARRAY[1..2, 1..3] OF DINT;
    recs : ARRAY[1..2] OF Rec;
    row : Row;
    rows : ARRAY[1..2] OF Row;
    pa : REF_TO ARRAY[1..3] OF DINT;
    pd : REF_TO DINT;
END_VAR
    {statement}
END_PROGRAM"
        );
        problems_in(
            &program,
            &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        )
    }

    fn only(problem: Problem) -> Vec<String> {
        vec![problem.code().to_string()]
    }

    /// How many times `problems` holds `problem`.
    fn count_of(problems: &[String], problem: Problem) -> usize {
        problems
            .iter()
            .filter(|code| *code == problem.code())
            .count()
    }

    #[rstest]
    #[case::scalar("x[1] := 5;")]
    #[case::scalar_read("x := x[1];")]
    #[case::string("q := q[1];")]
    #[case::structure("x := rec[1].n;")]
    #[case::function_block_instance("x := fb[1].o;")]
    #[case::structure_field("x := rec.n[1];")]
    #[case::function_block_output("x := fb.o[1];")]
    #[case::array_element("x := a[1][2];")]
    #[case::array_of_structures_element_field("x := recs[1].n[2];")]
    #[case::dereference_of_scalar("x := pd^[1];")]
    #[case::reference_to_scalar("x := pd[1];")]
    #[case::inside_a_subscript("x := a[x[1]];")]
    fn apply_when_subscripted_variable_not_array_then_subscript_not_array(#[case] statement: &str) {
        assert_eq!(problems_of(statement), only(Problem::SubscriptNotArray));
    }

    #[rstest]
    #[case::too_many("x := a[1, 2];")]
    #[case::too_many_two_dimensions("x := m[1, 2, 3];")]
    #[case::too_many_split("x := m[1][2, 3];")]
    #[case::too_few("a := m[1];")]
    #[case::structure_field_too_many("x := rec.v[1, 2];")]
    #[case::structure_field_too_few("a := rec.m[1];")]
    #[case::dereference_too_many("x := pa^[1, 2];")]
    #[case::array_of_structures_too_many("x := recs[1, 2].n;")]
    #[case::array_of_arrays_in_one_bracket("x := rows[1, 2];")]
    fn apply_when_subscript_count_not_dimensions_then_array_subscript_count_mismatch(
        #[case] statement: &str,
    ) {
        // A chain that stops short, or runs into an array of arrays, also
        // has a value of the wrong type, which the assignment rules report.
        let problems = problems_of(statement);
        assert_eq!(count_of(&problems, Problem::ArraySubscriptCountMismatch), 1);
        assert_eq!(count_of(&problems, Problem::SubscriptNotArray), 0);
    }

    #[rstest]
    #[case::one_dimension("x := a[1];")]
    #[case::one_dimension_target("a[1] := x;")]
    #[case::two_dimensions("x := m[1, 2];")]
    #[case::two_dimensions_chained("x := m[1][2];")]
    #[case::named_array_type("x := row[1];")]
    #[case::structure_field("x := rec.v[2];")]
    #[case::structure_field_two_dimensions("x := rec.m[1, 2];")]
    #[case::array_of_structures_field("x := recs[1].v[2];")]
    #[case::dereference("x := pa^[2];")]
    #[case::reference("x := pa[2];")]
    #[case::subscript_in_subscript("x := a[a[1]];")]
    fn apply_when_subscripts_select_element_then_ok(#[case] statement: &str) {
        assert!(problems_of(statement).is_empty());
    }

    #[test]
    fn apply_when_array_of_arrays_subscripted_per_array_then_no_subscript_problem() {
        let problems = problems_of("x := rows[1][2];");
        assert_eq!(count_of(&problems, Problem::SubscriptNotArray), 0);
        assert_eq!(count_of(&problems, Problem::ArraySubscriptCountMismatch), 0);
    }

    #[test]
    fn apply_when_subscripted_variable_undeclared_then_no_subscript_problem() {
        let problems = problems_of("x := nope[1];");
        assert!(!problems.is_empty());
        assert_eq!(count_of(&problems, Problem::SubscriptNotArray), 0);
    }

    #[rstest]
    #[case::array("r := g[1];", vec![])]
    #[case::scalar("r := k[1];", only(Problem::SubscriptNotArray))]
    #[case::too_many("r := g[1, 2];", only(Problem::ArraySubscriptCountMismatch))]
    fn apply_when_external_variable_subscripted_then_checked_as_its_global(
        #[case] statement: &str,
        #[case] expected: Vec<String>,
    ) {
        let program = format!(
            "TYPE
    Row : ARRAY[1..3] OF DINT;
END_TYPE
PROGRAM main
VAR_EXTERNAL
    g : Row;
    k : DINT;
END_VAR
VAR
    r : DINT;
END_VAR
    {statement}
END_PROGRAM
CONFIGURATION config
VAR_GLOBAL
    g : Row;
    k : DINT;
END_VAR
RESOURCE res ON PLC
    TASK t(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
END_RESOURCE
END_CONFIGURATION"
        );
        assert_eq!(problems_in(&program, &CompilerOptions::default()), expected);
    }

    #[rstest]
    #[case::array("r := io[1];", vec![])]
    #[case::scalar("r := r[1];", only(Problem::SubscriptNotArray))]
    fn apply_when_function_in_out_subscripted_then_checked(
        #[case] statement: &str,
        #[case] expected: Vec<String>,
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
        assert_eq!(problems_in(&program, &CompilerOptions::default()), expected);
    }

    #[rstest]
    #[case::array("r := ra[1];", vec![])]
    #[case::scalar("r := rd[1];", only(Problem::SubscriptNotArray))]
    fn apply_when_reference_to_subscripted_then_checked_as_referenced_type(
        #[case] statement: &str,
        #[case] expected: Vec<String>,
    ) {
        let program = format!(
            "PROGRAM main
VAR
    ra : REFERENCE TO ARRAY[1..3] OF DINT;
    rd : REFERENCE TO DINT;
    r : DINT;
END_VAR
    {statement}
END_PROGRAM"
        );
        let options = CompilerOptions {
            allow_reference_to: true,
            ..CompilerOptions::default()
        };
        assert_eq!(problems_in(&program, &options), expected);
    }

    /// The failing example on the P4070 page.
    #[test]
    fn apply_when_p4070_documented_example_then_two_subscript_not_array() {
        let program = "TYPE
    Reading : STRUCT
        level : DINT;
    END_STRUCT;
END_TYPE
PROGRAM main
    VAR
        count  : DINT;
        sensor : Reading;
    END_VAR
    count[1] := 5;
    count := sensor.level[1];
END_PROGRAM";
        let code = Problem::SubscriptNotArray.code().to_string();
        assert_eq!(
            problems_in(program, &CompilerOptions::default()),
            vec![code.clone(), code]
        );
    }

    /// The failing example on the P4071 page.
    #[test]
    fn apply_when_p4071_documented_example_then_two_array_subscript_count_mismatch() {
        let program = "PROGRAM main
    VAR
        values : ARRAY[1..3] OF DINT;
        matrix : ARRAY[1..2, 1..3] OF DINT;
        result : DINT;
    END_VAR
    result := values[1, 2];
    result := matrix[1, 2, 3];
END_PROGRAM";
        let code = Problem::ArraySubscriptCountMismatch.code().to_string();
        assert_eq!(
            problems_in(program, &CompilerOptions::default()),
            vec![code.clone(), code]
        );
    }

    #[rstest]
    #[case::elementary("x[1] := 5;", "'x' is DINT, not an array, so it cannot be subscripted")]
    #[case::declared_type(
        "x := rec[1].n;",
        "'rec' is Rec, not an array, so it cannot be subscripted"
    )]
    #[case::field(
        "x := rec.n[1];",
        "'rec.n' is DINT, not an array, so it cannot be subscripted"
    )]
    #[case::count(
        "x := m[1, 2, 3];",
        "'m' is ARRAY[1..2, 1..3] OF DINT, which takes 2 subscript(s), but is given 3"
    )]
    fn apply_when_subscript_rejected_then_label_names_variable_and_type(
        #[case] statement: &str,
        #[case] expected: &str,
    ) {
        let program = format!(
            "TYPE
    Rec : STRUCT
        n : DINT;
    END_STRUCT;
END_TYPE
PROGRAM main
VAR
    x : DINT;
    rec : Rec;
    m : ARRAY[1..2, 1..3] OF DINT;
END_VAR
    {statement}
END_PROGRAM"
        );
        let options = CompilerOptions::default();
        let library = parse_program(&program, &FileId::default(), &options).unwrap();
        let (_library, context) = analyze(&[&library], &options).unwrap();
        let labels: Vec<String> = context
            .diagnostics()
            .iter()
            .map(|d| d.primary.message.clone())
            .collect();
        assert_eq!(labels, vec![expected.to_string()]);
    }
}
