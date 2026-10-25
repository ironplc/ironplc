//! Tests of `rule_override_signature`. The derived block comes first in a
//! program whose label location is asserted, so that the overriding
//! method's name is the first occurrence of it.

use super::*;
use crate::test_helpers::{
    codes, diagnostic_codes, fb_inheritance_options, rule_codes, rule_diagnostics,
};
use rstest::rstest;

/// `FB_Root` declares `M` as `base`; `FB_Mid`, which extends it, declares
/// `M` as `derived`.
fn overriding(derived: &str, base: &str) -> String {
    format!(
        "
FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD {derived}
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Root
METHOD {base}
END_METHOD
END_FUNCTION_BLOCK"
    )
}

fn override_codes(derived: &str, base: &str) -> Vec<String> {
    rule_codes(apply, &overriding(derived, base), &fb_inheritance_options())
}

#[rstest]
#[case::same_return_type("M : INT", "M : INT")]
#[case::no_return_type("M", "M")]
#[case::same_parameters(
    "M\nVAR_INPUT a : INT; END_VAR\nVAR_OUTPUT q : BOOL; END_VAR\nVAR_IN_OUT io : DINT; END_VAR",
    "M\nVAR_INPUT a : INT; END_VAR\nVAR_OUTPUT q : BOOL; END_VAR\nVAR_IN_OUT io : DINT; END_VAR"
)]
#[case::extra_local(
    "M : INT\nVAR_INPUT a : INT; END_VAR\nVAR tmp : INT; END_VAR",
    "M : INT\nVAR_INPUT a : INT; END_VAR"
)]
#[case::same_sized_string("M : STRING[10]", "M : STRING[10]")]
#[case::same_inline_array(
    "M\nVAR_IN_OUT a : ARRAY[1..3] OF INT; END_VAR",
    "M\nVAR_IN_OUT a : ARRAY[1..3] OF INT; END_VAR"
)]
#[case::same_edge_input(
    "M\nVAR_INPUT go : BOOL R_EDGE; END_VAR",
    "M\nVAR_INPUT go : BOOL R_EDGE; END_VAR"
)]
#[case::unresolved_parameter_type(
    "M\nVAR_INPUT a : Undeclared; END_VAR",
    "M\nVAR_INPUT a : INT; END_VAR"
)]
fn apply_when_override_has_same_signature_then_ok(#[case] derived: &str, #[case] base: &str) {
    assert_eq!(codes(&[]), override_codes(derived, base));
}

#[rstest]
#[case::return_type("M : BOOL", "M : INT")]
#[case::return_type_added("M : INT", "M")]
#[case::return_type_removed("M", "M : INT")]
#[case::sized_string_capacity("M : STRING[20]", "M : STRING[10]")]
#[case::parameter_renamed("M\nVAR_INPUT b : INT; END_VAR", "M\nVAR_INPUT a : INT; END_VAR")]
#[case::parameter_retyped("M\nVAR_INPUT a : DINT; END_VAR", "M\nVAR_INPUT a : INT; END_VAR")]
#[case::parameter_kind("M\nVAR_IN_OUT a : INT; END_VAR", "M\nVAR_INPUT a : INT; END_VAR")]
#[case::parameter_added(
    "M\nVAR_INPUT a : INT; b : INT; END_VAR",
    "M\nVAR_INPUT a : INT; END_VAR"
)]
#[case::parameter_removed("M", "M\nVAR_INPUT a : INT; END_VAR")]
#[case::parameters_reordered(
    "M\nVAR_INPUT b : INT; a : INT; END_VAR",
    "M\nVAR_INPUT a : INT; b : INT; END_VAR"
)]
#[case::edge_input(
    "M\nVAR_INPUT go : BOOL F_EDGE; END_VAR",
    "M\nVAR_INPUT go : BOOL R_EDGE; END_VAR"
)]
fn apply_when_override_differs_then_error(#[case] derived: &str, #[case] base: &str) {
    assert_eq!(
        codes(&[Problem::OverrideSignatureMismatch]),
        override_codes(derived, base)
    );
}

rule_err_at!(
    apply_when_return_type_differs_then_error_at_overriding_method,
    "
FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Root
METHOD Describe : INT
END_METHOD
END_FUNCTION_BLOCK",
    Problem::OverrideSignatureMismatch,
    "Describe",
    fb_inheritance_options()
);

#[test]
fn apply_when_return_type_differs_then_label_names_both_types() {
    let diagnostics = rule_diagnostics(
        apply,
        &overriding("M : BOOL", "M : INT"),
        &fb_inheritance_options(),
    );

    assert_eq!(
        diagnostic_codes(&diagnostics),
        codes(&[Problem::OverrideSignatureMismatch])
    );
    assert_eq!(
        "Returns BOOL, where FB_Root.M returns INT",
        diagnostics[0].primary.message
    );
}

// `FB_Leaf` overrides `FB_Root.Describe` through `FB_Mid`, which declares no
// `Describe` of its own.
rule_err!(
    apply_when_override_two_levels_up_differs_then_error,
    "
FUNCTION_BLOCK FB_Root
METHOD Describe : INT
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Leaf EXTENDS FB_Mid
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    [Problem::OverrideSignatureMismatch],
    fb_inheritance_options()
);

// Each override is compared with the nearest: `FB_Mid` differs from
// `FB_Root`, and `FB_Leaf` matches `FB_Mid`, so only `FB_Mid` is reported.
rule_err!(
    apply_when_override_matches_nearest_override_then_only_that_one_reported,
    "
FUNCTION_BLOCK FB_Root
METHOD Describe : INT
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Leaf EXTENDS FB_Mid
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    [Problem::OverrideSignatureMismatch],
    fb_inheritance_options()
);

rule_err!(
    apply_when_abstract_method_overridden_differently_then_error,
    "
FUNCTION_BLOCK ABSTRACT FB_Root
METHOD ABSTRACT Describe : INT
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    [Problem::OverrideSignatureMismatch],
    fb_inheritance_options()
);

// A method of a name the base does not declare overrides nothing.
rule_ok!(
    apply_when_method_not_inherited_then_ok,
    "
FUNCTION_BLOCK FB_Root
METHOD Describe : INT
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Run : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    fb_inheritance_options()
);

// The missing base is reported for itself (P4088).
rule_ok!(
    apply_when_base_not_declared_then_ok,
    "
FUNCTION_BLOCK FB_Mid EXTENDS FB_Missing
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    fb_inheritance_options()
);

// `FB_Mid.Describe` differs from the interface member it implements, which
// interface conformance reports (P4068); it is not reported again here.
rule_ok!(
    apply_when_override_differs_from_interface_member_then_ok,
    "
INTERFACE I_Describe
METHOD Describe : INT
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Root IMPLEMENTS I_Describe
METHOD Describe : INT
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK",
    fb_inheritance_options()
);

/// `FB_Root` declares the property `P` as `base`, with a `GET` and a `SET`;
/// `FB_Mid`, which extends it, declares `P` as `derived`, with `accessors`.
fn overriding_property(derived: &str, accessors: &str, base: &str) -> String {
    format!(
        "
FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
PROPERTY {derived}
{accessors}
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Root
PROPERTY {base}
GET
END_GET
SET
END_SET
END_PROPERTY
END_FUNCTION_BLOCK"
    )
}

const GET_AND_SET: &str = "GET\nEND_GET\nSET\nEND_SET";

#[rstest]
#[case::same_type("P : INT", GET_AND_SET, "P : INT")]
#[case::same_sized_string("P : STRING[10]", GET_AND_SET, "P : STRING[10]")]
#[case::unresolved_type("P : Undeclared", GET_AND_SET, "P : INT")]
// Which accessors an override keeps is not compared.
#[case::get_only("P : INT", "GET\nEND_GET", "P : INT")]
fn apply_when_property_override_has_same_type_then_ok(
    #[case] derived: &str,
    #[case] accessors: &str,
    #[case] base: &str,
) {
    assert_eq!(
        codes(&[]),
        rule_codes(
            apply,
            &overriding_property(derived, accessors, base),
            &fb_inheritance_options()
        )
    );
}

#[rstest]
#[case::other_type("P : BOOL", "P : INT")]
#[case::sized_string_capacity("P : STRING[20]", "P : STRING[10]")]
fn apply_when_property_override_has_other_type_then_error(
    #[case] derived: &str,
    #[case] base: &str,
) {
    assert_eq!(
        codes(&[Problem::OverridePropertyTypeMismatch]),
        rule_codes(
            apply,
            &overriding_property(derived, GET_AND_SET, base),
            &fb_inheritance_options()
        )
    );
}

rule_err_at!(
    apply_when_property_type_differs_then_error_at_overriding_property,
    "
FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
PROPERTY Speed : BOOL
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Root
PROPERTY Speed : INT
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
    Problem::OverridePropertyTypeMismatch,
    "Speed",
    fb_inheritance_options()
);

#[test]
fn apply_when_property_type_differs_then_label_names_both_types() {
    let diagnostics = rule_diagnostics(
        apply,
        &overriding_property("P : BOOL", GET_AND_SET, "P : INT"),
        &fb_inheritance_options(),
    );

    assert_eq!(
        diagnostic_codes(&diagnostics),
        codes(&[Problem::OverridePropertyTypeMismatch])
    );
    assert_eq!(
        "Has type BOOL, where the property it overrides has type INT",
        diagnostics[0].primary.message
    );
}

// `FB_Leaf` overrides `FB_Root.Speed` through `FB_Mid`, which declares no
// `Speed` of its own.
rule_err!(
    apply_when_property_override_two_levels_up_differs_then_error,
    "
FUNCTION_BLOCK FB_Root
PROPERTY Speed : INT
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Leaf EXTENDS FB_Mid
PROPERTY Speed : BOOL
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
    [Problem::OverridePropertyTypeMismatch],
    fb_inheritance_options()
);

// A property of a name the base does not declare overrides nothing.
rule_ok!(
    apply_when_property_not_inherited_then_ok,
    "
FUNCTION_BLOCK FB_Root
PROPERTY Speed : INT
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
PROPERTY Ready : BOOL
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
    fb_inheritance_options()
);

// `FB_Mid.Speed` does not provide the interface property it implements,
// which interface conformance reports (P4068); it is not reported again
// here.
rule_ok!(
    apply_when_property_override_differs_from_interface_property_then_ok,
    "
INTERFACE I_Speed
PROPERTY Speed : INT
GET
END_GET
END_PROPERTY
END_INTERFACE

FUNCTION_BLOCK FB_Root IMPLEMENTS I_Speed
PROPERTY Speed : INT
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
PROPERTY Speed : BOOL
GET
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
    fb_inheritance_options()
);
