//! A body's statements, each compiled from the lowered program
//! ([`crate::lowered`]), the source position each records for the line map,
//! a `RETURN`, and the capacity a string declaration states.

use ironplc_dsl::common::FunctionBlockBodyKind;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Statements;

use super::compile::{CompileContext, CurrentFunctionReturn};
use super::compile_expr::emit_load_var;
use crate::emit::Emitter;

/// Compiles a function block body.
///
/// `pou_span` locates the program organization unit that owns the body. An
/// SFC body carries no span of its own, so a diagnostic about the body kind
/// points at the POU instead.
pub(crate) fn compile_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &FunctionBlockBodyKind,
    pou_span: &SourceSpan,
) -> Result<(), Diagnostic> {
    match body {
        FunctionBlockBodyKind::Statements(statements) => {
            compile_statements(emitter, ctx, statements)
        }
        FunctionBlockBodyKind::Empty => Ok(()),
        FunctionBlockBodyKind::Sfc(_) => Err(Diagnostic::not_implemented(Label::span(
            pou_span.clone(),
            "Sequential function chart body",
        ))),
    }
}

/// Compiles the top-level statements of the current body.
pub(crate) fn compile_statements(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    statements: &Statements,
) -> Result<(), Diagnostic> {
    for (index, stmt) in statements.body.iter().enumerate() {
        crate::lowered::compile_statement(emitter, ctx, index, stmt)?;
    }
    Ok(())
}

/// Records `span` as the source position of the opcodes that follow, so
/// they get a line-map entry pointing at the statement there.
///
/// Looks up the span's `FileId` in the SOURCE_FILE_TABLE registry built at
/// the start of compilation. If the file isn't registered (synthetic ASTs,
/// unknown spans) or the registry has no source bytes for it (the
/// [`crate::EmptyLookup`] case), no entry is recorded — there's no useful
/// (line, column) to surface, and a `file_id` without a position would only
/// confuse a debugger.
pub(crate) fn record_position(
    emitter: &mut crate::emit::Emitter,
    ctx: &CompileContext,
    span: &SourceSpan,
) {
    let Some(file_id) = ctx.debug_source_files.get(&span.file_id) else {
        return;
    };
    let Some(bytes) = ctx.debug_source_files.source_bytes(&span.file_id) else {
        return;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let lc = ironplc_dsl::diagnostic::LineColumn::from_offset(text, span.start);
    // 0-based → 1-based; clamp to u16 range (source files larger than
    // ~64k lines are theoretical, but the saturating add keeps us out
    // of UB territory either way).
    let line = u16::try_from(lc.line.saturating_add(1)).unwrap_or(u16::MAX);
    let column = u16::try_from(lc.column.saturating_add(1)).unwrap_or(u16::MAX);
    emitter.set_source_position(
        file_id,
        ironplc_container::SourceLine::new(line),
        ironplc_container::SourceColumn::new(column),
    );
}

/// Emits a `RETURN` from the current body.
///
/// Inside a function with a return value, an early RETURN must first push
/// the current return-value variable so the caller sees a value on the stack
/// (matching the trailing load+RET emitted at the end of the function body).
pub(crate) fn emit_return(emitter: &mut Emitter, ctx: &CompileContext) {
    match ctx.current_function_return {
        Some(CurrentFunctionReturn::Scalar { var_index, op_type }) => {
            emit_load_var(emitter, var_index, op_type);
            emitter.emit_ret();
        }
        Some(CurrentFunctionReturn::String { data_offset }) => {
            emitter.emit_str_load_var(data_offset);
            emitter.emit_ret();
        }
        None => {
            emitter.emit_ret_void();
        }
    }
}
