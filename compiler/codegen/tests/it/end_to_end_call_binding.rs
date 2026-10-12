//! End-to-end tests that codegen lowers the call the analyzer resolved and
//! bound: a function block instance runs the block its type is, a method call
//! runs the method its receiver's block declares or inherits, and a call's
//! arguments are evaluated in the order of its normal form. See
//! `specs/design/call-binding.md`.

use ironplc_codegen::Route;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::{ParamAssignmentKind, PositionalInput};
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

use crate::common::{assert_run_with, compile_by_route, parse, try_parse_and_compile};

/// Methods and `EXTENDS` are an extension.
fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[spec_test(REQ_CB_codegen_001)]
fn end_to_end_when_user_block_named_like_standard_then_runs_user_block() {
    let source = "
FUNCTION_BLOCK CTU_INT
VAR_INPUT CU : BOOL; END_VAR
VAR_OUTPUT CV : INT; END_VAR
CV := 42;
END_FUNCTION_BLOCK

PROGRAM main
VAR c : CTU_INT; cv : INT; END_VAR
c(CU := TRUE);
cv := c.CV;
END_PROGRAM
";
    assert_run_with(source, &CompilerOptions::default(), &[("cv", 42i64)]);
}

#[spec_test(REQ_CB_codegen_002)]
fn end_to_end_when_standard_block_instance_then_runs_its_vm_block() {
    let source = "
PROGRAM main
VAR c : CTU; cl : CTU_LINT; cv : INT; cvl : LINT; END_VAR
c(CU := TRUE, PV := 5);
cl(CU := TRUE, PV := 5);
cv := c.CV;
cvl := cl.CV;
END_PROGRAM
";
    assert_run_with(
        source,
        &CompilerOptions::default(),
        &[("cv", 1i64), ("cvl", 1i64)],
    );
}

#[spec_test(REQ_CB_codegen_003)]
fn end_to_end_when_method_inherited_from_base_without_fields_then_runs_base_method() {
    let source = "
FUNCTION_BLOCK base
METHOD Get : DINT
Get := 7;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; r : DINT; END_VAR
r := d.Get();
END_PROGRAM
";
    assert_run_with(source, &options(), &[("r", 7i64)]);
}

#[spec_test(REQ_CB_codegen_004)]
fn end_to_end_when_method_overridden_then_each_instance_runs_its_own_block_method() {
    let source = "
FUNCTION_BLOCK base
VAR k : DINT := 7; END_VAR
METHOD Get : DINT
Get := k;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
METHOD Get : DINT
Get := 9;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; b : base; rd : DINT; rb : DINT; END_VAR
rd := d.Get();
rb := b.Get();
END_PROGRAM
";
    assert_run_with(source, &options(), &[("rd", 9i64), ("rb", 7i64)]);
}

#[spec_test(REQ_CB_codegen_005)]
fn compile_when_method_inherited_from_base_with_fields_then_not_implemented() {
    let source = "
FUNCTION_BLOCK base
VAR k : DINT := 7; END_VAR
METHOD Get : DINT
Get := k;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK derived EXTENDS base
END_FUNCTION_BLOCK

PROGRAM main
VAR d : derived; r : DINT; END_VAR
r := d.Get();
END_PROGRAM
";
    let diagnostic = try_parse_and_compile(source, &options()).unwrap_err();

    assert_eq!(diagnostic.code, "P9999");
    let start = source.find("d.Get()").unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + "d.Get()".len());
}

/// A block of two inputs that joins them as tens and ones.
const PAIR: &str = "
FUNCTION_BLOCK Pair
VAR_INPUT tens : DINT; ones : DINT; END_VAR
VAR_OUTPUT joined : DINT; positive : BOOL; END_VAR
joined := tens * 10 + ones;
positive := joined > 0;
END_FUNCTION_BLOCK
";

#[spec_test(REQ_CB_codegen_020)]
fn end_to_end_when_function_block_called_positionally_then_inputs_in_declaration_order() {
    let source = format!(
        "{PAIR}
PROGRAM main
VAR p : Pair; r : DINT; END_VAR
p(1, 2);
r := p.joined;
END_PROGRAM"
    );
    assert_run_with(&source, &CompilerOptions::default(), &[("r", 12i64)]);
}

#[spec_test(REQ_CB_codegen_021)]
fn end_to_end_when_standard_block_called_positionally_then_inputs_stored() {
    let source = "
PROGRAM main
VAR c : CTU; cv : INT; pv : INT; q : BOOL; END_VAR
c(TRUE, FALSE, 1);
cv := c.CV;
pv := c.PV;
q := c.Q;
END_PROGRAM
";
    assert_run_with(
        source,
        &CompilerOptions::default(),
        &[("cv", 1i64), ("pv", 1i64), ("q", 1i64)],
    );
}

#[spec_test(REQ_CB_codegen_022)]
fn end_to_end_when_output_negated_then_stores_its_inverse() {
    // One call cannot read an output twice (P4024), so two calls read it.
    let source = format!(
        "{PAIR}
PROGRAM main
VAR p : Pair; pos : BOOL; neg : BOOL; END_VAR
p(tens := 1, ones := 2, positive => pos);
p(tens := 1, ones := 2, NOT positive => neg);
END_PROGRAM"
    );
    assert_run_with(
        &source,
        &CompilerOptions::default(),
        &[("pos", 1i64), ("neg", 0i64)],
    );
}

#[spec_test(REQ_CB_codegen_040)]
fn end_to_end_when_named_inputs_have_side_effects_then_evaluated_in_declaration_order() {
    let source = format!(
        "{PAIR}
FUNCTION trace : DINT
VAR_IN_OUT log : DINT; END_VAR
VAR_INPUT id : DINT; END_VAR
log := log * 10 + id;
trace := id;
END_FUNCTION

PROGRAM main
VAR p : Pair; log : DINT; r : DINT; END_VAR
p(ones := trace(log, 2), tens := trace(log, 1), joined => r);
END_PROGRAM"
    );
    assert_run_with(
        &source,
        &CompilerOptions::default(),
        &[("log", 12i64), ("r", 12i64)],
    );
}

#[spec_test(REQ_CB_codegen_050)]
fn end_to_end_when_input_omitted_then_keeps_value_of_previous_call() {
    let source = format!(
        "{PAIR}
PROGRAM main
VAR p : Pair; r : DINT; END_VAR
p(tens := 1, ones := 2);
p(ones := 5);
r := p.joined;
END_PROGRAM"
    );
    assert_run_with(&source, &CompilerOptions::default(), &[("r", 15i64)]);
}

/// A block whose method joins its two inputs as tens and ones.
const JOINER: &str = "
FUNCTION_BLOCK Joiner
METHOD Join : DINT
VAR_INPUT tens : DINT; ones : DINT; END_VAR
Join := tens * 10 + ones;
END_METHOD
END_FUNCTION_BLOCK
";

#[spec_test(REQ_CB_codegen_023)]
fn end_to_end_when_method_called_positionally_then_inputs_in_declaration_order() {
    let source = format!(
        "{JOINER}
PROGRAM main
VAR j : Joiner; r : DINT; END_VAR
r := j.Join(1, 2);
END_PROGRAM"
    );
    assert_run_with(&source, &options(), &[("r", 12i64)]);
}

#[spec_test(REQ_CB_codegen_041)]
fn end_to_end_when_method_named_inputs_have_side_effects_then_evaluated_in_declaration_order() {
    let source = format!(
        "{JOINER}
FUNCTION trace : DINT
VAR_IN_OUT log : DINT; END_VAR
VAR_INPUT id : DINT; END_VAR
log := log * 10 + id;
trace := id;
END_FUNCTION

PROGRAM main
VAR j : Joiner; log : DINT; r : DINT; END_VAR
r := j.Join(ones := trace(log, 2), tens := trace(log, 1));
END_PROGRAM"
    );
    assert_run_with(&source, &options(), &[("log", 12i64), ("r", 12i64)]);
}

#[spec_test(REQ_CB_codegen_024)]
fn compile_when_method_binds_in_out_then_not_implemented() {
    let source = "
FUNCTION_BLOCK Counter
METHOD Inc
VAR_IN_OUT v : DINT; END_VAR
v := v + 1;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR inst : Counter; counter : DINT := 5; END_VAR
inst.Inc(v := counter);
END_PROGRAM
";
    let diagnostic = try_parse_and_compile(source, &options()).unwrap_err();

    assert_eq!(diagnostic.code, "P9999");
    let start = source.find("v := counter").unwrap();
    assert_eq!(diagnostic.primary.location.start, start);
    assert_eq!(diagnostic.primary.location.end, start + "v".len());
}

/// A function that joins its two inputs as tens and ones, and a function that
/// appends the digit `id` to `log` and returns it.
const FUNCTIONS: &str = "
FUNCTION join : DINT
VAR_INPUT tens : DINT; ones : DINT; END_VAR
join := tens * 10 + ones;
END_FUNCTION

FUNCTION trace : DINT
VAR_IN_OUT log : DINT; END_VAR
VAR_INPUT id : DINT; END_VAR
log := log * 10 + id;
trace := id;
END_FUNCTION
";

#[spec_test(REQ_CB_codegen_025)]
fn end_to_end_when_function_named_then_computes_as_positional_in_declaration_order() {
    let source = format!(
        "{FUNCTIONS}
PROGRAM main
VAR x : DINT := 2; y : DINT := 1; named : DINT; positional : DINT; END_VAR
named := join(ones := x, tens := y);
positional := join(y, x);
END_PROGRAM"
    );
    assert_run_with(
        &source,
        &CompilerOptions::default(),
        &[("named", 12i64), ("positional", 12i64)],
    );
}

#[spec_test(REQ_CB_codegen_042)]
fn end_to_end_when_function_named_inputs_have_side_effects_then_evaluated_in_declaration_order() {
    let source = format!(
        "{FUNCTIONS}
PROGRAM main
VAR log : DINT; r : DINT; END_VAR
r := join(ones := trace(log, 2), tens := trace(log, 1));
END_PROGRAM"
    );
    assert_run_with(
        &source,
        &CompilerOptions::default(),
        &[("log", 12i64), ("r", 12i64)],
    );
}

/// Writes every named input of every function call as a positional input,
/// as a call the analyzer did not bind would be.
struct UnnameInputs;

impl Fold<()> for UnnameInputs {
    fn fold_param_assignment_kind(
        &mut self,
        node: ParamAssignmentKind,
    ) -> Result<ParamAssignmentKind, ()> {
        match node.recurse_fold(self)? {
            ParamAssignmentKind::NamedInput(input) => {
                Ok(ParamAssignmentKind::PositionalInput(PositionalInput {
                    expr: input.expr,
                }))
            }
            other => Ok(other),
        }
    }
}

#[spec_test(REQ_CB_codegen_026)]
fn compile_when_user_function_argument_positional_then_internal_error() {
    let source = format!(
        "{FUNCTIONS}
PROGRAM main
VAR r : DINT; END_VAR
r := join(1, 2);
END_PROGRAM"
    );
    let options = CompilerOptions::default();
    let (library, context) = parse(&source, &options);
    let library = UnnameInputs.fold_library(library).unwrap();

    let diagnostic = compile_by_route(&library, &context, &options, Route::Ast).unwrap_err();

    assert_eq!(diagnostic.code, "P9998");
}
