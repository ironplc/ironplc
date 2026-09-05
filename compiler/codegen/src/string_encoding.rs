//! Compile-time enforcement of the string-encoding invariant.
//!
//! A `STRING` and a `WSTRING` are the same shape in the data region and differ
//! only in the `char_width` each slot declares, so bytecode that points one
//! string operation at slots of both widths is well-formed in every respect
//! except the one that matters. The VM catches it — every string opcode checks
//! the widths and traps with `V9014` — but only once the program is running,
//! on a machine that is not the one that built it, with two numbers and no
//! instruction to point at.
//!
//! This module is the choke point that stops such bytecode from reaching a
//! container. The analysis itself lives in
//! [`ironplc_container::verify_string`], beside the opcode tables whose
//! operand layouts it reads; this module supplies the codegen-facing entry
//! point and turns a violation into a compiler diagnostic.
//!
//! It is the third of three layers, and the last: the analyzer rejects a
//! program that mixes declared encodings (P4034 -- `rule_string_encoding_compat`),
//! [`crate::string_width`] resolves the one encoding each operation's operands
//! share and produces every operand at it, and this pass reads back what was
//! actually emitted.

use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

/// Verifies string-encoding agreement across every function in `container`.
///
/// A violation is reported as [`Problem::InternalError`] (P9998), not as a
/// user-facing program error: an encoding mismatch is not something a program
/// can ask for. A program that mixes `STRING` and `WSTRING` is rejected before
/// this point with P4034, so bytecode that reaches here disagreeing with
/// itself means codegen emitted it wrong.
///
/// [`Problem::InternalError`]: ironplc_problems::Problem::InternalError
pub(crate) fn verify_container(container: &Container) -> Result<(), Diagnostic> {
    ironplc_container::verify_string_encoding(container).map_err(|violation| {
        Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            format!(
                "codegen emitted a string operation whose operands disagree on encoding \
                 (verifier rule {}): {violation}",
                violation.rule()
            ),
        ))
    })
}

#[cfg(test)]
mod tests {
    use ironplc_container::{opcode, CharWidth, ContainerBuilder, FunctionId};

    use crate::emit::Emitter;

    /// Wraps an emitter's finished bytecode in a container, the same sequence
    /// `compile_program_with_functions` performs, reduced to one function --
    /// so what the verifier sees is what codegen would have shipped.
    fn container_from(emitter: &mut Emitter) -> ironplc_container::Container {
        let max_stack_depth = {
            let _ = emitter.bytecode();
            emitter.max_stack_depth()
        };
        let bytecode = emitter.bytecode().to_vec();
        ContainerBuilder::new()
            .num_variables(1)
            .max_call_depth(1)
            .data_region_bytes(64)
            .add_str_constant(b"abc")
            .add_i32_constant(0)
            .add_i32_constant(32)
            .add_function(FunctionId::INIT, &bytecode, max_stack_depth, 1, 0)
            .build()
    }

    #[test]
    fn verify_container_when_comparison_mixes_encodings_then_internal_error() {
        // Issue #1550's bytecode: a WSTRING variable's slot compared against a
        // scratch slot codegen initialized narrow because the operand was a
        // literal, which the VM could only report as a V9014 trap mid-run.
        let mut emitter = Emitter::new();
        emitter.emit_str_init(0, 4, CharWidth::Wide);
        emitter.emit_str_init(32, 4, CharWidth::Narrow);
        emitter.emit_load_const_str(0);
        emitter.emit_str_store_var(32);
        emitter.emit_load_const_i32(1);
        emitter.emit_load_const_i32(2);
        emitter.emit_builtin(opcode::builtin::CMP_STR);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter);
        let diagnostic = super::verify_container(&container).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(diagnostic.primary.message.contains("R0304"));
        assert!(diagnostic.primary.message.contains("disagree on encoding"));
    }

    #[test]
    fn verify_container_when_narrow_literal_stored_into_wide_slot_then_internal_error() {
        let mut emitter = Emitter::new();
        emitter.emit_str_init(0, 4, CharWidth::Wide);
        emitter.emit_load_const_str(0);
        emitter.emit_str_store_var(0);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter);
        let diagnostic = super::verify_container(&container).unwrap_err();

        assert_eq!(diagnostic.code, "P9998");
        assert!(diagnostic.primary.message.contains("R0304"));
    }

    #[test]
    fn verify_container_when_comparison_operands_share_encoding_then_ok() {
        let mut emitter = Emitter::new();
        emitter.emit_str_init(0, 4, CharWidth::Wide);
        emitter.emit_str_init(32, 4, CharWidth::Wide);
        emitter.emit_load_const_i32(1);
        emitter.emit_load_const_i32(2);
        emitter.emit_builtin(opcode::builtin::CMP_STR);
        emitter.emit_ret_void();

        let container = container_from(&mut emitter);
        assert!(super::verify_container(&container).is_ok());
    }
}
