//! Unit tests for the conversion of a value stored into a variable of a
//! function block or a loop: an input of a function block call, an
//! assignment to a function block field or through a dereference, an
//! argument of a method call, and the bounds and step of a `FOR` loop.

use std::convert::Infallible;

use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::CompilerOptions;

use spec_test_macro::spec_test;

use super::tests::{assigned_values_with, describe};
use crate::stages::analyze;
use crate::type_environment::TypeEnvironment;

/// The values a function block call, a method call or a `FOR` loop stores,
/// in source order, each as [`describe`] shows it: the inputs of a function
/// block or method call, and the bounds and step of a loop.
struct StoredValues<'a> {
    types: &'a TypeEnvironment,
    values: Vec<String>,
}

impl StoredValues<'_> {
    fn push_inputs(&mut self, params: &[ParamAssignmentKind]) {
        let inputs = params.iter().filter_map(|p| p.input_expr());
        let values: Vec<String> = inputs.map(|e| describe(self.types, e)).collect();
        self.values.extend(values);
    }
}

impl Visitor<Infallible> for StoredValues<'_> {
    type Value = ();

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.push_inputs(&node.params);
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<(), Infallible> {
        self.push_inputs(&node.params);
        node.recurse_visit(self)
    }

    fn visit_for(&mut self, node: &For) -> Result<(), Infallible> {
        let bounds = [Some(&node.from), Some(&node.to), node.step.as_ref()];
        let values: Vec<String> = bounds
            .into_iter()
            .flatten()
            .map(|e| describe(self.types, e))
            .collect();
        self.values.extend(values);
        node.recurse_visit(self)
    }
}

/// Analyzes `source` under `options`, which must be free of diagnostics,
/// and returns the values its function block calls, method calls and loops
/// store.
fn stored_values(source: &str, options: &CompilerOptions) -> Vec<String> {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = StoredValues {
        types: context.types(),
        values: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.values
}

/// A function block `B` with an `LINT` input `x` and an input `r` of `R`, a
/// subrange of `LINT`, followed by a program declaring `VAR <vars> END_VAR`
/// whose body is `body`.
fn block_program(vars: &str, body: &str) -> String {
    format!(
        "TYPE R : LINT (0..9); END_TYPE
        FUNCTION_BLOCK B VAR_INPUT x : LINT; r : R; END_VAR END_FUNCTION_BLOCK
        PROGRAM main VAR b : B; {vars} END_VAR {body} END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_038)]
#[test]
fn apply_when_function_block_field_target_then_converted_to_its_declared_type() {
    let source = block_program("d : DINT; l : LINT;", "b.x := d; b.r := d; b.r := l;");
    assert_eq!(
        assigned_values_with(&source, &CompilerOptions::default()),
        vec!["DINT->LINT", "DINT->LINT", "LINT"]
    );
}

#[spec_test(REQ_IC_analyzer_039)]
#[test]
fn apply_when_dereferenced_target_then_converted_to_referenced_type() {
    let source = "PROGRAM main VAR x : LINT; d : DINT; l : LINT; r : REF_TO LINT; END_VAR
        r := REF(x); r^ := d; r^ := l; END_PROGRAM";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let values = assigned_values_with(source, &options);
    assert_eq!(values[1..], ["DINT->LINT", "LINT"]);
}

#[spec_test(REQ_IC_analyzer_079)]
#[test]
fn apply_when_for_bounds_narrower_than_control_then_converted_to_its_type() {
    let source = "PROGRAM main VAR l : LINT; d : DINT; e : DINT; END_VAR
        FOR l := d TO e BY 2 DO d := e; END_FOR; END_PROGRAM";
    assert_eq!(
        stored_values(source, &CompilerOptions::default()),
        vec!["DINT->LINT", "DINT->LINT", "LINT"]
    );
}

#[spec_test(REQ_IC_analyzer_046)]
#[test]
fn apply_when_function_block_input_then_converted_to_declared_type_of_its_field() {
    let source = block_program(
        "c : CTU; cl : CTU_LINT; d : DINT; l : LINT; s : SINT;",
        "b(x := d, r := l); c(CU := TRUE, PV := s); cl(CU := TRUE, PV := d);",
    );
    assert_eq!(
        stored_values(&source, &CompilerOptions::default()),
        vec!["DINT->LINT", "LINT", "BOOL", "SINT", "BOOL", "DINT->LINT"]
    );
}

#[spec_test(REQ_IC_analyzer_048)]
#[test]
fn apply_when_function_block_input_positional_then_converted_to_type_of_input_in_its_place() {
    let source = "FUNCTION_BLOCK B VAR_INPUT x : LINT; END_VAR VAR_OUTPUT q : LINT; END_VAR
        VAR_INPUT y : REAL; END_VAR END_FUNCTION_BLOCK
        PROGRAM main VAR b : B; d : DINT; i : INT; END_VAR b(d, i); END_PROGRAM";
    assert_eq!(
        stored_values(source, &CompilerOptions::default()),
        vec!["DINT->LINT", "INT->REAL"]
    );
}

#[spec_test(REQ_IC_analyzer_047)]
#[test]
fn apply_when_method_argument_then_converted_to_declared_type_of_its_parameter() {
    let source = "TYPE R : LINT (0..9); END_TYPE
        FUNCTION_BLOCK K
        METHOD Set VAR_INPUT x : LINT; y : REAL; END_VAR END_METHOD
        METHOD SetR VAR_INPUT r : R; END_VAR END_METHOD
        METHOD Twice : LINT VAR_INPUT x : LINT; END_VAR Twice := x + x; END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main VAR k : K; u : UDINT; i : INT; d : DINT; a : LINT; END_VAR
        k.Set(u, i); k.Set(y := i, x := d); k.SetR(d); a := k.Twice(d); END_PROGRAM";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_eq!(
        stored_values(source, &options),
        vec![
            "UDINT->LINT",
            "INT->REAL",
            "INT->REAL",
            "DINT->LINT",
            "DINT->LINT",
            "DINT->LINT"
        ]
    );
}
