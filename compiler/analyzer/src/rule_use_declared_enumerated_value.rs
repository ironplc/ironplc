//! Semantic rule that an enumerated value is a value of the enumeration it
//! is used as.
//!
//! The value must be one of the members the enumeration declares. This
//! holds for an inline enumeration too: `e : (A, B) := C` names a value
//! its own list does not declare.
//!
//! A value written with its type (`COLOR#RED`) names that type as well. The
//! type must be an enumeration (`P2004`), and it must be the enumeration the
//! value is used as or an alias of it (`P2045`), before its value is
//! checked. `c : COLOR := SIZE#RED` names `SIZE`, so it is reported whether
//! or not `SIZE` declares `RED`. Two enumerations that list the same members
//! are two enumerations, and an inline enumeration has no name for a value
//! to be written with.
//!
//! The values checked are the initial value of a variable or a structure
//! field, the default a `TYPE` declaration gives an enumeration, and the
//! labels of a `CASE`, which are values of the selector's type. A label
//! written without its type is checked only when the selector is an
//! enumeration; one written with its type is checked against any selector
//! whose type is known, so `CASE count OF COLOR#RED:` is reported when
//! `count` is an `INT`. A selector whose type the analyzer did not resolve
//! is skipped.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    LEVEL : (CRITICAL) := CRITICAL;
//!    SEVERITY : LEVEL := LEVEL#CRITICAL;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;
//!       OTHER : LEVEL := SEVERITY#CRITICAL;
//!    END_VAR
//!    CASE LEVEL OF
//!       LEVEL#CRITICAL: ;
//!    END_CASE;
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    LEVEL : (INFO) := INFO;
//!    MODE : (INFO, DEBUG);
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;     (* P2006: not a value of LEVEL *)
//!       OTHER : LEVEL := MODE#INFO;    (* P2045: MODE is not LEVEL *)
//!    END_VAR
//!    CASE LEVEL OF
//!       MODE#DEBUG: ;                  (* P2045: MODE is not LEVEL *)
//!    END_CASE;
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{Case, CaseSelectionKind, ExprType},
    type_id::TypeId,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    enumeration_members::EnumerationMembers,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    value_type,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    // Walk the library to find all references to enumerations
    // checking that all references use an enumeration value
    // that is part of the enumeration
    run_rule(RuleDeclaredEnumeratedValues::new(context), lib)
}

/// The enumeration an enumerated value is used as.
#[derive(Clone, Copy)]
enum UsedAs {
    /// The type `id` identifies: the type a declaration names, or the type
    /// of a `CASE` selector.
    Type(TypeId),
    /// An enumeration spelled out in place (`e : (A, B)`), which has no name.
    Inline,
}

struct RuleDeclaredEnumeratedValues<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> RuleDeclaredEnumeratedValues<'a> {
    fn new(context: &'a SemanticContext) -> Self {
        RuleDeclaredEnumeratedValues {
            context,
            diagnostics: Vec::new(),
        }
    }

    /// The id of the enumeration `type_name` names and its members, as the
    /// type environment records them. An alias (`B : A`) has the members of
    /// the enumeration it names.
    ///
    /// # Errors
    ///
    /// `EnumNotDeclared` when `type_name` names no type, or a type that is
    /// not an enumeration.
    fn enumeration(
        &self,
        type_name: &TypeName,
    ) -> Result<(TypeId, &'a EnumerationMembers), Diagnostic> {
        let types = self.context.types();
        types
            .id_of(type_name)
            .and_then(|id| {
                let members = types.get_by_id(id)?.representation.enumeration_members()?;
                Some((id, members))
            })
            .ok_or_else(|| {
                Diagnostic::problem(
                    Problem::EnumNotDeclared,
                    Label::span(type_name.span(), "Type is not an enumeration"),
                )
            })
    }

    /// Checks `value`, used as `used_as`, whose value `is_member` says is
    /// one of the members of that enumeration: its written type first, and
    /// its value only when the written type is the right one.
    fn check_value(&mut self, value: &EnumeratedValue, used_as: UsedAs, is_member: bool) {
        if self.written_type_matches(value, used_as) && !is_member {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::EnumValueNotDefined,
                    Label::span(value.span(), "Expected value in enumeration"),
                )
                .with_context_id("value", &value.value),
            );
        }
    }

    /// Whether the type `value` is written with is the enumeration
    /// `used_as` or an alias of it; true for a value written without one.
    /// Reports a written type that is not an enumeration (`P2004`) or that
    /// is another type (`P2045`).
    ///
    /// The enumeration and its aliases are one enumeration, so the alias
    /// chain of each is followed to the type it ends at.
    fn written_type_matches(&mut self, value: &EnumeratedValue, used_as: UsedAs) -> bool {
        let Some(written) = &value.type_name else {
            return true;
        };
        let written_id = match self.enumeration(written) {
            Ok((id, _)) => id,
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                return false;
            }
        };
        let types = self.context.types();
        let expected = match used_as {
            UsedAs::Type(expected)
                if types.alias_root(written_id) == types.alias_root(expected) =>
            {
                return true;
            }
            UsedAs::Type(expected) => value_type::describe(types, expected),
            UsedAs::Inline => "an inline enumeration".to_string(),
        };
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::EnumValueTypeMismatch,
                Label::span(written.span(), "Type of the enumerated value"),
            )
            .with_context_type("type", written)
            .with_context("expected", &expected),
        );
        false
    }
}

impl DiagnosticVisitor for RuleDeclaredEnumeratedValues<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleDeclaredEnumeratedValues<'_> {
    type Value = ();

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Infallible> {
        // The default a declaration gives is a value of the type it declares.
        // A declaration that lists its values is checked against its own
        // list, which is the type's unless the name is declared twice.
        if let (Some(default), Ok((id, members))) =
            (&node.spec_init.default, self.enumeration(&node.type_name))
        {
            let is_member = match &node.spec_init.spec {
                SpecificationKind::Inline(spec) => spec
                    .values
                    .iter()
                    .any(|member| member.value == default.value),
                SpecificationKind::Named(_) => members.contains(&default.value),
            };
            self.check_value(default, UsedAs::Type(id), is_member);
        }
        node.recurse_visit(self)
    }

    fn visit_enumerated_initial_value_assignment(
        &mut self,
        init: &EnumeratedInitialValueAssignment,
    ) -> Result<Self::Value, Infallible> {
        let (id, members) = match self.enumeration(&init.type_name) {
            Ok(enumeration) => enumeration,
            Err(diagnostic) => {
                // The type is not an enumeration, so there is nothing to check
                // this initializer's value against. Report that and carry on to
                // the next declaration.
                self.diagnostics.push(diagnostic);
                return Ok(());
            }
        };
        if let Some(value) = &init.initial_value {
            self.check_value(value, UsedAs::Type(id), members.contains(&value.value));
        }

        Ok(())
    }

    fn visit_enumerated_values_initializer(
        &mut self,
        init: &EnumeratedValuesInitializer,
    ) -> Result<Self::Value, Infallible> {
        // An inline enumeration's initial value must be one of the values
        // the declaration itself lists.
        if let Some(value) = &init.initial_value {
            let is_member = init.values.iter().any(|member| member.value == value.value);
            self.check_value(value, UsedAs::Inline, is_member);
        }

        Ok(())
    }

    fn visit_case(&mut self, node: &Case) -> Result<Self::Value, Infallible> {
        let Some(selector) = node.selector.expr_type.as_ref().and_then(ExprType::type_id) else {
            return node.recurse_visit(self);
        };
        let members = self
            .context
            .types()
            .get_by_id(selector)
            .and_then(|attributes| attributes.representation.enumeration_members());
        for selection in node
            .statement_groups
            .iter()
            .flat_map(|group| &group.selectors)
        {
            let CaseSelectionKind::EnumeratedValue(value) = selection else {
                continue;
            };
            match members {
                Some(members) => self.check_value(
                    value,
                    UsedAs::Type(selector),
                    members.contains(&value.value),
                ),
                // A selector that is not an enumeration has no members for a
                // label to name, and a label written without its type may
                // name something else; one written with its type names an
                // enumeration, which is not the selector's type.
                None => {
                    self.written_type_matches(value, UsedAs::Type(selector));
                }
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::diagnostic_codes;
    use crate::test_helpers::{rule_codes, rule_diagnostics};
    use ironplc_dsl::diagnostic::Diagnostic;
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;
    use rstest::rstest;

    /// The diagnostics this rule reports for `program` under default options.
    fn diagnostics_of(program: &str) -> Vec<Diagnostic> {
        rule_diagnostics(super::apply, program, &CompilerOptions::default())
    }

    fn codes_of(program: &str) -> Vec<String> {
        rule_codes(super::apply, program, &CompilerOptions::default())
    }

    #[test]
    fn apply_when_two_undefined_enum_values_then_reports_both() {
        let program = "
TYPE
LEVEL : (INFO, WARN) := INFO;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
A : LEVEL := CRITICAL;
B : LEVEL := FATAL;
END_VAR
END_FUNCTION_BLOCK";

        let diagnostics = diagnostics_of(program);

        let codes = diagnostic_codes(&diagnostics);
        assert_eq!(codes, [Problem::EnumValueNotDefined.code(); 2]);
        let reported: Vec<&str> = diagnostics
            .iter()
            .flat_map(|d| &d.described)
            .map(String::as_str)
            .filter(|d| d.starts_with("value="))
            .collect();
        assert_eq!(reported, ["value=CRITICAL", "value=FATAL"]);
    }

    #[test]
    fn apply_when_two_enumerations_in_one_block_then_ok_on_every_run() {
        // Each analysis builds its hash maps with fresh random keys, so a
        // result that depends on hash iteration order shows up within a
        // few runs (issue #1945).
        let program = "
TYPE A1 : (P, Q, R); A2 : (S0, S1); END_TYPE
PROGRAM main
VAR st : A2 := S1; pt : A1 := R; END_VAR
END_PROGRAM";

        for _ in 0..32 {
            assert_eq!(codes_of(program), Vec::<String>::new());
        }
    }

    #[test]
    fn apply_when_two_enumerations_declare_same_value_then_each_accepts_it() {
        let program = "
TYPE A : (X, Y); B : (X, Z); END_TYPE
PROGRAM main
VAR a : A := X; b : B := X; END_VAR
END_PROGRAM";

        assert_eq!(codes_of(program), Vec::<String>::new());
    }

    #[test]
    fn apply_when_value_of_other_enumeration_then_enum_value_not_defined() {
        let program = "
TYPE A : (X, Y); B : (X, Z); END_TYPE
PROGRAM main
VAR a : A := Z; END_VAR
END_PROGRAM";

        assert_eq!(codes_of(program), [Problem::EnumValueNotDefined.code()]);
    }

    #[test]
    fn apply_when_value_of_enumeration_with_equal_representation_then_enum_value_not_defined_on_every_run(
    ) {
        // Every enumeration has the same representation, so neither is an
        // alias of the other, whichever order the types are visited in.
        let program = "
TYPE A1 : (P, Q, R); A2 : (S0, S1); END_TYPE
PROGRAM main
VAR b : A2 := P; END_VAR
END_PROGRAM";

        for _ in 0..32 {
            assert_eq!(codes_of(program), [Problem::EnumValueNotDefined.code()]);
        }
    }

    #[test]
    fn apply_when_alias_chain_then_values_of_declaring_enumeration() {
        let program = "
TYPE LEVEL : (INFO, WARN); LEVEL1 : LEVEL; LEVEL2 : LEVEL1; END_TYPE
PROGRAM main
VAR ok : LEVEL2 := WARN; bad : LEVEL2 := FATAL; END_VAR
END_PROGRAM";

        for _ in 0..32 {
            assert_eq!(codes_of(program), [Problem::EnumValueNotDefined.code()]);
        }
    }

    #[test]
    fn apply_when_multiple_enum_values_with_one_undefined_then_error() {
        let program = "
TYPE
LEVEL : (INFO, WARN) := INFO;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
A : LEVEL := INFO;
B : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        assert_eq!(codes_of(program), [Problem::EnumValueNotDefined.code()]);
    }

    #[test]
    fn apply_when_var_init_undefined_enum_value_then_error() {
        let program = "
TYPE
LEVEL : (INFO) := INFO;
END_TYPE
        
FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        assert_eq!(codes_of(program), [Problem::EnumValueNotDefined.code()]);
    }

    rule_ok!(
        apply_when_var_init_valid_enum_value_then_ok,
        "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_var_init_valid_enum_value_through_alias_then_ok,
        "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
NAME : LEVEL_ALIAS := CRITICAL;
END_VAR

END_FUNCTION_BLOCK"
    );

    rule_err_at!(
        apply_when_var_init_undefined_enum_value_through_alias_then_error,
        "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
NAME : LEVEL_ALIAS := FATAL;
END_VAR

END_FUNCTION_BLOCK",
        Problem::EnumValueNotDefined,
        "FATAL"
    );

    #[test]
    fn apply_when_two_enums_share_value_then_each_value_defined() {
        let diagnostics = diagnostics_of(
            "
TYPE
  Colors : (Red, Green) := Red;
  Lights : (Red, Amber) := Red;
END_TYPE

PROGRAM main
VAR
  c : Colors := Colors#Green;
  l : Lights := Lights#Red;
END_VAR
END_PROGRAM",
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn apply_when_global_shares_enum_value_name_then_value_defined() {
        let diagnostics = diagnostics_of(
            "
TYPE
  Colors : (Red, Green) := Red;
END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    Red : INT;
  END_VAR
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR
  c : Colors := Colors#Red;
END_VAR
END_PROGRAM",
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    rule_err_at!(
        apply_when_inline_enum_init_not_a_member_then_enum_value_not_defined,
        "
PROGRAM main
VAR
e : (A, B) := C;
END_VAR
END_PROGRAM",
        Problem::EnumValueNotDefined,
        "C"
    );

    rule_ok!(
        apply_when_inline_enum_init_is_a_member_then_ok,
        "
PROGRAM main
VAR
e : (A, B) := B;
END_VAR
END_PROGRAM"
    );

    /// A program whose variables are `declarations` and whose body is
    /// `body`, and the enumerations and aliases the tests of a value written
    /// with its type use. The types follow the program, so that the first
    /// place a type name appears is where the program uses it.
    fn program_with_written_types(declarations: &str, body: &str) -> String {
        format!(
            "
PROGRAM main
VAR
{declarations}
END_VAR
{body}
END_PROGRAM

TYPE
  E_Color : (red, green);
  E_Size : (small, large);
  E_Twin : (red, green);
  E_Paint : E_Color;
  E_Shade : E_Paint;
  E_Tint : E_Color;
END_TYPE"
        )
    }

    rule_err_at!(
        apply_when_written_type_is_other_enumeration_then_enum_value_type_mismatch,
        &program_with_written_types("c : E_Color := E_Size#red;", ""),
        Problem::EnumValueTypeMismatch,
        "E_Size"
    );

    #[rstest]
    // The value is not a member of the written type either.
    #[case::value_of_neither("c : E_Color := E_Size#red;")]
    // The value is a member of the written type, which is not the
    // declared type.
    #[case::value_of_written_type("c : E_Color := E_Size#small;")]
    // Two enumerations that list the same members are two enumerations.
    #[case::same_members("c : E_Color := E_Twin#red;")]
    #[case::alias_of_other_enumeration("s : E_Size := E_Paint#red;")]
    #[case::declared_alias_and_other_enumeration("p : E_Paint := E_Twin#green;")]
    fn apply_when_written_type_is_not_declared_enumeration_then_enum_value_type_mismatch(
        #[case] declaration: &str,
    ) {
        assert_eq!(
            codes_of(&program_with_written_types(declaration, "")),
            [Problem::EnumValueTypeMismatch.code()]
        );
    }

    #[rstest]
    #[case::declared_enumeration("c : E_Color := E_Color#red;")]
    #[case::alias_of_declared_enumeration("c : E_Color := E_Paint#red;")]
    #[case::alias_chain_of_declared_enumeration("c : E_Color := E_Shade#green;")]
    #[case::declared_alias("p : E_Paint := E_Color#green;")]
    #[case::alias_of_same_enumeration("p : E_Paint := E_Tint#green;")]
    fn apply_when_written_type_is_declared_enumeration_or_alias_then_ok(#[case] declaration: &str) {
        assert_eq!(
            codes_of(&program_with_written_types(declaration, "")),
            Vec::<String>::new()
        );
    }

    rule_err_at!(
        apply_when_written_type_is_declared_enumeration_and_value_undefined_then_enum_value_not_defined,
        &program_with_written_types("c : E_Color := E_Color#blue;", ""),
        Problem::EnumValueNotDefined,
        "E_Color#blue"
    );

    rule_err_at!(
        apply_when_written_type_not_declared_then_enum_not_declared,
        &program_with_written_types("c : E_Color := E_Nope#red;", ""),
        Problem::EnumNotDeclared,
        "E_Nope"
    );

    rule_err_at!(
        apply_when_inline_enum_init_has_written_type_then_enum_value_type_mismatch,
        &program_with_written_types("c : (red, blue) := E_Color#red;", ""),
        Problem::EnumValueTypeMismatch,
        "E_Color"
    );

    rule_err_at!(
        apply_when_structure_field_init_has_other_written_type_then_enum_value_type_mismatch,
        "
TYPE
  S : STRUCT c : E_Color := E_Size#small; END_STRUCT;
  E_Color : (red, green);
  E_Size : (small, large);
END_TYPE",
        Problem::EnumValueTypeMismatch,
        "E_Size"
    );

    #[rstest]
    #[case::declaration_with_values("E_Light : (red, green) := E_Size#red;")]
    #[case::alias("E_Paint : E_Color := E_Size#red;")]
    fn apply_when_type_default_has_other_written_type_then_enum_value_type_mismatch(
        #[case] declaration: &str,
    ) {
        let program = format!(
            "
TYPE
  E_Color : (red, green);
  E_Size : (small, large);
  {declaration}
END_TYPE"
        );

        assert_eq!(codes_of(&program), [Problem::EnumValueTypeMismatch.code()]);
    }

    rule_ok!(
        apply_when_type_default_written_with_declared_type_or_alias_then_ok,
        "
TYPE
  E_Color : (red, green) := E_Color#green;
  E_Paint : E_Color := E_Color#red;
  E_Shade : E_Paint := E_Paint#green;
END_TYPE"
    );

    #[rstest]
    #[case::declaration_with_values("E_Color : (red, green) := blue;")]
    #[case::alias("E_Paint : E_Color := blue;")]
    fn apply_when_type_default_not_a_member_then_enum_value_not_defined(#[case] declaration: &str) {
        let program = format!(
            "
TYPE
  E_Color : (red, green);
  {declaration}
END_TYPE"
        );

        assert_eq!(codes_of(&program), [Problem::EnumValueNotDefined.code()]);
    }

    /// A program that selects on `selector`, declared as `selector_type`,
    /// with one `CASE` label, `label`.
    fn program_with_case_label(selector_type: &str, label: &str) -> String {
        program_with_written_types(
            &format!("sel : {selector_type}; x : INT;"),
            &format!("CASE sel OF\n  {label}: x := 1;\nEND_CASE;"),
        )
    }

    rule_err_at!(
        apply_when_case_label_written_with_other_enumeration_then_enum_value_type_mismatch,
        &program_with_case_label("E_Color", "E_Size#red"),
        Problem::EnumValueTypeMismatch,
        "E_Size"
    );

    #[rstest]
    #[case::value_of_written_type("E_Color", "E_Size#small")]
    #[case::same_members("E_Color", "E_Twin#red")]
    #[case::integer_selector("INT", "E_Color#red")]
    fn apply_when_case_label_written_with_type_other_than_selector_then_enum_value_type_mismatch(
        #[case] selector_type: &str,
        #[case] label: &str,
    ) {
        assert_eq!(
            codes_of(&program_with_case_label(selector_type, label)),
            [Problem::EnumValueTypeMismatch.code()]
        );
    }

    #[rstest]
    #[case::unqualified("E_Color", "green")]
    #[case::selector_type("E_Color", "E_Color#red")]
    #[case::alias_of_selector_type("E_Color", "E_Shade#red")]
    #[case::alias_selector("E_Paint", "E_Color#green")]
    #[case::alias_of_same_enumeration("E_Paint", "E_Tint#green")]
    fn apply_when_case_label_is_value_of_selector_type_then_ok(
        #[case] selector_type: &str,
        #[case] label: &str,
    ) {
        assert_eq!(
            codes_of(&program_with_case_label(selector_type, label)),
            Vec::<String>::new()
        );
    }

    #[rstest]
    #[case::unqualified("blue")]
    #[case::value_of_other_enumeration("small")]
    #[case::written_with_selector_type("E_Color#blue")]
    fn apply_when_case_label_not_a_member_then_enum_value_not_defined(#[case] label: &str) {
        assert_eq!(
            codes_of(&program_with_case_label("E_Color", label)),
            [Problem::EnumValueNotDefined.code()]
        );
    }

    rule_err_at!(
        apply_when_case_label_of_inline_enumeration_has_written_type_then_enum_value_type_mismatch,
        &program_with_case_label("(red, blue)", "E_Color#red"),
        Problem::EnumValueTypeMismatch,
        "E_Color"
    );

    // The examples of `docs/reference/compiler/problems/P2045.rst`.
    rule_err!(
        apply_when_documented_failing_example_then_enum_value_type_mismatch,
        "
TYPE
    Color : (Red, Green);
    Size : (Small, Large);
END_TYPE

PROGRAM main
    VAR
        shade : Color := Size#Small;
        count : INT;
    END_VAR
    CASE shade OF
        Size#Large: count := 1;
    END_CASE;
END_PROGRAM",
        [Problem::EnumValueTypeMismatch; 2]
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        "
TYPE
    Color : (Red, Green);
    Size : (Small, Large);
    Paint : Color;
END_TYPE

PROGRAM main
    VAR
        shade : Color := Color#Green;
        count : INT;
    END_VAR
    CASE shade OF
        Paint#Red: count := 1;
        Green: count := 2;
    END_CASE;
END_PROGRAM"
    );
}
