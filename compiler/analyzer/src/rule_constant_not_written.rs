//! Semantic rule that a variable declared `CONSTANT` is not written.
//!
//! A write is found and resolved to its declaration by the same collector
//! that decides which variables are never written (see
//! `specs/design/constant-variable-inference.md`). The rule reports the
//! statements that write: an assignment, a `FOR` control variable, an output
//! binding, and an argument bound to a `VAR_IN_OUT` parameter. Taking the
//! address with `REF` or `ADR` is not a write by itself, and a write the
//! collector cannot resolve to one declaration is not reported, so the rule
//! never rejects a program that leaves its constants alone.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR CONSTANT
//!     k : INT := 1;
//! END_VAR
//! VAR
//!     x : INT;
//! END_VAR
//!     x := k;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR CONSTANT
//!     k : INT := 1;
//! END_VAR
//!     k := 2;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    semantic_context::SemanticContext,
    write_collector::{collect, WriteKind},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let writes = collect(lib, &context.types, &context.functions, &context.symbols).written;

    // The qualifier that counts is the one of the declaration the write
    // reaches: a write through a plain `VAR_EXTERNAL` is allowed even when
    // another unit declares the same global `VAR_EXTERNAL CONSTANT`.
    let diagnostics: Vec<Diagnostic> = writes
        .sites
        .iter()
        .filter(|site| is_statement_write(site.kind))
        .filter_map(|site| {
            let declared = context.symbols.find(&site.name, &site.declaration)?;
            declared.is_constant().then(|| {
                Diagnostic::problem(
                    Problem::ConstantVariableWritten,
                    Label::span(site.name.span(), "Write to a constant"),
                )
                .with_context_id("variable", &site.name)
                .with_secondary(Label::span(declared.span.clone(), "Declared CONSTANT"))
            })
        })
        .collect();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Whether a write of this kind is a statement that changes the variable.
fn is_statement_write(kind: WriteKind) -> bool {
    match kind {
        WriteKind::Assignment
        | WriteKind::ForControl
        | WriteKind::OutputBinding
        | WriteKind::InOutArgument => true,
        WriteKind::UnboundArgument
        | WriteKind::AddressTaken
        | WriteKind::Invocation
        | WriteKind::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;

    rule_err!(
        apply_when_assign_to_var_constant_then_error,
        "
PROGRAM main
VAR CONSTANT
    k : INT := 1;
END_VAR
    k := 2;
END_PROGRAM",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_assign_to_element_of_constant_array_then_error,
        "
PROGRAM main
VAR CONSTANT
    k : ARRAY[1..2] OF INT := [1, 2];
END_VAR
    k[1] := 5;
END_PROGRAM",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_constant_is_for_control_then_error,
        "
FUNCTION f : INT
VAR CONSTANT
    i : INT := 0;
END_VAR
    FOR i := 1 TO 3 DO
        f := i;
    END_FOR;
END_FUNCTION",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_constant_is_output_binding_target_then_error,
        "
FUNCTION_BLOCK counter
VAR_OUTPUT
    q : INT;
END_VAR
    q := 1;
END_FUNCTION_BLOCK

PROGRAM main
VAR CONSTANT
    k : INT := 1;
END_VAR
VAR
    c : counter;
END_VAR
    c(q => k);
END_PROGRAM",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_constant_bound_to_in_out_of_function_block_then_error,
        "
FUNCTION_BLOCK bump
VAR_IN_OUT
    v : INT;
END_VAR
    v := v + 1;
END_FUNCTION_BLOCK

PROGRAM main
VAR CONSTANT
    k : INT := 1;
END_VAR
VAR
    b : bump;
END_VAR
    b(v := k);
END_PROGRAM",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_assign_to_external_constant_then_error,
        "
PROGRAM main
VAR_EXTERNAL CONSTANT
    limit : INT;
END_VAR
    limit := 5;
END_PROGRAM

CONFIGURATION config
    VAR_GLOBAL CONSTANT
        limit : INT := 10;
    END_VAR
    RESOURCE res ON PLC
        TASK t(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM inst WITH t : main;
    END_RESOURCE
END_CONFIGURATION",
        [Problem::ConstantVariableWritten]
    );

    rule_err!(
        apply_when_assign_through_external_constant_to_non_constant_global_then_error,
        "
PROGRAM main
VAR_EXTERNAL CONSTANT
    g : INT;
END_VAR
    g := 5;
END_PROGRAM

CONFIGURATION config
    VAR_GLOBAL
        g : INT := 10;
    END_VAR
    RESOURCE res ON PLC
        TASK t(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM inst WITH t : main;
    END_RESOURCE
END_CONFIGURATION",
        [Problem::ConstantVariableWritten]
    );

    rule_ok!(
        apply_when_assign_through_external_while_other_pou_declares_it_external_constant_then_ok,
        "
PROGRAM reader
VAR_EXTERNAL CONSTANT
    g : INT;
END_VAR
VAR
    x : INT;
END_VAR
    x := g;
END_PROGRAM

PROGRAM writer
VAR_EXTERNAL
    g : INT;
END_VAR
    g := 5;
END_PROGRAM

CONFIGURATION config
    VAR_GLOBAL
        g : INT := 10;
    END_VAR
    RESOURCE res ON PLC
        TASK t(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM r WITH t : reader;
        PROGRAM w WITH t : writer;
    END_RESOURCE
END_CONFIGURATION"
    );

    rule_ok!(
        apply_when_constant_only_read_then_ok,
        "
PROGRAM main
VAR CONSTANT
    k : INT := 1;
END_VAR
VAR
    x : INT;
END_VAR
    x := k + 1;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_same_name_is_constant_in_another_pou_then_ok,
        "
FUNCTION f : INT
VAR CONSTANT
    k : INT := 1;
END_VAR
    f := k;
END_FUNCTION

PROGRAM main
VAR
    k : INT;
END_VAR
    k := f();
END_PROGRAM"
    );

    rule_ok!(
        apply_when_constant_passed_to_input_then_ok,
        "
FUNCTION_BLOCK show
VAR_INPUT
    v : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR CONSTANT
    k : INT := 1;
END_VAR
VAR
    s : show;
END_VAR
    s(v := k);
END_PROGRAM"
    );
}
