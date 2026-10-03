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
//! Fires only when the assignment *target* is a whole array or structure
//! variable. Two neighbouring cases belong elsewhere:
//!
//! * Scalar assignment is deliberately untouched — checking it is the right
//!   end state but interacts with implicit widening (ADR-0029, ADR-0031) and
//!   would reject programs that compile today.
//! * A function result is left to `rule_function_call_type_check` (P4027),
//!   which already compares a call's return type against its assignment
//!   destination. Reporting here as well would produce two diagnostics for
//!   one mistake.
//!
//! Everything else reaching an aggregate target is rejected, including a
//! source whose type does not resolve at all. Codegen relies on that: having
//! reached `COPY_REGION` emission with an aggregate destination, an
//! unresolvable source is a compiler defect rather than a bad program.
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

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::IntermediateType,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
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

impl RuleAggregateAssignment<'_> {
    /// Resolves a declared variable to its [`IntermediateType`]: a named
    /// type (`p : Point`) or one spelled out in place
    /// (`a : ARRAY[1..2] OF DINT`), whose representation states its shape.
    fn declared_type(&self, id: &Id) -> Option<IntermediateType> {
        variable_type::declared(id, self.context, &self.scope.current()).cloned()
    }

    /// P2037: whole-array and whole-structure assignment requires identical
    /// declared types.
    fn check_aggregate_assignment(&mut self, target: &Variable, value: &Expr) {
        let Variable::Symbolic(SymbolicVariableKind::Named(named)) = target else {
            // An element or field write is not a whole-aggregate assignment.
            return;
        };
        let Some(target_type) = self.declared_type(&named.name) else {
            return;
        };
        if !matches!(
            target_type,
            IntermediateType::Array { .. } | IntermediateType::Structure { .. }
        ) {
            return;
        }
        // P4027 owns a call's return type against its destination.
        if matches!(value.kind, ExprKind::Function(_)) {
            return;
        }

        let value_type = match &value.kind {
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(source))) => {
                self.declared_type(&source.name)
            }
            _ => None,
        };
        // An unresolvable source is a mismatch too: nothing other than a
        // same-typed aggregate may be assigned to an aggregate.
        if value_type.as_ref() != Some(&target_type) {
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
        self.check_aggregate_assignment(&node.target, &node.value);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::fb_inheritance_options;
    use ironplc_problems::Problem;

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
        /// An element write is not a whole-aggregate assignment.
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
}
