//! Requires both sides of a whole-aggregate assignment to have identical
//! declared types.
//!
//! IEC 61131-3 §7.3.3.1 makes assignment over a multi-element variable a value
//! copy. Codegen implements that with `COPY_REGION`, whose length the VM
//! derives from the two array descriptors — so the two ends must describe the
//! same shape. The VM cross-checks the derived byte sizes and traps
//! (`RegionSizeMismatch`), but that is a backstop against a compiler defect:
//! declared-type equality is a static property and belongs here.
//!
//! Descriptors cannot tell `ARRAY[1..6] OF INT` from `ARRAY[1..2,1..3] OF INT`
//! (same element count, same element type), so the runtime check would accept
//! that pair. This rule is what rejects it.
//!
//! ## Scope
//!
//! Fires when the assignment *target* is an array or a structure: a whole
//! variable, an element of an array of arrays or of structures
//! (`rows[1] := row`), a field (`s.inner := p`), or what a reference refers
//! to (`p^ := q`). Two neighbouring cases belong elsewhere:
//!
//! * Scalar assignment is deliberately untouched — checking it is the right
//!   end state but interacts with implicit widening (ADR-0029, ADR-0031) and
//!   would reject programs that compile today.
//! * A function result is left to `rule_function_call_type_check` (P4027),
//!   which already compares a call's return type against its assignment
//!   destination. Reporting here as well would produce two diagnostics for
//!   one mistake.
//!
//! The source may be a variable, an element (`row := rows[1]`), a field of a
//! structure or of a function block instance (`r := f.p`), what a reference
//! refers to, or the output an output assignment stores
//! (`f(p => q)`), which is checked as `q := f.p` after the call is
//! (`specs/design/call-binding.md`). A negated output is not an aggregate,
//! and `rule_function_call_type_check` reports it (P4083).
//!
//! Everything else reaching an aggregate target is rejected, including a
//! source whose type does not resolve at all. Code generation relies on
//! that: a copy between places of different types breaks an invariant of the
//! lowered program, so it is a compiler defect rather than a bad program.
//!
//! ## Examples
//!
//! ```ignore
//! VAR
//!     a : ARRAY[1..2] OF DINT;
//!     b : ARRAY[1..2] OF DINT;
//!     c : ARRAY[1..5] OF DINT;
//! END_VAR
//!     a := b;   (* ok *)
//!     a := c;   (* P2037: different extents *)
//! ```
//!
//! An element of an array of arrays is an array, and is checked as one:
//!
//! ```ignore
//! TYPE Row : ARRAY[1..2] OF DINT; END_TYPE
//! VAR
//!     rows : ARRAY[1..3] OF Row;
//!     row : Row;
//!     c : ARRAY[1..5] OF DINT;
//! END_VAR
//!     rows[1] := row;      (* ok *)
//!     row := rows[2];      (* ok *)
//!     rows[1] := c;        (* P2037: different extents *)
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
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    symbol_environment::ScopeTracker,
    variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleAggregateAssignment {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleAggregateAssignment<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleAggregateAssignment<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'a> RuleAggregateAssignment<'a> {
    /// The [`SemanticType`] of the value the reference `kind` names: a
    /// variable, an element, a field or what a reference refers to, by the
    /// walk [`variable_type::type_id`] makes, so a named type (`p : Point`)
    /// and one spelled out in place (`a : ARRAY[1..2] OF DINT`) both state
    /// their shape. With `deref`, what the reference `kind` names refers to,
    /// as the target of `p^ := v`.
    fn selected_type(&self, kind: &SymbolicVariableKind, deref: bool) -> Option<&'a SemanticType> {
        let types = self.context.types();
        let id =
            variable_type::type_id(kind, self.context.symbols(), types, &self.scope.current())?;
        let id = if deref {
            types.referenced_type(id)?
        } else {
            id
        };
        Some(&types.get_by_id(id)?.representation)
    }

    /// P2037: assignment of a whole array or structure requires identical
    /// types. `deref` is set for `p^ := value`, which assigns what `target`
    /// refers to.
    fn check_aggregate_assignment(&mut self, target: &Variable, deref: bool, value: &Expr) {
        // A directly represented variable is never an array or a structure.
        let Variable::Symbolic(target) = target else {
            return;
        };
        let Some(target_type) = self.selected_type(target, deref) else {
            return;
        };
        if !matches!(
            target_type,
            SemanticType::Array { .. } | SemanticType::Structure { .. }
        ) {
            return;
        }
        // P4027 owns a call's return type against its destination.
        if matches!(value.kind, ExprKind::Function(_)) {
            return;
        }

        let value_type = match &value.kind {
            ExprKind::Variable(Variable::Symbolic(source)) => self.selected_type(source, false),
            // What a reference refers to (`p^`), and the output a call
            // stores, have the type the analyzer recorded for them: the
            // referenced type, and the type the output is declared with.
            ExprKind::Deref(_) | ExprKind::CallOutput(_) => {
                self.context.types().representation_of_expr(value)
            }
            _ => None,
        };
        // An unresolvable source is a mismatch too: nothing other than a
        // same-typed aggregate may be assigned to an aggregate.
        if value_type != Some(target_type) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::AggregateAssignmentTypeMismatch,
                Label::span(
                    value.span(),
                    "Assignment between arrays or structures requires identical types",
                ),
            ));
        }
    }
}

impl Visitor<Infallible> for RuleAggregateAssignment<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.check_aggregate_assignment(&node.target, node.deref, &node.value);
        node.recurse_visit(self)
    }

    /// An output assignment stores the value the binding pass recorded for
    /// it, the output the call leaves, so it is checked as `q := f.p` after
    /// the call is.
    fn visit_output(&mut self, node: &Output) -> Result<(), Infallible> {
        if let (Some(value), false) = (&node.value, node.not) {
            self.check_aggregate_assignment(&node.tgt, false, value);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use crate::test_helpers::{codes, edition3_options, fb_inheritance_options, rule_codes};
    use ironplc_problems::Problem;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// No problems: both sides have identical types.
    const OK: &[Problem] = &[];
    /// The one problem an assignment between different types reports.
    const MISMATCH: &[Problem] = &[Problem::AggregateAssignmentTypeMismatch];

    /// A program with an array of arrays `rows` of `Row`, an array of
    /// structures `ps` of `P`, a structure `o` with a field of `P`, a
    /// reference `rp` to a `Row`, and a variable of each of those element
    /// types and of others, whose body is `body`.
    fn elements_and_fields(body: &str) -> String {
        format!(
            "
TYPE
  Row : ARRAY[1..3] OF DINT;
  P : STRUCT a : DINT; END_STRUCT;
  Wide : STRUCT a : DINT; b : DINT; END_STRUCT;
  Outer : STRUCT inner : P; END_STRUCT;
END_TYPE

PROGRAM main
VAR
  rows : ARRAY[1..2] OF Row;
  row : Row;
  c : ARRAY[1..5] OF DINT;
  ps : ARRAY[1..2] OF P;
  q : P;
  w : Wide;
  o : Outer;
  rp : REF_TO Row;
END_VAR
  {body}
END_PROGRAM
"
        )
    }

    #[rstest]
    #[case::element_target("rows[1] := row;", OK)]
    #[case::element_source("row := rows[2];", OK)]
    #[case::element_target_and_source("rows[1] := rows[2];", OK)]
    #[case::structure_element_source("q := ps[2];", OK)]
    #[case::structure_element_target("ps[1] := q;", OK)]
    #[case::field_target("o.inner := ps[1];", OK)]
    #[case::dereference_target("rp^ := row;", OK)]
    #[case::dereference_source("rows[1] := rp^;", OK)]
    #[case::element_target_other_extent("rows[1] := c;", MISMATCH)]
    #[case::element_source_other_extent("c := rows[1];", MISMATCH)]
    #[case::element_target_constant("rows[1] := 5;", MISMATCH)]
    #[case::element_target_scalar_element("rows[1] := rows[1][2];", MISMATCH)]
    #[case::element_target_structure("rows[1] := q;", MISMATCH)]
    #[case::structure_element_target_other_type("ps[1] := w;", MISMATCH)]
    #[case::structure_element_source_other_type("w := ps[1];", MISMATCH)]
    #[case::field_target_other_type("o.inner := w;", MISMATCH)]
    #[case::dereference_target_other_extent("rp^ := c;", MISMATCH)]
    #[case::dereference_source_other_extent("c := rp^;", MISMATCH)]
    fn apply_when_element_field_or_dereference_assigned_then_types_compared(
        #[case] body: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &elements_and_fields(body), &edition3_options()),
            codes(expected)
        );
    }

    rule_err!(
        /// The example of an array of arrays in
        /// `docs/reference/compiler/problems/P2037.rst`.
        apply_when_documented_array_of_arrays_example_then_reports_only_other_extent,
        "
TYPE
    Row : ARRAY[1..3] OF DINT;
END_TYPE

PROGRAM Main
    VAR
        rows : ARRAY[1..2] OF Row;
        row : Row;
        wide : ARRAY[1..5] OF DINT;
    END_VAR

    rows[1] := row;
    row := rows[2];
    rows[1] := wide;
END_PROGRAM",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        /// An output stored in an element of an array of structures is
        /// checked as the element assigned from the output.
        apply_when_structure_output_stored_in_element_of_other_type_then_reports_mismatch,
        &block_with_structure_output("f(x := 1, p => ws[1]);").replace(
            "w : Wide;",
            "w : Wide;\n  ws : ARRAY[1..2] OF Wide;"
        ),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    /// A function block with a structure output `p`, called as `f`, and two
    /// structures: `r` and `q` of the output's type, and `w` of another.
    fn block_with_structure_output(body: &str) -> String {
        format!(
            "
TYPE
  P : STRUCT
    a : DINT;
    b : REAL;
  END_STRUCT;
  Wide : STRUCT
    a : DINT;
    b : REAL;
    c : DINT;
  END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB
VAR_INPUT
  x : DINT;
END_VAR
VAR_OUTPUT
  p : P;
END_VAR
  p.a := x;
END_FUNCTION_BLOCK

PROGRAM main
VAR
  f : FB;
  r : P;
  q : P;
  w : Wide;
END_VAR
  {body}
END_PROGRAM
"
        )
    }

    fn program_with(declarations: &str, body: &str) -> String {
        format!("PROGRAM main\nVAR\n{declarations}END_VAR\n{body}END_PROGRAM\n")
    }

    rule_ok!(
        apply_when_array_types_identical_then_accepted,
        &program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..2] OF DINT;\n",
            "a := b;\n",
        )
    );

    rule_err!(
        apply_when_array_extents_differ_then_reports_mismatch,
        &program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..5] OF DINT;\n",
            "a := b;\n",
        ),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        apply_when_array_element_types_differ_then_reports_mismatch,
        &program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..2] OF INT;\n",
            "a := b;\n",
        ),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        /// The pair the VM cannot distinguish: same element count and element
        /// type, different dimensions. Only the static check rejects it.
        apply_when_array_dimensions_differ_but_element_count_matches_then_reports_mismatch,
        &program_with(
            "a : ARRAY[1..6] OF DINT;\nb : ARRAY[1..2, 1..3] OF DINT;\n",
            "a := b;\n",
        ),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        apply_when_string_array_max_lengths_differ_then_reports_mismatch,
        &program_with(
            "a : ARRAY[1..2] OF STRING[8];\nb : ARRAY[1..2] OF STRING[16];\n",
            "a := b;\n",
        ),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_ok!(
        apply_when_struct_types_identical_then_accepted,
        "
TYPE
  Point : STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
VAR
  a : Point;
  b : Point;
END_VAR
  a := b;
END_PROGRAM
"
    );

    rule_err!(
        apply_when_struct_types_differ_then_reports_mismatch,
        "
TYPE
  Point : STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
  Wide : STRUCT
    x : DINT;
    y : DINT;
    z : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
VAR
  a : Point;
  b : Wide;
END_VAR
  a := b;
END_PROGRAM
",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        apply_when_array_assigned_to_struct_then_reports_mismatch,
        "
TYPE
  Point : STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
VAR
  a : Point;
  b : ARRAY[1..2] OF DINT;
END_VAR
  a := b;
END_PROGRAM
",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_ok!(
        /// Scalar assignment is out of this rule's scope: a narrowing store that
        /// compiles today must keep compiling.
        apply_when_scalar_widths_differ_then_no_diagnostic,
        &program_with("a : DINT;\nb : INT;\n", "a := b;\n")
    );

    rule_err!(
        /// A global is reached through a `VAR_EXTERNAL` redeclaration, which is
        /// what carries the type inside the POU. The outer scope still matters:
        /// the `VAR_GLOBAL` block itself is visited outside any POU.
        apply_when_global_array_extent_differs_then_reports_mismatch,
        "
CONFIGURATION config
  VAR_GLOBAL
    g : ARRAY[1..5] OF DINT;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR_EXTERNAL
  g : ARRAY[1..5] OF DINT;
END_VAR
VAR
  a : ARRAY[1..2] OF DINT;
END_VAR
  a := g;
END_PROGRAM
",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_ok!(
        apply_when_global_array_type_matches_then_accepted,
        "
CONFIGURATION config
  VAR_GLOBAL
    g : ARRAY[1..2] OF DINT;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR_EXTERNAL
  g : ARRAY[1..2] OF DINT;
END_VAR
VAR
  a : ARRAY[1..2] OF DINT;
END_VAR
  a := g;
END_PROGRAM
"
    );

    rule_ok!(
        /// A function block's own declaration hides an outer one of the same
        /// name, so the inner type is what gets compared. (A program may not
        /// reuse a global's name at all; that is `rule_program_var_hides_global`.)
        apply_when_local_hides_global_then_local_type_is_compared,
        "
CONFIGURATION config
  VAR_GLOBAL
    g : ARRAY[1..5] OF DINT;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK FB_Copy
VAR
  g : ARRAY[1..2] OF DINT;
  a : ARRAY[1..2] OF DINT;
END_VAR
  a := g;
END_FUNCTION_BLOCK

PROGRAM main
VAR
  fb : FB_Copy;
END_VAR
  fb();
END_PROGRAM
"
    );

    rule_err!(
        apply_when_array_assigned_inside_function_then_reports_mismatch,
        "
FUNCTION Copy : DINT
VAR
  a : ARRAY[1..2] OF DINT;
  b : ARRAY[1..5] OF DINT;
END_VAR
  a := b;
  Copy := 0;
END_FUNCTION

PROGRAM main
VAR
  r : DINT;
END_VAR
  r := Copy();
END_PROGRAM
",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_err!(
        apply_when_array_assigned_inside_function_block_then_reports_mismatch,
        "
FUNCTION_BLOCK Holder
VAR
  a : ARRAY[1..2] OF DINT;
  b : ARRAY[1..5] OF DINT;
END_VAR
  a := b;
END_FUNCTION_BLOCK

PROGRAM main
VAR
  h : Holder;
END_VAR
  h();
END_PROGRAM
",
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_ok!(
        /// A function block's locals must not leak into a later POU's scope.
        apply_when_pou_ends_then_its_declarations_leave_scope,
        "
FUNCTION_BLOCK Holder
VAR
  a : ARRAY[1..2] OF DINT;
END_VAR
  a[1] := 1;
END_FUNCTION_BLOCK

PROGRAM main
VAR
  a : ARRAY[1..5] OF DINT;
  b : ARRAY[1..5] OF DINT;
END_VAR
  a := b;
END_PROGRAM
"
    );

    rule_ok!(
        /// A function result is P4027's business; reporting here too would give
        /// two diagnostics for one mistake.
        apply_when_function_result_assigned_then_defers_to_return_type_rule,
        "
TYPE
  Point : STRUCT
    x : DINT;
  END_STRUCT;
  Other : STRUCT
    x : DINT;
    y : DINT;
  END_STRUCT;
END_TYPE

FUNCTION MakePoint : Point
  MakePoint.x := 1;
END_FUNCTION

PROGRAM main
VAR
  a : Other;
END_VAR
  a := MakePoint();
END_PROGRAM
"
    );

    rule_ok!(
        apply_when_matching_struct_returning_function_then_accepted,
        "
TYPE
  Point : STRUCT
    x : DINT;
  END_STRUCT;
END_TYPE

FUNCTION MakePoint : Point
  MakePoint.x := 1;
END_FUNCTION

PROGRAM main
VAR
  a : Point;
END_VAR
  a := MakePoint();
END_PROGRAM
"
    );

    rule_err_at!(
        /// Nothing but a same-typed aggregate may be assigned to an aggregate.
        /// Codegen depends on this: it treats an unresolvable source at
        /// COPY_REGION emission as a compiler defect.
        apply_when_constant_assigned_to_array_then_reports_mismatch,
        &program_with("a : ARRAY[1..2] OF DINT;\n", "a := 5;\n"),
        Problem::AggregateAssignmentTypeMismatch,
        "5");

    rule_ok!(
        /// An element of an elementary type is not an aggregate.
        apply_when_array_element_assigned_then_no_diagnostic,
        &program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..5] OF DINT;\n",
            "a[1] := b[1];\n",
        )
    );

    rule_err!(
        /// A method's local belongs to the method. Before the traversal
        /// opened a scope for a method, every method's declarations landed in
        /// the enclosing function block's frame, so a method local overwrote
        /// a field of the same name for every method compiled after it --
        /// and the mismatch it hid was accepted.
        apply_when_method_local_shadows_field_then_sibling_method_uses_field_type,
        "
TYPE
    Pt : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
    Other : STRUCT
        a : INT;
    END_STRUCT;
END_TYPE
FUNCTION_BLOCK FB_Motor
VAR
    v : Pt;
    src : Other;
END_VAR
METHOD A
VAR
    v : Other;
END_VAR
    v := src;
END_METHOD
METHOD B
    v := src;
END_METHOD
END_FUNCTION_BLOCK
",
        [Problem::AggregateAssignmentTypeMismatch],
        fb_inheritance_options()
    );

    rule_ok!(
        /// A field of a function block instance has the type the block
        /// declares it with, a structure included.
        #[spec_test(REQ_VB_analyzer_051)]
        apply_when_structure_field_of_block_read_into_same_type_then_accepted,
        &block_with_structure_output("f(x := 1);\n  r := f.p;")
    );

    rule_err_at!(
        apply_when_structure_field_of_block_read_into_other_type_then_reports_mismatch,
        &block_with_structure_output("f(x := 1);\n  w := f.p;"),
        Problem::AggregateAssignmentTypeMismatch,
        "f.p"
    );

    rule_ok!(
        #[spec_test(REQ_CB_analyzer_038)]
        apply_when_structure_output_stored_in_same_type_then_accepted,
        &block_with_structure_output("f(x := 1, p => q);")
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_038)]
        apply_when_structure_output_stored_in_other_type_then_reports_mismatch,
        &block_with_structure_output("f(x := 1, p => w);"),
        [Problem::AggregateAssignmentTypeMismatch]
    );

    rule_ok!(
        /// A negated output is not an aggregate; `NOT p` of a structure is
        /// P4083, which another rule reports.
        apply_when_structure_output_negated_then_left_to_negated_output_rule,
        &block_with_structure_output("f(x := 1, NOT p => q);")
    );
}
