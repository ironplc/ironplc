//! Statement and control flow compilation for IEC 61131-3 code generation.
//!
//! Contains statement dispatch, control flow (IF, CASE), and function block
//! call compilation; the loops (FOR, WHILE, REPEAT) are in `compile_loop`.
//! Separated from compile.rs to keep module sizes within the 1000-line
//! guideline.

use ironplc_dsl::common::{
    BitStringLiteral, FunctionBlockBodyKind, Integer, IntegerRef, SignedInteger, SignedIntegerRef,
    StringInitializer, StringSpecification,
};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CaseSelectionKind, Expr, FbCall, ParamAssignmentKind, Statements, StmtKind,
};

use super::compile::{
    CompileContext, CurrentFunctionReturn, OpType, OpWidth, Signedness, DEFAULT_STRING_MAX_LENGTH,
};
use super::compile_assign::compile_assignment;
use super::compile_call_output::{store_outputs, CallOutputs};
use super::compile_expr::{
    compile_condition, compile_expr, emit_classified_cmp_br, emit_eq, emit_ge, emit_le,
    emit_load_var, op_type, try_classify_cmp,
};
use super::compile_fb_call::FbCallSite;
use super::compile_loop::{compile_for, compile_repeat, compile_while};
use super::compile_method::compile_method_call_statement;
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

/// Compiles a sequence of statements.
pub(crate) fn compile_statements(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    statements: &Statements,
) -> Result<(), Diagnostic> {
    for stmt in &statements.body {
        // A top-level statement takes the lowered route where the route asks
        // for it and lowering supports it.
        if !crate::lowered::try_compile_statement(emitter, ctx, stmt)? {
            compile_statement(emitter, ctx, stmt)?;
        }
    }
    Ok(())
}

/// Records the statement's source position on the emitter so the
/// subsequent opcode(s) get a line-map entry pointing at this
/// statement.
///
/// Looks up the statement's `FileId` in the SOURCE_FILE_TABLE
/// registry built at the start of compilation. If the file isn't
/// registered (synthetic ASTs, unknown spans) or the registry has no
/// source bytes for it (the [`crate::EmptyLookup`] case), no entry is
/// recorded — there's no useful (line, column) to surface, and a
/// `file_id` without a position would only confuse a debugger.
pub(crate) fn record_statement_position(
    emitter: &mut crate::emit::Emitter,
    ctx: &CompileContext,
    stmt: &StmtKind,
) {
    record_position(emitter, ctx, &stmt.span());
}

/// Records `span` as the source position of the opcodes that follow, as
/// [`record_statement_position`] does for a statement.
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

/// Compiles a single statement.
fn compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &StmtKind,
) -> Result<(), Diagnostic> {
    record_statement_position(emitter, ctx, stmt);
    match stmt {
        StmtKind::Assignment(assignment) => compile_assignment(emitter, ctx, assignment),
        StmtKind::FbCall(fb_call) => compile_fb_call(emitter, ctx, fb_call),
        StmtKind::MethodCall(method_call) => {
            compile_method_call_statement(emitter, ctx, method_call)
        }
        StmtKind::If(if_stmt) => compile_if(emitter, ctx, if_stmt),
        StmtKind::Case(case_stmt) => compile_case(emitter, ctx, case_stmt),
        StmtKind::For(for_stmt) => compile_for(emitter, ctx, for_stmt),
        StmtKind::While(while_stmt) => compile_while(emitter, ctx, while_stmt),
        StmtKind::Repeat(repeat_stmt) => compile_repeat(emitter, ctx, repeat_stmt),
        StmtKind::Return => {
            emit_return(emitter, ctx);
            Ok(())
        }
        // Analysis rejects an EXIT or CONTINUE outside a loop
        // (`rule_loop_control_inside_loop`, P4021 and P4065).
        StmtKind::Exit(span) => {
            let label = ctx.current_loop_exit().ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(span.clone(), "EXIT outside a loop"))
            })?;
            emitter.emit_jmp(label);
            Ok(())
        }
        StmtKind::Continue(span) => {
            let label = ctx.current_loop_next().ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(span.clone(), "CONTINUE outside a loop"))
            })?;
            emitter.emit_jmp(label);
            Ok(())
        }
    }
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

/// Compiles a function block invocation: stores inputs, calls FB, reads outputs.
///
/// The analyzer has written the call's arguments in normal form
/// (`specs/design/call-binding.md`): one named input for each input the call
/// supplies, in the order the block declares them, then its outputs in
/// declaration order. They are compiled in that order, which is the order
/// they are evaluated in. An input the call omits keeps its value.
fn compile_fb_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    fb_call: &FbCall,
) -> Result<(), Diagnostic> {
    let call = FbCallSite::open(emitter, ctx, &fb_call.var_name, &fb_call.span())?;
    for param in &fb_call.params {
        match param {
            ParamAssignmentKind::NamedInput(input) => {
                compile_expr(emitter, ctx, &input.expr)?;
                call.store_input(emitter, &input.name)?;
            }
            // The binding pass names every input of a call that binds, and
            // the rules reject one that does not.
            ParamAssignmentKind::PositionalInput(input) => {
                return Err(Diagnostic::internal_error_at(Label::span(
                    input.expr.span(),
                    "Positional input of a function block call the analyzer did not bind",
                )));
            }
            ParamAssignmentKind::Output(_) => {}
        }
    }
    call.close(emitter, ctx);

    // An output of a function block is a field of its instance.
    store_outputs(
        emitter,
        ctx,
        CallOutputs::Instance(fb_call.var_name.clone()),
        &fb_call.params,
    )
}

/// Compiles a slice of statements.
pub(crate) fn compile_stmts(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmts: &[StmtKind],
) -> Result<(), Diagnostic> {
    for stmt in stmts {
        compile_statement(emitter, ctx, stmt)?;
    }
    Ok(())
}

/// Compiles an IF/ELSIF/ELSE statement.
fn compile_if(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    if_stmt: &ironplc_dsl::textual::If,
) -> Result<(), Diagnostic> {
    let has_else_ifs = !if_stmt.else_ifs.is_empty();
    let has_else = !if_stmt.else_body.is_empty();
    let needs_end_label = has_else_ifs || has_else;

    let end_label = if needs_end_label {
        Some(emitter.create_label())
    } else {
        None
    };

    // Jump past the then-body if condition is false. When the condition is
    // a fusable `var <cmp> const`, replace LOAD_VAR + LOAD_CONST + CMP +
    // JMP_IF_NOT with a single `CMP_BR_*` using the negated comparison.
    let next_label = emitter.create_label();
    if let Some(classified) = try_classify_cmp(ctx, &if_stmt.expr) {
        emit_classified_cmp_br(emitter, classified, false, next_label)?;
    } else {
        compile_condition(emitter, ctx, &if_stmt.expr)?;
        emitter.emit_jmp_if_not(next_label);
    }

    // Compile the then-body.
    compile_stmts(emitter, ctx, &if_stmt.body)?;

    // If there are more branches, jump to end.
    if let Some(end) = end_label {
        emitter.emit_jmp(end);
    }

    emitter.bind_label(next_label);

    // Compile ELSIF clauses.
    for elsif in &if_stmt.else_ifs {
        let elsif_next = emitter.create_label();
        if let Some(classified) = try_classify_cmp(ctx, &elsif.expr) {
            emit_classified_cmp_br(emitter, classified, false, elsif_next)?;
        } else {
            compile_condition(emitter, ctx, &elsif.expr)?;
            emitter.emit_jmp_if_not(elsif_next);
        }

        compile_stmts(emitter, ctx, &elsif.body)?;

        // `end_label` exists whenever there is an ELSIF clause.
        let end = end_label.ok_or_else(Diagnostic::internal_error)?;
        emitter.emit_jmp(end);

        emitter.bind_label(elsif_next);
    }

    // Compile ELSE body (if present).
    if has_else {
        compile_stmts(emitter, ctx, &if_stmt.else_body)?;
    }

    // Bind the end label.
    if let Some(end) = end_label {
        emitter.bind_label(end);
    }

    Ok(())
}

/// Compiles a CASE statement.
///
/// Each `CaseStatementGroup` is compiled as a chain of comparisons (like
/// IF/ELSIF/ELSE). Multi-value selectors are OR'd together.
///
/// ```text
///   // For each arm:
///   compile(selector)
///   LOAD_CONST case_value
///   EQ_I32
///   JMP_IF_NOT → next_arm
///   compile(body)
///   JMP → END
/// next_arm:
///   // ... next arm / ELSE body ...
/// END:
/// ```
fn compile_case(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    case_stmt: &ironplc_dsl::textual::Case,
) -> Result<(), Diagnostic> {
    let end_label = emitter.create_label();
    // An enumeration selector operates as a DINT (REQ-EN-codegen-003), which
    // `op_type` answers from its type.
    let selector = CaseSelector {
        expr: &case_stmt.selector,
        op_type: op_type(ctx, &case_stmt.selector)?,
    };

    for group in &case_stmt.statement_groups {
        let next_label = emitter.create_label();

        // Compile selector comparisons with OR logic.
        for (i, selection) in group.selectors.iter().enumerate() {
            compile_case_selector(emitter, ctx, &selector, selection)?;
            if i > 0 {
                emitter.emit_bool_or();
            }
        }

        emitter.emit_jmp_if_not(next_label);

        // Compile body.
        compile_stmts(emitter, ctx, &group.statements)?;

        emitter.emit_jmp(end_label);

        emitter.bind_label(next_label);
    }

    // Compile ELSE body if present.
    compile_stmts(emitter, ctx, &case_stmt.else_body)?;

    emitter.bind_label(end_label);

    Ok(())
}

/// The selector of a `CASE` statement: the expression every label is
/// compared against, and the width the comparison is made at.
struct CaseSelector<'a> {
    expr: &'a Expr,
    op_type: OpType,
}

impl CaseSelector<'_> {
    /// Emits `selector <cmp> label` at the selector's width, leaving a
    /// boolean result on the stack.
    ///
    /// The width is decided here, once, for every label kind. A `CASE`
    /// compares its selector against integer labels, so only an integer
    /// width is meaningful; a float-width selector is rejected against the
    /// selector expression.
    fn cmp_label(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        label: CaseLabelValue<'_>,
        cmp: fn(&mut Emitter, OpType),
    ) -> Result<(), Diagnostic> {
        compile_expr(emitter, ctx, self.expr)?;
        match self.op_type.0 {
            OpWidth::W32 => {
                let pool_index = ctx.add_i32_constant(label.to_i32(self.op_type.1)?);
                emitter.emit_load_const_i32(pool_index);
            }
            OpWidth::W64 => {
                let pool_index = ctx.add_i64_constant(label.to_i64(self.op_type.1)?);
                emitter.emit_load_const_i64(pool_index);
            }
            // CASE with float types is not meaningful in IEC 61131-3.
            OpWidth::F32 | OpWidth::F64 => {
                return Err(non_integer_case_selector(self.expr));
            }
        }
        cmp(emitter, self.op_type);
        Ok(())
    }
}

/// A `CASE` label's value, whatever radix it was written in.
///
/// A label is a value, not a bit pattern: `16#FFFFFFFF` and `4294967295` are
/// the same label, and both narrow to the selector's width by the value they
/// state. Analysis rejects a label outside the selector's type
/// (`rule_constant_range`, P2026), including the type it records for an
/// untyped literal selector (`CASE 5 OF ...`), and the selector's storage is
/// at least that wide. So every label fits by the time it arrives here, and
/// one that does not is a broken invariant, reported as an internal error.
struct CaseLabelValue<'a> {
    is_neg: bool,
    magnitude: &'a Integer,
}

impl<'a> CaseLabelValue<'a> {
    fn signed(value: &'a SignedInteger) -> Self {
        Self {
            is_neg: value.is_neg,
            magnitude: &value.value,
        }
    }

    fn radix(literal: &'a BitStringLiteral) -> Self {
        Self {
            is_neg: false,
            magnitude: &literal.value,
        }
    }

    /// The label's value at the selector's 32-bit width. An unsigned
    /// selector holds its value's bit pattern in the slot, so a label for one
    /// is stored the same way.
    fn to_i32(&self, signedness: Signedness) -> Result<i32, Diagnostic> {
        let value = self.value()?;
        match signedness {
            Signedness::Signed => i32::try_from(value).ok(),
            Signedness::Unsigned => u32::try_from(value).ok().map(|value| value as i32),
        }
        .ok_or_else(|| self.overflow())
    }

    /// The label's value at the selector's 64-bit width, stored the way
    /// [`Self::to_i32`] stores it.
    fn to_i64(&self, signedness: Signedness) -> Result<i64, Diagnostic> {
        let value = self.value()?;
        match signedness {
            Signedness::Signed => i64::try_from(value).ok(),
            Signedness::Unsigned => u64::try_from(value).ok().map(|value| value as i64),
        }
        .ok_or_else(|| self.overflow())
    }

    fn value(&self) -> Result<i128, Diagnostic> {
        let magnitude = i128::try_from(self.magnitude.value).map_err(|_| self.overflow())?;
        Ok(if self.is_neg { -magnitude } else { magnitude })
    }

    fn overflow(&self) -> Diagnostic {
        let sign = if self.is_neg { "-" } else { "" };
        Diagnostic::internal_error_at(Label::span(
            self.magnitude.span(),
            format!(
                "CASE label {sign}{} does not fit the selector's storage",
                self.magnitude.value
            ),
        ))
    }
}

/// Compiles a single case selector, leaving a boolean result on the stack.
///
/// - `SignedInteger`: `selector == value`
/// - `Subrange`: `(selector >= start) AND (selector <= end)`
/// - `EnumeratedValue`: `selector == ordinal` (REQ-EN-codegen-040)
/// - `BitStringLiteral`: `selector == value` (same shape as `SignedInteger`)
fn compile_case_selector(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    selector: &CaseSelector,
    selection: &CaseSelectionKind,
) -> Result<(), Diagnostic> {
    match selection {
        CaseSelectionKind::SignedInteger(si) => {
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(si), emit_eq)
        }
        CaseSelectionKind::Subrange(sr) => {
            let start = resolve_signed_integer_ref(&sr.start)?;
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(start), emit_ge)?;
            let end = resolve_signed_integer_ref(&sr.end)?;
            selector.cmp_label(emitter, ctx, CaseLabelValue::signed(end), emit_le)?;
            emitter.emit_bool_and();
            Ok(())
        }
        CaseSelectionKind::EnumeratedValue(ev) => {
            // REQ-EN-codegen-040: Load selector, load ordinal constant, compare with EQ_I32.
            // The label is a value of the selector's type.
            compile_expr(emitter, ctx, selector.expr)?;
            let members = crate::compile_enum::members_of_expr(ctx, selector.expr);
            let ordinal = crate::compile_enum::ordinal_in(members, ev)?;
            let pool_index = ctx.add_i32_constant(ordinal);
            emitter.emit_load_const_i32(pool_index);
            emitter.emit_eq_i32();
            Ok(())
        }
        CaseSelectionKind::BitStringLiteral(lit) => {
            selector.cmp_label(emitter, ctx, CaseLabelValue::radix(lit), emit_eq)
        }
    }
}

/// Builds the internal error for a `CASE` whose selector is not an integer
/// type, pointing at the selector expression.
///
/// Analysis rejects such a selector (P4053) before codegen runs, so reaching
/// this is a broken invariant rather than a missing capability.
#[track_caller]
fn non_integer_case_selector(selector_expr: &Expr) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        selector_expr.span(),
        "CASE selector is not an integer type",
    ))
}

/// Extracts the max length from a `StringInitializer`, returning a
/// not-implemented diagnostic if the length is an unresolved constant reference.
pub(crate) fn resolve_string_max_length(
    string_init: &StringInitializer,
) -> Result<u16, Diagnostic> {
    match &string_init.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH),
        Some(IntegerRef::Literal(i)) => string_length_u16(i),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id)),
    }
}

/// Extracts the max length from a `StringSpecification` (used for function
/// return types), returning a not-implemented diagnostic if the length is an
/// unresolved constant reference.
pub(crate) fn resolve_string_spec_max_length(
    spec: &StringSpecification,
) -> Result<u16, Diagnostic> {
    match &spec.length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH),
        Some(IntegerRef::Literal(i)) => string_length_u16(i),
        Some(IntegerRef::Constant(id)) => Err(Diagnostic::todo_with_id(id)),
    }
}

/// Converts a declared string length to the `u16` the string header holds.
///
/// The analyzer rejects a length above `u16::MAX` (P2041) before codegen
/// runs, so a failure here is a compiler defect -- the rule missed a
/// declaration site -- and is reported as one rather than wrapped to a
/// capacity the program never wrote.
fn string_length_u16(length: &Integer) -> Result<u16, Diagnostic> {
    u16::try_from(length.value).map_err(|_| {
        Diagnostic::internal_error_at(Label::span(
            length.span.clone(),
            format!(
                "String length {} was not rejected by the analyzer",
                length.value
            ),
        ))
    })
}

/// Extracts a concrete `SignedInteger` from a `SignedIntegerRef`, returning a
/// not-implemented diagnostic if it is an unresolved constant reference.
fn resolve_signed_integer_ref(sir: &SignedIntegerRef) -> Result<&SignedInteger, Diagnostic> {
    match sir {
        SignedIntegerRef::Literal(si) => Ok(si),
        SignedIntegerRef::Constant(id) => Err(Diagnostic::todo_with_id(id)),
    }
}
