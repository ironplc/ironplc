//! Requires both sides of a whole-aggregate assignment to have identical
//! declared types.
//!
//! IEC 61131-3 §7.3.3.1 makes assignment over a multi-element variable a value
//! copy. Codegen implements that with `COPY_REGION`, whose length the VM
//! derives from the two array descriptors — so the two ends must describe the
//! same shape. The VM cross-checks the derived byte sizes and traps
//! (`RegionSizeMismatch`, V9018), but that is a backstop against a compiler
//! defect: declared-type equality is a static property and belongs here.
//!
//! Descriptors cannot tell `ARRAY[1..6] OF INT` from `ARRAY[1..2,1..3] OF INT`
//! (same element count, same element type), so the runtime check would accept
//! that pair. This rule is what rejects it.
//!
//! ## Scope
//!
//! Only fires when the assignment *target* is a whole array or structure
//! variable. Scalar assignment is deliberately untouched — checking it is the
//! right end state but interacts with implicit widening (ADR-0029, ADR-0031)
//! and would reject programs that compile today.
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
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::collections::HashMap;

use crate::{
    intermediate_type::IntermediateType,
    intermediates,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleAggregateAssignment {
            type_environment: context.types(),
            var_decls: HashMap::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleAggregateAssignment<'a> {
    type_environment: &'a TypeEnvironment,
    /// Declared initializer for each variable in the POU currently being
    /// visited, which is what carries the declared type.
    var_decls: HashMap<Id, InitialValueAssignmentKind>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleAggregateAssignment<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleAggregateAssignment<'_> {
    fn collect_variables(&mut self, variables: &[VarDecl]) {
        for var in variables {
            if let VariableIdentifier::Symbol(id) = &var.identifier {
                self.var_decls.insert(id.clone(), var.initializer.clone());
            }
        }
    }

    fn clear_variables(&mut self) {
        self.var_decls.clear();
    }

    /// Resolves a declared variable to its [`IntermediateType`].
    ///
    /// Handles both spellings a variable's type can take: a named type
    /// (`p : Point`) resolved through the type environment, and an inline
    /// specification (`a : ARRAY[1..2] OF DINT`) built from the declaration.
    fn declared_type(&self, id: &Id) -> Option<IntermediateType> {
        match self.var_decls.get(id)? {
            InitialValueAssignmentKind::Array(array) => {
                // An inline array specification. `node_name` only feeds
                // diagnostics inside the helper, which are discarded here:
                // a malformed declaration is already reported by the
                // declaration rules, and this rule stays silent on it.
                let name = TypeName::from_id(id);
                match intermediates::array::try_from(&name, &array.spec, self.type_environment) {
                    Ok(intermediates::array::IntermediateResult::Type(attrs)) => {
                        Some(attrs.representation)
                    }
                    Ok(intermediates::array::IntermediateResult::Alias(alias)) => self
                        .type_environment
                        .get(&alias)
                        .map(|attrs| attrs.representation.clone()),
                    Err(_) => None,
                }
            }
            InitialValueAssignmentKind::Simple(simple) => self
                .type_environment
                .get(&simple.type_name)
                .map(|attrs| attrs.representation.clone()),
            InitialValueAssignmentKind::Structure(structure) => self
                .type_environment
                .get(&structure.type_name)
                .map(|attrs| attrs.representation.clone()),
            InitialValueAssignmentKind::LateResolvedType(type_name) => self
                .type_environment
                .get(type_name)
                .map(|attrs| attrs.representation.clone()),
            _ => None,
        }
    }

    /// Resolves an expression to the declared type it yields, for the two
    /// shapes that can produce a whole aggregate: another variable, and a
    /// function result.
    fn expression_type(&self, expr: &Expr) -> Option<IntermediateType> {
        match &expr.kind {
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
                self.declared_type(&named.name)
            }
            ExprKind::Function(function) => self
                .type_environment
                .get(&TypeName::from_id(&function.name))
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
        // A source whose type cannot be resolved gets no opinion — an array
        // literal or an expression is rejected by codegen as unimplemented,
        // which is a clearer message than a type mismatch.
        let Some(value_type) = self.expression_type(value) else {
            return;
        };
        if target_type != value_type {
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

impl Visitor<Diagnostic> for RuleAggregateAssignment<'_> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.clear_variables();
        self.collect_variables(&node.variables);
        let ret = node.recurse_visit(self);
        self.clear_variables();
        ret
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Diagnostic> {
        self.check_aggregate_assignment(&node.target, &node.value);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

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
}
