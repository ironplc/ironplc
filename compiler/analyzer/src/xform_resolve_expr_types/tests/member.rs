//! The type of a field of a structure, wherever the structure is: a
//! variable, an element of an array variable, or an element of an array that
//! is itself a field.

use super::*;
use spec_test_macro::spec_test;

/// The type of `value` assigned in a program declaring structures nested in
/// arrays and fields of each kind the tests read.
fn member_type(value: &str) -> Option<String> {
    let program = format!(
        "TYPE Small : INT (-100..100); END_TYPE
         TYPE Inner : STRUCT v : LINT; END_STRUCT; END_TYPE
         TYPE Item : STRUCT a : DINT; r : Small; inner : ARRAY[1..2] OF Inner; END_STRUCT; END_TYPE
         TYPE Holder : STRUCT items : ARRAY[1..3] OF Item; one : Item; END_STRUCT; END_TYPE
         PROGRAM main
         VAR x : LINT; h : Holder; it : Item; arr : ARRAY[1..2] OF Item; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let resolved = run_pass(&program);
    type_name_upper(&collect_assignment_types(&resolved)[0])
}

#[spec_test(REQ_IC_analyzer_095)]
#[rstest]
#[case::field_of_variable("it.a", "DINT")]
#[case::field_of_field("h.one.a", "DINT")]
#[case::field_of_array_element("arr[1].a", "DINT")]
#[case::field_of_array_field_element("h.items[1].a", "DINT")]
#[case::field_through_two_array_fields("h.items[1].inner[2].v", "LINT")]
#[case::subrange_field("it.r", "INT")]
#[case::subrange_field_of_array_field_element("h.items[2].r", "INT")]
#[case::arithmetic_on_fields("h.items[1].a + h.items[2].a", "DINT")]
fn apply_when_field_of_structure_then_has_declared_type(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(member_type(value), Some(expected.to_string()));
}

/// The type of `value` assigned in a program declaring structure and
/// function block fields of string, enumeration and string-array types, as
/// the name the type was declared with.
fn declared_field_type(value: &str) -> Option<String> {
    let program = format!(
        "TYPE COLOR : (RED, GREEN); END_TYPE
         TYPE Holder : STRUCT
           name : STRING[10];
           wide : WSTRING[10];
           names : ARRAY[1..2] OF WSTRING[10];
           color : COLOR;
         END_STRUCT; END_TYPE
         TYPE Alias : Holder; END_TYPE
         FUNCTION_BLOCK fb
         VAR_OUTPUT label : WSTRING[10]; state : COLOR; END_VAR
         END_FUNCTION_BLOCK
         PROGRAM main
         VAR h : Holder; a : Alias; inst : fb; s : STRING[10]; END_VAR
         s := {value};
         END_PROGRAM"
    );
    let resolved = run_pass(&program);
    let value = assignment_values(&resolved.library).remove(0);
    let (ExprType::Concrete(id) | ExprType::Inferred(id)) = value.expr_type? else {
        return None;
    };
    resolved
        .types
        .name_of(id)
        .map(|name| name.id.original().to_uppercase())
}

/// The value of each assignment, in order.
fn assignment_values(library: &Library) -> Vec<Expr> {
    struct Values(Vec<Expr>);
    impl Fold<()> for Values {
        fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, ()> {
            self.0.push(node.value.clone());
            node.recurse_fold(self)
        }
    }
    let mut values = Values(vec![]);
    let _ = values.fold_library(library.clone());
    values.0
}

#[spec_test(REQ_IC_analyzer_103)]
#[rstest]
#[case::string_field("h.name", "STRING")]
#[case::wstring_field("h.wide", "WSTRING")]
#[case::element_of_wstring_array_field("h.names[2]", "WSTRING")]
#[case::enumeration_field("h.color", "COLOR")]
#[case::field_of_alias_of_structure("a.wide", "WSTRING")]
#[case::function_block_wstring_output("inst.label", "WSTRING")]
#[case::function_block_enumeration_output("inst.state", "COLOR")]
fn apply_when_field_of_any_type_then_has_declared_type(
    #[case] value: &str,
    #[case] expected: &str,
) {
    assert_eq!(declared_field_type(value), Some(expected.to_string()));
}

/// A field a function block inherits through `EXTENDS`, read from outside
/// the instance, has the type the block that declares it gives it.
#[test]
fn apply_when_inherited_field_of_function_block_then_has_declared_type() {
    let program = "
FUNCTION_BLOCK Base VAR_OUTPUT count : LINT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base END_FUNCTION_BLOCK
PROGRAM main VAR d : Derived; x : LINT; END_VAR x := d.count; END_PROGRAM";
    let resolved = run_pass_with_options(program, &crate::test_helpers::fb_inheritance_options());
    assert_type_eq(&collect_assignment_types(&resolved)[0], "LINT");
}
