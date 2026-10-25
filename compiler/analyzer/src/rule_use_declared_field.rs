//! Semantic rule that a field selection `v.f` names a field of `v`'s type:
//! a field of a structure, or a field a function block declares or inherits
//! through `EXTENDS`.
//!
//! A name that is a property of the block, or of a block it inherits from,
//! is not reported: property access is not implemented, and code generation
//! says so. A prefix whose type is not known is not reported either; the
//! rule that cannot type it reports why.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    Item : STRUCT a : DINT; b : DINT; END_STRUCT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       one : Item;
//!       timer : TON;
//!       result : DINT;
//!    END_VAR
//!
//!    result := one.a;
//!    timer(IN := result > 0, PT := T#1s);
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    Item : STRUCT a : DINT; b : DINT; END_STRUCT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       one : Item;
//!       result : DINT;
//!    END_VAR
//!
//!    result := one.missing;
//! END_PROGRAM
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::StructuredVariable,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    string_similarity::find_closest_match,
    symbol_environment::ScopeTracker,
    variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        FieldSelectionChecker {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Checks each field selection against the type of the variable it selects
/// from, which the type environment answers for.
struct FieldSelectionChecker<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to type a selection's prefix from.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl FieldSelectionChecker<'_> {
    /// Reports `node` when the type of its prefix is a structure or a
    /// function block with no field and no property of the name it selects.
    fn check(&mut self, node: &StructuredVariable) {
        let types = self.context.types();
        let Some(record) = variable_type::type_id(
            &node.record,
            self.context.symbols(),
            types,
            &self.scope.current(),
        ) else {
            return;
        };
        if types.field_type(record, &node.field).is_some() {
            return;
        }
        // `field_type` has no answer for a field whose declared type did not
        // resolve either, which the declaration's own rule reports; only a
        // name that is no field at all is this rule's to report.
        let Some(fields) = types.field_names(record) else {
            return;
        };
        if fields.contains(&&node.field) || types.has_property(record, &node.field) {
            return;
        }

        let suggestion = find_closest_match(
            node.field.original(),
            fields.iter().map(|field| field.original().as_str()),
        );
        let mut diagnostic = Diagnostic::problem(
            Problem::FieldNotFound,
            Label::span(node.field.span(), "Undefined field"),
        )
        .with_context_id("field", &node.field)
        .with_context("variable", &node.record.to_string());
        if let Some(suggestion) = suggestion {
            diagnostic = diagnostic.with_context("did you mean", &suggestion);
        }
        self.diagnostics.push(diagnostic);
    }
}

impl DiagnosticVisitor for FieldSelectionChecker<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for FieldSelectionChecker<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_structured_variable(&mut self, node: &StructuredVariable) -> Result<(), Infallible> {
        // The prefix first, so that two selections in one reference are
        // reported in the order they are written.
        node.recurse_visit(self)?;
        self.check(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        diagnostic_codes, edition3_options, fb_inheritance_options, rule_diagnostics,
    };

    /// The types the tests select from: a structure `Item`, a structure
    /// `Outer` holding an `Item`, a block `DOUBLER` with an input and an
    /// output, and a block `Holder` whose output is an `Item`.
    const TYPES: &str = "
TYPE
  Item : STRUCT a : DINT; b : DINT; END_STRUCT;
  Outer : STRUCT inner : Item; END_STRUCT;
END_TYPE

FUNCTION_BLOCK DOUBLER
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Holder
  VAR_OUTPUT o : Item; END_VAR
END_FUNCTION_BLOCK
";

    /// [`TYPES`] and a program `main` with a variable of each kind the tests
    /// select from, whose body is `body`.
    fn program(body: &str) -> String {
        format!(
            "{TYPES}
PROGRAM main
  VAR
    fb : DOUBLER;
    h : Holder;
    one : Item;
    arr : ARRAY[1..3] OF Item;
    nest : Outer;
    p : REF_TO Item;
    timer : TON;
    result : DINT;
    flag : BOOL;
  END_VAR
  {body}
END_PROGRAM"
        )
    }

    // ---------------------------------------------------------------------
    // A selection that names no field, for each kind of prefix.
    // ---------------------------------------------------------------------

    rule_err_at!(
        apply_when_function_block_field_written_does_not_exist_then_error,
        &program("fb.bogus := 1;"),
        Problem::FieldNotFound,
        "bogus",
        edition3_options()
    );

    rule_err_at!(
        apply_when_function_block_field_read_does_not_exist_then_error,
        &program("result := fb.bogus;"),
        Problem::FieldNotFound,
        "bogus",
        edition3_options()
    );

    rule_err_at!(
        apply_when_structure_field_does_not_exist_then_error,
        &program("result := one.missing;"),
        Problem::FieldNotFound,
        "missing",
        edition3_options()
    );

    rule_err_at!(
        apply_when_array_element_field_does_not_exist_then_error,
        &program("result := arr[1].missing;"),
        Problem::FieldNotFound,
        "missing",
        edition3_options()
    );

    rule_err_at!(
        apply_when_dereferenced_field_does_not_exist_then_error,
        &program("result := p^.missing;"),
        Problem::FieldNotFound,
        "missing",
        edition3_options()
    );

    rule_err_at!(
        apply_when_nested_field_does_not_exist_then_error,
        &program("result := nest.inner.missing;"),
        Problem::FieldNotFound,
        "missing",
        edition3_options()
    );

    rule_err_at!(
        apply_when_function_block_output_field_does_not_exist_then_error,
        &program("result := h.o.missing;"),
        Problem::FieldNotFound,
        "missing",
        edition3_options()
    );

    rule_err_at!(
        apply_when_standard_block_field_does_not_exist_then_error,
        &program("flag := timer.BOGUS;"),
        Problem::FieldNotFound,
        "BOGUS",
        edition3_options()
    );

    rule_err_at!(
        apply_when_in_out_field_does_not_exist_then_error,
        "
TYPE
  Item : STRUCT a : DINT; END_STRUCT;
END_TYPE

FUNCTION_BLOCK Writer
  VAR_IN_OUT io : Item; END_VAR
  io.missing := 1;
END_FUNCTION_BLOCK",
        Problem::FieldNotFound,
        "missing"
    );

    rule_err_at!(
        /// The selection that names no field is reported; the one after it,
        /// whose prefix has no type, is not reported again.
        apply_when_middle_of_selection_does_not_exist_then_one_error,
        &program("result := nest.nope.a;"),
        Problem::FieldNotFound,
        "nope",
        edition3_options()
    );

    // ---------------------------------------------------------------------
    // A selection that names a field or a property.
    // ---------------------------------------------------------------------

    rule_ok!(
        apply_when_each_selection_names_a_field_then_ok,
        &program(
            "fb.x := 1;
  result := fb.y;
  result := one.a;
  result := arr[1].b;
  result := p^.a;
  result := nest.inner.a;
  result := h.o.b;
  flag := timer.Q;
  flag := timer.q;"
        ),
        edition3_options()
    );

    rule_ok!(
        apply_when_in_out_field_exists_then_ok,
        "
TYPE
  Item : STRUCT a : DINT; END_STRUCT;
END_TYPE

FUNCTION_BLOCK Writer
  VAR_IN_OUT io : Item; END_VAR
  io.a := 1;
END_FUNCTION_BLOCK"
    );

    /// A block whose base and base's base each declare a field and a
    /// property.
    const INHERITANCE: &str = "
FUNCTION_BLOCK FB_A
  VAR_INPUT count : INT; END_VAR
PROPERTY Speed : REAL
GET
  Speed := 1.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
  VAR_OUTPUT done : BOOL; END_VAR
PROPERTY Mode : INT
GET
  Mode := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C EXTENDS FB_B
END_FUNCTION_BLOCK
";

    rule_ok!(
        apply_when_field_inherited_through_extends_then_ok,
        &format!(
            "{INHERITANCE}
PROGRAM main
  VAR c : FB_C; n : INT; b : BOOL; END_VAR
  n := c.count;
  b := c.done;
END_PROGRAM"
        ),
        fb_inheritance_options()
    );

    rule_ok!(
        /// Property access is not implemented; code generation reports it.
        apply_when_property_own_or_inherited_then_ok,
        &format!(
            "{INHERITANCE}
PROGRAM main
  VAR b : FB_B; c : FB_C; r : REAL; n : INT; END_VAR
  n := b.Mode;
  r := b.Speed;
  r := c.Speed;
END_PROGRAM"
        ),
        fb_inheritance_options()
    );

    // ---------------------------------------------------------------------
    // A selection this rule leaves to others.
    // ---------------------------------------------------------------------

    rule_ok!(
        /// `rule_use_declared_symbolic_var` reports the undeclared prefix.
        apply_when_prefix_undeclared_then_ok,
        &program("result := undeclared.a;"),
        edition3_options()
    );

    rule_ok!(
        /// The field `u` exists; the rule that resolves declarations reports
        /// that its type does not.
        apply_when_field_type_did_not_resolve_then_ok,
        "
FUNCTION_BLOCK Broken
  VAR_INPUT u : Undeclared; END_VAR
END_FUNCTION_BLOCK

PROGRAM main
  VAR b : Broken; END_VAR
  b.u := 1;
END_PROGRAM"
    );

    rule_ok!(
        /// An interface is not supported, which `rule_unsupported_extension`
        /// reports; its property is not reported as a missing field.
        apply_when_interface_property_then_ok,
        "
INTERFACE I_Axis
PROPERTY Position : REAL
GET
END_GET
END_PROPERTY
END_INTERFACE

PROGRAM main
  VAR a : I_Axis; r : REAL; END_VAR
  r := a.Position;
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_ok!(
        /// `THIS^` has no type yet (issue #1406).
        apply_when_prefix_is_this_then_ok,
        "
FUNCTION_BLOCK FB_Motor
  VAR speed : INT; END_VAR
METHOD Stop
  THIS^.missing := 0;
END_METHOD
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_ok!(
        /// A method call is `rule_method_call_declared`'s (P4046).
        apply_when_method_call_then_ok,
        "
FUNCTION_BLOCK FB_Motor
END_FUNCTION_BLOCK

PROGRAM main
  VAR m : FB_Motor; END_VAR
  m.Missing();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_ok!(
        /// A selection through `REFERENCE TO` is not implemented whatever
        /// field it names.
        apply_when_prefix_is_reference_to_then_ok,
        "
TYPE
  Item : STRUCT a : DINT; END_STRUCT;
END_TYPE

PROGRAM main
  VAR r : REFERENCE TO Item; x : DINT; END_VAR
  x := r.missing;
END_PROGRAM",
        CompilerOptions {
            allow_reference_to: true,
            ..CompilerOptions::default()
        }
    );

    // ---------------------------------------------------------------------
    // The message.
    // ---------------------------------------------------------------------

    #[test]
    fn apply_when_field_misspelled_then_names_field_variable_and_suggestion() {
        let diagnostics =
            rule_diagnostics(apply, &program("result := h.o.bb;"), &edition3_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::FieldNotFound.code()]
        );
        assert!(diagnostics[0].described.contains(&"field=bb".to_owned()));
        assert!(diagnostics[0]
            .described
            .contains(&"variable=h.o".to_owned()));
        assert!(diagnostics[0]
            .described
            .contains(&"did you mean=b".to_owned()));
    }

    #[test]
    fn apply_when_inherited_field_misspelled_then_suggests_it() {
        let program = format!(
            "{INHERITANCE}
PROGRAM main
  VAR c : FB_C; n : INT; END_VAR
  n := c.cont;
END_PROGRAM"
        );

        let diagnostics = rule_diagnostics(apply, &program, &fb_inheritance_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::FieldNotFound.code()]
        );
        assert!(diagnostics[0]
            .described
            .contains(&"did you mean=count".to_owned()));
    }

    #[test]
    fn apply_when_no_field_close_then_no_suggestion() {
        let diagnostics = rule_diagnostics(
            apply,
            &program("result := arr[1].missing;"),
            &edition3_options(),
        );

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::FieldNotFound.code()]
        );
        assert!(diagnostics[0]
            .described
            .contains(&"variable=arr[1]".to_owned()));
        assert!(!diagnostics[0]
            .described
            .iter()
            .any(|described| described.starts_with("did you mean")));
    }

    #[test]
    fn apply_when_two_selections_name_no_field_then_reports_both_in_order() {
        let diagnostics = rule_diagnostics(
            apply,
            &program("result := arr[one.bad].missing;"),
            &edition3_options(),
        );

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::FieldNotFound.code(); 2]
        );
        let fields: Vec<&str> = diagnostics
            .iter()
            .flat_map(|diagnostic| &diagnostic.described)
            .map(String::as_str)
            .filter(|described| described.starts_with("field="))
            .collect();
        assert_eq!(fields, ["field=bad", "field=missing"]);
    }

    // The examples of `docs/reference/compiler/problems/P4084.rst`.
    const DOCUMENTED_PROGRAM: &str = "
TYPE
  Motor : STRUCT
    speed : INT;
    running : BOOL;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    pump : Motor;
  END_VAR
  pump.sped := 100;
END_PROGRAM";

    rule_err_at!(
        apply_when_documented_failing_example_then_error,
        DOCUMENTED_PROGRAM,
        Problem::FieldNotFound,
        "sped"
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        &DOCUMENTED_PROGRAM.replace("pump.sped", "pump.speed")
    );
}
