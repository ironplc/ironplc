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
    intermediates,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
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
        RuleAggregateAssignment {
            type_environment: context.types(),
            // `Declarations::new` opens the base scope. That is where
            // declarations made outside any POU land -- a CONFIGURATION's
            // VAR_GLOBAL block, most importantly -- so a POU scope's lookups
            // fall through to them. Opening another here would leave the
            // stack unbalanced when the table drops.
            declarations: Declarations::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleAggregateAssignment<'a> {
    type_environment: &'a TypeEnvironment,
    /// Declared type of every variable in scope.
    declarations: Declarations<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleAggregateAssignment<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleAggregateAssignment<'_> {
    /// Resolves a declared variable to its [`IntermediateType`].
    ///
    /// Handles both spellings a variable's type can take: a named type
    /// (`p : Point`) resolved through the type environment, and an inline
    /// specification (`a : ARRAY[1..2] OF DINT`) built from the declaration.
    fn declared_type(&mut self, id: &Id) -> Option<IntermediateType> {
        let type_environment = self.type_environment;
        let declared = self.declarations.find(id)?;
        match &declared.0 {
            InitialValueAssignmentKind::Array(array) => {
                // An inline array specification. The name passed here only
                // feeds diagnostics inside the helper, which are discarded:
                // a malformed declaration is already reported by the
                // declaration rules, and this rule stays silent on it.
                let name = TypeName::from_id(id);
                match intermediates::array::try_from(&name, &array.spec, type_environment) {
                    Ok(intermediates::array::IntermediateResult::Type(attrs)) => {
                        Some(attrs.representation)
                    }
                    Ok(intermediates::array::IntermediateResult::Alias(alias)) => type_environment
                        .get(&alias)
                        .map(|attrs| attrs.representation.clone()),
                    Err(_) => None,
                }
            }
            InitialValueAssignmentKind::Simple(simple) => type_environment
                .get(&simple.type_name)
                .map(|attrs| attrs.representation.clone()),
            InitialValueAssignmentKind::Structure(structure) => type_environment
                .get(&structure.type_name)
                .map(|attrs| attrs.representation.clone()),
            InitialValueAssignmentKind::LateResolvedType(type_name) => type_environment
                .get(type_name)
                .map(|attrs| attrs.representation.clone()),
            _ => None,
        }
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

    /// Opens a declaration's scope.
    ///
    /// Every kind contributes the same thing -- a frame its own
    /// declarations go into -- but the match stays exhaustive so that a
    /// new kind of scope has to say so rather than silently sharing the
    /// enclosing declaration's frame.
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

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        self.declarations.add_if(
            node.identifier.symbolic_id(),
            Declared(node.initializer.clone()),
        );
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        self.check_aggregate_assignment(&node.target, &node.value);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use ironplc_problems::Problem;

    /// Analyzes `program`, returning the problem codes it reported.
    fn problem_codes(program: &str) -> Vec<String> {
        let options = CompilerOptions::default();
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        match analyze(&[&library], &options) {
            Ok((_, context)) => context
                .diagnostics()
                .iter()
                .map(|d| d.code.clone())
                .collect(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code.clone()).collect(),
        }
    }

    fn program_with(declarations: &str, body: &str) -> String {
        format!("PROGRAM main\nVAR\n{declarations}END_VAR\n{body}END_PROGRAM\n")
    }

    #[test]
    fn apply_when_array_types_identical_then_accepted() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..2] OF DINT;\n",
            "a := b;\n",
        ));
        assert!(codes.is_empty(), "expected no diagnostics, got {codes:?}");
    }

    #[test]
    fn apply_when_array_extents_differ_then_reports_mismatch() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..5] OF DINT;\n",
            "a := b;\n",
        ));
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_array_element_types_differ_then_reports_mismatch() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..2] OF INT;\n",
            "a := b;\n",
        ));
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    /// The pair the VM cannot distinguish: same element count and element
    /// type, different dimensions. Only the static check rejects it.
    #[test]
    fn apply_when_array_dimensions_differ_but_element_count_matches_then_reports_mismatch() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..6] OF DINT;\nb : ARRAY[1..2, 1..3] OF DINT;\n",
            "a := b;\n",
        ));
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_string_array_max_lengths_differ_then_reports_mismatch() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..2] OF STRING[8];\nb : ARRAY[1..2] OF STRING[16];\n",
            "a := b;\n",
        ));
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_struct_types_identical_then_accepted() {
        let codes = problem_codes(
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
",
        );
        assert!(codes.is_empty(), "expected no diagnostics, got {codes:?}");
    }

    #[test]
    fn apply_when_struct_types_differ_then_reports_mismatch() {
        let codes = problem_codes(
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
        );
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_array_assigned_to_struct_then_reports_mismatch() {
        let codes = problem_codes(
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
        );
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    /// Scalar assignment is out of this rule's scope: a narrowing store that
    /// compiles today must keep compiling.
    #[test]
    fn apply_when_scalar_widths_differ_then_no_diagnostic() {
        let codes = problem_codes(&program_with("a : DINT;\nb : INT;\n", "a := b;\n"));
        assert!(
            !codes.contains(&"P2037".to_string()),
            "scalars are out of scope, got {codes:?}"
        );
    }

    /// A global is reached through a `VAR_EXTERNAL` redeclaration, which is
    /// what carries the type inside the POU. The outer scope still matters:
    /// the `VAR_GLOBAL` block itself is visited outside any POU.
    #[test]
    fn apply_when_global_array_extent_differs_then_reports_mismatch() {
        let codes = problem_codes(
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
        );
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_global_array_type_matches_then_accepted() {
        let codes = problem_codes(
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
",
        );
        assert!(codes.is_empty(), "expected no diagnostics, got {codes:?}");
    }

    /// A POU's own declaration shadows an outer one of the same name, so the
    /// inner type is what gets compared.
    #[test]
    fn apply_when_local_shadows_global_then_local_type_is_compared() {
        let codes = problem_codes(
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
VAR
  g : ARRAY[1..2] OF DINT;
  a : ARRAY[1..2] OF DINT;
END_VAR
  a := g;
END_PROGRAM
",
        );
        assert!(codes.is_empty(), "expected no diagnostics, got {codes:?}");
    }

    #[test]
    fn apply_when_array_assigned_inside_function_then_reports_mismatch() {
        let codes = problem_codes(
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
        );
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_array_assigned_inside_function_block_then_reports_mismatch() {
        let codes = problem_codes(
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
        );
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    /// A function block's locals must not leak into a later POU's scope.
    #[test]
    fn apply_when_pou_ends_then_its_declarations_leave_scope() {
        let codes = problem_codes(
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
",
        );
        assert!(
            codes.is_empty(),
            "main's own `a` is ARRAY[1..5], not the FB's ARRAY[1..2]; got {codes:?}"
        );
    }

    /// A function result is P4027's business; reporting here too would give
    /// two diagnostics for one mistake.
    #[test]
    fn apply_when_function_result_assigned_then_defers_to_return_type_rule() {
        let codes = problem_codes(
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
",
        );
        assert!(
            !codes.contains(&"P2037".to_string()),
            "P4027 owns this case; got {codes:?}"
        );
        assert!(codes.contains(&"P4027".to_string()), "got {codes:?}");
    }

    #[test]
    fn apply_when_matching_struct_returning_function_then_accepted() {
        let codes = problem_codes(
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
",
        );
        assert!(codes.is_empty(), "expected no diagnostics, got {codes:?}");
    }

    /// Nothing but a same-typed aggregate may be assigned to an aggregate.
    /// Codegen depends on this: it treats an unresolvable source at
    /// COPY_REGION emission as a compiler defect.
    #[test]
    fn apply_when_constant_assigned_to_array_then_reports_mismatch() {
        let codes = problem_codes(&program_with("a : ARRAY[1..2] OF DINT;\n", "a := 5;\n"));
        assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
    }

    /// An element write is not a whole-aggregate assignment.
    #[test]
    fn apply_when_array_element_assigned_then_no_diagnostic() {
        let codes = problem_codes(&program_with(
            "a : ARRAY[1..2] OF DINT;\nb : ARRAY[1..5] OF DINT;\n",
            "a[1] := b[1];\n",
        ));
        assert!(
            !codes.contains(&"P2037".to_string()),
            "element writes are out of scope, got {codes:?}"
        );
    }

    /// Analyzes `program` with methods enabled, returning the problem
    /// codes it reported.
    fn problem_codes_with_methods(program: &str) -> Vec<String> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        match analyze(&[&library], &options) {
            Ok((_, context)) => context
                .diagnostics()
                .iter()
                .map(|d| d.code.clone())
                .collect(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code.clone()).collect(),
        }
    }

    /// A method's local belongs to the method. Before the traversal
    /// opened a scope for a method, every method's declarations landed in
    /// the enclosing function block's frame, so a method local overwrote
    /// a field of the same name for every method compiled after it --
    /// and the mismatch it hid was accepted.
    #[test]
    fn apply_when_method_local_shadows_field_then_sibling_method_uses_field_type() {
        let codes = problem_codes_with_methods(
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
        );

        assert!(
            codes
                .iter()
                .any(|c| c == Problem::AggregateAssignmentTypeMismatch.code()),
            "expected an aggregate assignment mismatch on the function block's field, got {codes:?}"
        );
    }
}
