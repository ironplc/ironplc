//! OOP extension: member qualifiers (`PUBLIC`, `PRIVATE`, `PROTECTED`,
//! `INTERNAL`, `FINAL`, `OVERRIDE`, `ABSTRACT`) on methods.
//! See specs/design/beckhoff-twincat-dialect.md §1.5.

use super::common::*;
use dsl::common::MethodDeclaration;
use dsl::member_qualifier::{AccessSpecifier, MemberQualifierKind};

/// Parses `method` as the only method of a function block and returns it.
fn parse_method(method: &str) -> MethodDeclaration {
    let source = format!(
        "
FUNCTION_BLOCK ABSTRACT FB_Motor
VAR
    x : INT;
END_VAR
{method}
END_FUNCTION_BLOCK"
    );
    let library = parse_program(&source, &FileId::default(), &opts_with_fb_inheritance())
        .unwrap_or_else(|e| panic!("Source did not parse: {e:?}\n{source}"));
    let fb = extract_fb(&library);
    assert_eq!(fb.methods.len(), 1);
    fb.methods[0].clone()
}

fn kinds(method: &MethodDeclaration) -> Vec<MemberQualifierKind> {
    method.qualifiers.iter().map(|q| q.kind).collect()
}

const PUBLIC: MemberQualifierKind = MemberQualifierKind::Access(AccessSpecifier::Public);
const PRIVATE: MemberQualifierKind = MemberQualifierKind::Access(AccessSpecifier::Private);
const PROTECTED: MemberQualifierKind = MemberQualifierKind::Access(AccessSpecifier::Protected);
const INTERNAL: MemberQualifierKind = MemberQualifierKind::Access(AccessSpecifier::Internal);

#[rstest]
#[case::private("METHOD PRIVATE Reset\n    x := 0;\nEND_METHOD", vec![PRIVATE])]
#[case::public("METHOD PUBLIC Reset\n    x := 0;\nEND_METHOD", vec![PUBLIC])]
#[case::protected("METHOD PROTECTED Reset\n    x := 0;\nEND_METHOD", vec![PROTECTED])]
#[case::internal("METHOD INTERNAL Reset\n    x := 0;\nEND_METHOD", vec![INTERNAL])]
#[case::final_only("METHOD FINAL Reset\n    x := 0;\nEND_METHOD", vec![MemberQualifierKind::Final])]
#[case::override_only("METHOD OVERRIDE Reset\n    x := 0;\nEND_METHOD", vec![MemberQualifierKind::Override])]
#[case::public_final(
    "METHOD PUBLIC FINAL Reset : BOOL\n    x := 0;\nEND_METHOD",
    vec![PUBLIC, MemberQualifierKind::Final]
)]
#[case::public_abstract(
    "METHOD PUBLIC ABSTRACT Reset : BOOL\n    x := 0;\nEND_METHOD",
    vec![PUBLIC, MemberQualifierKind::Abstract]
)]
#[case::lower_case("METHOD private Reset\n    x := 0;\nEND_METHOD", vec![PRIVATE])]
#[case::order_kept_when_wrong(
    "METHOD FINAL PUBLIC Reset\n    x := 0;\nEND_METHOD",
    vec![MemberQualifierKind::Final, PUBLIC]
)]
#[case::duplicate_kept("METHOD PUBLIC PUBLIC Reset\n    x := 0;\nEND_METHOD", vec![PUBLIC, PUBLIC])]
#[case::body_starts_with_assignment("METHOD PRIVATE Reset\n    x := 1;\nEND_METHOD", vec![PRIVATE])]
#[case::body_starts_with_call("METHOD PRIVATE Reset\n    Init();\nEND_METHOD", vec![PRIVATE])]
fn parse_when_method_has_qualifiers_then_kept_in_source_order(
    #[case] method: &str,
    #[case] expected: Vec<MemberQualifierKind>,
) {
    let method = parse_method(method);
    assert_eq!(method.name, Id::from("Reset"));
    assert_eq!(kinds(&method), expected);
}

#[test]
fn parse_when_method_has_no_qualifiers_then_qualifiers_empty() {
    let method = parse_method("METHOD Reset\n    x := 0;\nEND_METHOD");
    assert!(kinds(&method).is_empty());
}

#[test]
fn parse_when_method_has_qualifier_then_body_and_return_type_kept() {
    let method = parse_method(
        "METHOD PUBLIC Start : BOOL
VAR_INPUT
    speed : INT;
END_VAR
    x := speed;
    Start := TRUE;
END_METHOD",
    );
    assert_eq!(kinds(&method), vec![PUBLIC]);
    assert_eq!(
        method.return_type,
        Some(FunctionReturnType::Named(TypeName::from("BOOL")))
    );
    assert_eq!(method.variables.len(), 1);
    assert_eq!(method.body.len(), 2);
}

/// A qualifier word is only a qualifier when the method name follows it.
/// Otherwise it is the method name, as it was before qualifiers existed.
/// `OVERRIDE` in particular is a legal name in TwinCAT 4024.
#[rstest]
#[case::with_return_type("METHOD Override : BOOL\n    x := 0;\nEND_METHOD", "Override")]
#[case::with_var_block(
    "METHOD Private\nVAR\n    y : INT;\nEND_VAR\n    x := 0;\nEND_METHOD",
    "Private"
)]
#[case::empty("METHOD Final\n    x := 0;\nEND_METHOD", "Final")]
#[case::body_assignment("METHOD Override\n    x := 1;\nEND_METHOD", "Override")]
#[case::body_call("METHOD Override\n    x();\nEND_METHOD", "Override")]
#[case::body_member_access("METHOD Override\n    x.y := 1;\nEND_METHOD", "Override")]
#[case::body_subscript("METHOD Override\n    x[0] := 1;\nEND_METHOD", "Override")]
#[case::body_deref("METHOD Override\n    x^ := 1;\nEND_METHOD", "Override")]
#[case::body_set_bind("METHOD Override\n    x S= TRUE;\nEND_METHOD", "Override")]
fn parse_when_qualifier_word_is_method_name_then_not_a_qualifier(
    #[case] method: &str,
    #[case] name: &str,
) {
    let method = parse_method(method);
    assert_eq!(method.name, Id::from(name));
    assert!(kinds(&method).is_empty());
}

#[test]
fn parse_when_qualifier_word_follows_qualifier_as_name_then_it_is_the_name() {
    let method = parse_method("METHOD PUBLIC Override : BOOL\n    x := 0;\nEND_METHOD");
    assert_eq!(method.name, Id::from("Override"));
    assert_eq!(kinds(&method), vec![PUBLIC]);
}

#[test]
fn parse_when_qualifier_words_are_variable_names_then_ok() {
    let method = parse_method(
        "METHOD PRIVATE Reset
VAR
    Public : INT;
    Private : INT;
    Protected : INT;
    Internal : INT;
    Final : INT;
    Override : INT;
END_VAR
    x := Private;
    Final := Override + Public;
END_METHOD",
    );
    assert_eq!(kinds(&method), vec![PRIVATE]);
    assert_eq!(method.variables.len(), 6);
    assert_eq!(method.body.len(), 2);
}
