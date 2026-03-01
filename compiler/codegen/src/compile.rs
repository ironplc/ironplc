//! Compiles an IEC 61131-3 AST into a bytecode container.
//!
//! This module walks the AST produced by the parser/analyzer and generates
//! bytecode that the IronPLC VM can execute.
//!
//! # Supported constructs
//!
//! - PROGRAM declarations with all 8 IEC 61131-3 integer types
//!   (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT)
//! - Assignment statements with truncation for narrow types
//! - Integer literal constants (i32 and i64)
//! - Boolean literal constants (TRUE, FALSE)
//! - Binary Add, Sub, Mul, Div, Mod, and Pow operators
//! - Unary Neg and Not operators
//! - Comparison operators (=, <>, <, <=, >, >=)
//! - Boolean operators (AND, OR, XOR, NOT)
//! - Variable references (named symbolic variables)
//! - IF/ELSIF/ELSE statements
//! - CASE statements (integer and subrange selectors)
//! - WHILE, FOR, and REPEAT iteration statements
//! - EXIT (break from innermost loop) and RETURN (early program exit)
//!
//! # Integer type strategy: promote-operate-truncate
//!
//! Two native operation widths: **i32** (for ≤32-bit types) and **i64**
//! (for 64-bit types). Variables are loaded/stored at native width.
//! After arithmetic at native width, narrow types (SINT, INT, USINT, UINT)
//! are truncated back to their declared range before storing.

use std::collections::HashMap;

use ironplc_container::{opcode, Container, ContainerBuilder};
use ironplc_dsl::common::{
    Boolean, ConstantKind, FunctionBlockBodyKind, InitialValueAssignmentKind, Library,
    LibraryElementKind, ProgramDeclaration, SignedInteger, VarDecl,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CaseSelectionKind, CompareOp, ExprKind, Operator, Statements, StmtKind, SymbolicVariableKind,
    UnaryOp, Variable,
};
use ironplc_problems::Problem;

use crate::emit::Emitter;

/// The native operation width used for arithmetic and comparisons.
#[derive(Clone, Copy, PartialEq)]
enum OpWidth {
    /// 32-bit operations (for SINT, INT, DINT, USINT, UINT, UDINT).
    W32,
    /// 64-bit operations (for LINT, ULINT).
    W64,
}

/// Whether a type uses signed or unsigned semantics for division and comparison.
#[derive(Clone, Copy, PartialEq)]
enum Signedness {
    Signed,
    Unsigned,
}

/// Type information for a variable, used to select the correct opcodes.
#[derive(Clone, Copy)]
struct VarTypeInfo {
    /// The native operation width (i32 or i64).
    op_width: OpWidth,
    /// Whether signed or unsigned opcodes are used for division/comparison.
    signedness: Signedness,
    /// The declared storage width in bits (8, 16, 32, or 64).
    storage_bits: u8,
}

/// Shorthand for the operation type tuple used during expression compilation.
type OpType = (OpWidth, Signedness);

/// The default operation type: 32-bit signed (used for pure-constant expressions).
const DEFAULT_OP_TYPE: OpType = (OpWidth::W32, Signedness::Signed);

/// A constant in the pool, either 32-bit or 64-bit.
enum PoolConstant {
    I32(i32),
    I64(i64),
}

/// Compiles a library into a bytecode container.
///
/// Finds the first PROGRAM declaration in the library and compiles it
/// into a container suitable for execution by the VM.
///
/// Returns an error if no program is found or if the program contains
/// unsupported constructs.
pub fn compile(library: &Library) -> Result<Container, Diagnostic> {
    let program = find_program(library)?;
    compile_program(program)
}

/// Finds the first PROGRAM declaration in the library.
fn find_program(library: &Library) -> Result<&ProgramDeclaration, Diagnostic> {
    for element in &library.elements {
        if let LibraryElementKind::ProgramDeclaration(program) = element {
            return Ok(program);
        }
    }
    Err(Diagnostic::todo(file!(), line!()))
}

/// Compiles a single PROGRAM into a container.
fn compile_program(program: &ProgramDeclaration) -> Result<Container, Diagnostic> {
    let mut ctx = CompileContext::new();

    // Assign variable indices for all declared variables.
    assign_variables(&mut ctx, &program.variables)?;

    // Compile the program body.
    let mut emitter = Emitter::new();
    compile_body(&mut emitter, &mut ctx, &program.body)?;
    emitter.emit_ret_void();

    // Build the container.
    let num_variables = ctx.variables.len() as u16;
    let num_locals = num_variables;
    let max_stack_depth = emitter.max_stack_depth();
    let bytecode = emitter.bytecode();

    let mut builder = ContainerBuilder::new().num_variables(num_variables);

    // Add constants to the pool.
    for constant in &ctx.constants {
        match constant {
            PoolConstant::I32(v) => builder = builder.add_i32_constant(*v),
            PoolConstant::I64(v) => builder = builder.add_i64_constant(*v),
        }
    }

    builder = builder.add_function(0, bytecode, max_stack_depth, num_locals);

    Ok(builder.build())
}

/// Tracks state during compilation of a single program.
struct CompileContext {
    /// Maps variable identifiers to their variable table indices.
    variables: HashMap<Id, u16>,
    /// Maps variable identifiers to their type information.
    var_types: HashMap<Id, VarTypeInfo>,
    /// Ordered list of constants added to the constant pool.
    constants: Vec<PoolConstant>,
    /// Stack of loop exit labels for EXIT statement compilation.
    /// Each enclosing loop pushes its end label; EXIT jumps to the top.
    loop_exit_labels: Vec<crate::emit::Label>,
}

impl CompileContext {
    fn new() -> Self {
        CompileContext {
            variables: HashMap::new(),
            var_types: HashMap::new(),
            constants: Vec::new(),
            loop_exit_labels: Vec::new(),
        }
    }

    /// Returns the exit label for the innermost enclosing loop, if any.
    fn current_loop_exit(&self) -> Option<crate::emit::Label> {
        self.loop_exit_labels.last().copied()
    }

    /// Looks up a variable index by identifier, using the provided span for error reporting.
    fn var_index(&self, name: &Id) -> Result<u16, Diagnostic> {
        self.variables.get(name).copied().ok_or_else(|| {
            Diagnostic::problem(
                Problem::VariableUndefined,
                Label::span(name.span(), "Variable reference"),
            )
            .with_context("variable", &name.to_string())
        })
    }

    /// Looks up type information for a variable by identifier.
    fn var_type_info(&self, name: &Id) -> Option<VarTypeInfo> {
        self.var_types.get(name).copied()
    }

    /// Returns the op_type for a variable by identifier, falling back to defaults.
    fn var_op_type(&self, name: &Id) -> OpType {
        self.var_types
            .get(name)
            .map(|info| (info.op_width, info.signedness))
            .unwrap_or(DEFAULT_OP_TYPE)
    }

    /// Adds an i32 constant to the pool and returns its index.
    fn add_i32_constant(&mut self, value: i32) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::I32(v) = existing {
                if *v == value {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::I32(value));
        index
    }

    /// Adds an i64 constant to the pool and returns its index.
    fn add_i64_constant(&mut self, value: i64) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::I64(v) = existing {
                if *v == value {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::I64(value));
        index
    }
}

/// Assigns variable table indices and type info for all variable declarations.
fn assign_variables(ctx: &mut CompileContext, declarations: &[VarDecl]) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            let index = ctx.variables.len() as u16;
            ctx.variables.insert(id.clone(), index);

            if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
                if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                    ctx.var_types.insert(id.clone(), type_info);
                }
            }
        }
    }
    Ok(())
}

/// Maps an IEC 61131-3 type name to its `VarTypeInfo`.
///
/// Returns `None` for unrecognized type names (e.g., user-defined types).
fn resolve_type_name(name: &Id) -> Option<VarTypeInfo> {
    match name.lower_case().as_str() {
        "sint" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 8,
        }),
        "int" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 16,
        }),
        "dint" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        "lint" => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        "usint" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        "uint" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        "udint" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        "ulint" => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        _ => None,
    }
}

/// Infers the operation type from an expression by finding the first variable reference.
///
/// IEC 61131-3 requires homogeneous operand types, so any variable in the expression
/// determines the operation type. For expressions without variables (pure constants),
/// returns the default `(W32, Signed)`.
fn infer_op_type(ctx: &CompileContext, expr: &ExprKind) -> OpType {
    match expr {
        ExprKind::Variable(variable) => {
            if let Variable::Symbolic(SymbolicVariableKind::Named(named)) = variable {
                return ctx.var_op_type(&named.name);
            }
            DEFAULT_OP_TYPE
        }
        ExprKind::LateBound(late_bound) => ctx.var_op_type(&late_bound.value),
        ExprKind::BinaryOp(binary) => {
            let left = infer_op_type(ctx, &binary.left);
            if left != DEFAULT_OP_TYPE {
                return left;
            }
            infer_op_type(ctx, &binary.right)
        }
        ExprKind::UnaryOp(unary) => infer_op_type(ctx, &unary.term),
        ExprKind::Compare(compare) => {
            let left = infer_op_type(ctx, &compare.left);
            if left != DEFAULT_OP_TYPE {
                return left;
            }
            infer_op_type(ctx, &compare.right)
        }
        ExprKind::Expression(inner) => infer_op_type(ctx, inner),
        _ => DEFAULT_OP_TYPE,
    }
}

/// Compiles a function block body.
fn compile_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &FunctionBlockBodyKind,
) -> Result<(), Diagnostic> {
    match body {
        FunctionBlockBodyKind::Statements(statements) => {
            compile_statements(emitter, ctx, statements)
        }
        FunctionBlockBodyKind::Empty => Ok(()),
        FunctionBlockBodyKind::Sfc(_) => Err(Diagnostic::todo(file!(), line!())),
    }
}

/// Compiles a sequence of statements.
fn compile_statements(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    statements: &Statements,
) -> Result<(), Diagnostic> {
    for stmt in &statements.body {
        compile_statement(emitter, ctx, stmt)?;
    }
    Ok(())
}

/// Compiles a single statement.
fn compile_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    stmt: &StmtKind,
) -> Result<(), Diagnostic> {
    match stmt {
        StmtKind::Assignment(assignment) => {
            // Look up the target variable's type info.
            let var_index = resolve_variable(ctx, &assignment.target)?;
            let target_name = resolve_variable_name(&assignment.target);
            let type_info = target_name.and_then(|name| ctx.var_type_info(name));
            let op_type = type_info
                .map(|ti| (ti.op_width, ti.signedness))
                .unwrap_or(DEFAULT_OP_TYPE);

            // Compile the right-hand side expression at the target's operation width.
            compile_expr(emitter, ctx, &assignment.value, op_type)?;

            // Truncate if the storage width is narrower than the operation width.
            if let Some(ti) = type_info {
                emit_truncation(emitter, ti);
            }

            // Store into the target variable.
            match op_type.0 {
                OpWidth::W32 => emitter.emit_store_var_i32(var_index),
                OpWidth::W64 => emitter.emit_store_var_i64(var_index),
            }
            Ok(())
        }
        StmtKind::FbCall(fb_call) => {
            Err(Diagnostic::todo_with_span(fb_call.span(), file!(), line!()))
        }
        StmtKind::If(if_stmt) => compile_if(emitter, ctx, if_stmt),
        StmtKind::Case(case_stmt) => compile_case(emitter, ctx, case_stmt),
        StmtKind::For(for_stmt) => compile_for(emitter, ctx, for_stmt),
        StmtKind::While(while_stmt) => compile_while(emitter, ctx, while_stmt),
        StmtKind::Repeat(repeat_stmt) => compile_repeat(emitter, ctx, repeat_stmt),
        StmtKind::Return => {
            emitter.emit_ret_void();
            Ok(())
        }
        StmtKind::Exit => {
            let label = ctx
                .current_loop_exit()
                .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
            emitter.emit_jmp(label);
            Ok(())
        }
    }
}

/// Compiles a slice of statements.
fn compile_stmts(
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

    // Compile the IF condition at its inferred operation type.
    let op_type = infer_op_type(ctx, &if_stmt.expr);
    compile_expr(emitter, ctx, &if_stmt.expr, op_type)?;

    // Jump past the then-body if condition is false.
    let next_label = emitter.create_label();
    emitter.emit_jmp_if_not(next_label);

    // Compile the then-body.
    compile_stmts(emitter, ctx, &if_stmt.body)?;

    // If there are more branches, jump to end.
    if needs_end_label {
        emitter.emit_jmp(end_label.unwrap());
    }

    emitter.bind_label(next_label);

    // Compile ELSIF clauses.
    for elsif in &if_stmt.else_ifs {
        let elsif_op_type = infer_op_type(ctx, &elsif.expr);
        compile_expr(emitter, ctx, &elsif.expr, elsif_op_type)?;
        let elsif_next = emitter.create_label();
        emitter.emit_jmp_if_not(elsif_next);

        compile_stmts(emitter, ctx, &elsif.body)?;

        emitter.emit_jmp(end_label.unwrap());

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
    let op_type = infer_op_type(ctx, &case_stmt.selector);

    for group in &case_stmt.statement_groups {
        let next_label = emitter.create_label();

        // Compile selector comparisons with OR logic.
        for (i, selection) in group.selectors.iter().enumerate() {
            compile_case_selector(emitter, ctx, &case_stmt.selector, selection, op_type)?;
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

/// Compiles a single case selector, leaving a boolean result on the stack.
///
/// - `SignedInteger`: `selector == value`
/// - `Subrange`: `(selector >= start) AND (selector <= end)`
/// - `EnumeratedValue`: not yet supported (returns todo diagnostic)
fn compile_case_selector(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    selector_expr: &ExprKind,
    selection: &CaseSelectionKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match selection {
        CaseSelectionKind::SignedInteger(si) => {
            compile_expr(emitter, ctx, selector_expr, op_type)?;
            match op_type.0 {
                OpWidth::W32 => {
                    let value = signed_integer_to_i32(si)?;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                    emitter.emit_eq_i32();
                }
                OpWidth::W64 => {
                    let value = signed_integer_to_i64(si)?;
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                    emitter.emit_eq_i64();
                }
            }
            Ok(())
        }
        CaseSelectionKind::Subrange(sr) => {
            // (selector >= start) AND (selector <= end)
            compile_expr(emitter, ctx, selector_expr, op_type)?;
            match op_type.0 {
                OpWidth::W32 => {
                    let start = signed_integer_to_i32(&sr.start)?;
                    let start_index = ctx.add_i32_constant(start);
                    emitter.emit_load_const_i32(start_index);
                    emit_ge(emitter, op_type);

                    compile_expr(emitter, ctx, selector_expr, op_type)?;
                    let end = signed_integer_to_i32(&sr.end)?;
                    let end_index = ctx.add_i32_constant(end);
                    emitter.emit_load_const_i32(end_index);
                    emit_le(emitter, op_type);
                }
                OpWidth::W64 => {
                    let start = signed_integer_to_i64(&sr.start)?;
                    let start_index = ctx.add_i64_constant(start);
                    emitter.emit_load_const_i64(start_index);
                    emit_ge(emitter, op_type);

                    compile_expr(emitter, ctx, selector_expr, op_type)?;
                    let end = signed_integer_to_i64(&sr.end)?;
                    let end_index = ctx.add_i64_constant(end);
                    emitter.emit_load_const_i64(end_index);
                    emit_le(emitter, op_type);
                }
            }

            emitter.emit_bool_and();
            Ok(())
        }
        CaseSelectionKind::EnumeratedValue(ev) => {
            Err(Diagnostic::todo_with_span(ev.span(), file!(), line!()))
        }
    }
}

/// Converts a `SignedInteger` AST node to an `i32` value.
fn signed_integer_to_i32(si: &SignedInteger) -> Result<i32, Diagnostic> {
    if si.is_neg {
        let unsigned = si.value.value as i128;
        let signed = -unsigned;
        i32::try_from(signed).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &signed.to_string())
        })
    } else {
        i32::try_from(si.value.value).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &si.value.value.to_string())
        })
    }
}

/// Compiles a WHILE statement.
///
/// ```text
/// LOOP:
///   compile(condition)
///   JMP_IF_NOT → END
///   compile(body)
///   JMP → LOOP
/// END:
/// ```
fn compile_while(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    while_stmt: &ironplc_dsl::textual::While,
) -> Result<(), Diagnostic> {
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    emitter.bind_label(loop_label);
    let op_type = infer_op_type(ctx, &while_stmt.condition);
    compile_expr(emitter, ctx, &while_stmt.condition, op_type)?;
    emitter.emit_jmp_if_not(end_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &while_stmt.body)?;
    ctx.loop_exit_labels.pop();
    emitter.emit_jmp(loop_label);
    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles a REPEAT statement.
///
/// ```text
/// LOOP:
///   compile(body)
///   compile(condition)
///   JMP_IF_NOT → LOOP
/// ```
fn compile_repeat(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    repeat_stmt: &ironplc_dsl::textual::Repeat,
) -> Result<(), Diagnostic> {
    let loop_label = emitter.create_label();
    let end_label = emitter.create_label();

    emitter.bind_label(loop_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &repeat_stmt.body)?;
    ctx.loop_exit_labels.pop();
    let op_type = infer_op_type(ctx, &repeat_stmt.until);
    compile_expr(emitter, ctx, &repeat_stmt.until, op_type)?;
    emitter.emit_jmp_if_not(loop_label);
    emitter.bind_label(end_label);

    Ok(())
}

/// Whether a compile-time constant step is positive or negative.
enum StepSign {
    Positive,
    Negative,
}

/// Inspects an expression and returns its sign if it is a compile-time constant
/// integer literal (positive or negative). Returns `None` for non-constant
/// expressions.
fn try_constant_sign(expr: &ExprKind) -> Option<StepSign> {
    match expr {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            if lit.value.is_neg {
                Some(StepSign::Negative)
            } else {
                Some(StepSign::Positive)
            }
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => {
            // -<literal> is negative
            if matches!(
                &unary.term,
                ExprKind::Const(ConstantKind::IntegerLiteral(_))
            ) {
                Some(StepSign::Negative)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Compiles a FOR statement.
///
/// ```text
///   compile(from)
///   STORE_VAR control
/// LOOP:
///   LOAD_VAR control
///   compile(to)
///   GT_I32 (or LT_I32 for negative step)
///   JMP_IF_NOT → BODY
///   JMP → END
/// BODY:
///   compile(body)
///   LOAD_VAR control
///   compile(step)  // default: LOAD_CONST 1
///   ADD_I32
///   STORE_VAR control
///   JMP → LOOP
/// END:
/// ```
fn compile_for(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    for_stmt: &ironplc_dsl::textual::For,
) -> Result<(), Diagnostic> {
    let var_index = ctx.var_index(&for_stmt.control)?;
    let op_type = ctx.var_op_type(&for_stmt.control);
    let type_info = ctx.var_type_info(&for_stmt.control);

    // Determine step sign.
    let step_sign = match &for_stmt.step {
        None => StepSign::Positive,
        Some(step_expr) => match try_constant_sign(step_expr) {
            Some(sign) => sign,
            None => return Err(Diagnostic::todo(file!(), line!())),
        },
    };

    // Initialize: compile(from), STORE_VAR control
    compile_expr(emitter, ctx, &for_stmt.from, op_type)?;
    if let Some(ti) = type_info {
        emit_truncation(emitter, ti);
    }
    emit_store_var(emitter, var_index, op_type);

    let loop_label = emitter.create_label();
    let body_label = emitter.create_label();
    let end_label = emitter.create_label();

    // LOOP: check termination condition
    emitter.bind_label(loop_label);
    emit_load_var(emitter, var_index, op_type);
    compile_expr(emitter, ctx, &for_stmt.to, op_type)?;
    match step_sign {
        StepSign::Positive => emit_gt(emitter, op_type),
        StepSign::Negative => emit_lt(emitter, op_type),
    }
    emitter.emit_jmp_if_not(body_label);
    emitter.emit_jmp(end_label);

    // BODY:
    emitter.bind_label(body_label);
    ctx.loop_exit_labels.push(end_label);
    compile_stmts(emitter, ctx, &for_stmt.body)?;
    ctx.loop_exit_labels.pop();

    // Increment: LOAD_VAR control, compile(step), ADD, truncate, STORE_VAR control
    emit_load_var(emitter, var_index, op_type);
    match &for_stmt.step {
        Some(step_expr) => compile_expr(emitter, ctx, step_expr, op_type)?,
        None => match op_type.0 {
            OpWidth::W32 => {
                let one_index = ctx.add_i32_constant(1);
                emitter.emit_load_const_i32(one_index);
            }
            OpWidth::W64 => {
                let one_index = ctx.add_i64_constant(1);
                emitter.emit_load_const_i64(one_index);
            }
        },
    }
    emit_add(emitter, op_type);
    if let Some(ti) = type_info {
        emit_truncation(emitter, ti);
    }
    emit_store_var(emitter, var_index, op_type);
    emitter.emit_jmp(loop_label);

    // END:
    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles an expression, leaving the result on the stack.
///
/// The `op_type` determines which width (i32/i64) and signedness to use
/// for arithmetic, comparison, and load/store instructions.
fn compile_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &ExprKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match expr {
        ExprKind::Const(constant) => compile_constant(emitter, ctx, constant, op_type),
        ExprKind::Variable(variable) => {
            let var_index = resolve_variable(ctx, variable)?;
            emit_load_var(emitter, var_index, op_type);
            Ok(())
        }
        ExprKind::BinaryOp(binary) => {
            compile_expr(emitter, ctx, &binary.left, op_type)?;
            compile_expr(emitter, ctx, &binary.right, op_type)?;
            match binary.op {
                Operator::Add => emit_add(emitter, op_type),
                Operator::Sub => emit_sub(emitter, op_type),
                Operator::Mul => emit_mul(emitter, op_type),
                Operator::Div => emit_div(emitter, op_type),
                Operator::Mod => emit_mod(emitter, op_type),
                Operator::Pow => {
                    emitter.emit_builtin(opcode::builtin::EXPT_I32);
                }
            }
            Ok(())
        }
        ExprKind::UnaryOp(unary) => match unary.op {
            UnaryOp::Neg => {
                // Constant folding: if the operand is an integer literal,
                // emit the negated constant directly.
                if let ExprKind::Const(ConstantKind::IntegerLiteral(lit)) = &unary.term {
                    let unsigned = lit.value.value.value as i128;
                    let signed = -unsigned;
                    match op_type.0 {
                        OpWidth::W32 => {
                            let value = i32::try_from(signed).map_err(|_| {
                                Diagnostic::problem(
                                    Problem::ConstantOverflow,
                                    Label::span(lit.value.value.span(), "Integer literal"),
                                )
                                .with_context("value", &format!("-{}", unsigned))
                            })?;
                            let pool_index = ctx.add_i32_constant(value);
                            emitter.emit_load_const_i32(pool_index);
                        }
                        OpWidth::W64 => {
                            let value = i64::try_from(signed).map_err(|_| {
                                Diagnostic::problem(
                                    Problem::ConstantOverflow,
                                    Label::span(lit.value.value.span(), "Integer literal"),
                                )
                                .with_context("value", &format!("-{}", unsigned))
                            })?;
                            let pool_index = ctx.add_i64_constant(value);
                            emitter.emit_load_const_i64(pool_index);
                        }
                    }
                    Ok(())
                } else {
                    compile_expr(emitter, ctx, &unary.term, op_type)?;
                    emit_neg(emitter, op_type);
                    Ok(())
                }
            }
            UnaryOp::Not => {
                compile_expr(emitter, ctx, &unary.term, op_type)?;
                emitter.emit_bool_not();
                Ok(())
            }
        },
        ExprKind::LateBound(late_bound) => {
            let var_index = ctx.var_index(&late_bound.value)?;
            emit_load_var(emitter, var_index, op_type);
            Ok(())
        }
        ExprKind::Expression(inner) => compile_expr(emitter, ctx, inner, op_type),
        ExprKind::Compare(compare) => {
            compile_expr(emitter, ctx, &compare.left, op_type)?;
            compile_expr(emitter, ctx, &compare.right, op_type)?;
            match compare.op {
                CompareOp::Eq => emit_eq(emitter, op_type),
                CompareOp::Ne => emit_ne(emitter, op_type),
                CompareOp::Lt => emit_lt(emitter, op_type),
                CompareOp::Gt => emit_gt(emitter, op_type),
                CompareOp::LtEq => emit_le(emitter, op_type),
                CompareOp::GtEq => emit_ge(emitter, op_type),
                CompareOp::And => emitter.emit_bool_and(),
                CompareOp::Or => emitter.emit_bool_or(),
                CompareOp::Xor => emitter.emit_bool_xor(),
            }
            Ok(())
        }
        ExprKind::EnumeratedValue(enum_val) => Err(Diagnostic::todo_with_span(
            enum_val.span(),
            file!(),
            line!(),
        )),
        ExprKind::Function(func) => Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        )),
    }
}

/// Compiles a constant literal, pushing it onto the stack.
fn compile_constant(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    constant: &ConstantKind,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match constant {
        ConstantKind::IntegerLiteral(lit) => {
            let span = lit.value.value.span();
            match op_type {
                (OpWidth::W32, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        let unsigned = lit.value.value.value as i128;
                        let signed = -unsigned;
                        i32::try_from(signed).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &signed.to_string())
                        })?
                    } else {
                        i32::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })?
                    };
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W32, Signedness::Unsigned) => {
                    // Unsigned 32-bit: values up to u32::MAX are valid.
                    // Store the bit-pattern as i32.
                    let value = if lit.value.is_neg {
                        return Err(Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Integer literal"),
                        )
                        .with_context("value", &format!("-{}", lit.value.value.value)));
                    } else {
                        u32::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })? as i32
                    };
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W64, Signedness::Signed) => {
                    let value = if lit.value.is_neg {
                        let unsigned = lit.value.value.value as i128;
                        let signed = -unsigned;
                        i64::try_from(signed).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &signed.to_string())
                        })?
                    } else {
                        i64::try_from(lit.value.value.value).map_err(|_| {
                            Diagnostic::problem(
                                Problem::ConstantOverflow,
                                Label::span(span.clone(), "Integer literal"),
                            )
                            .with_context("value", &lit.value.value.value.to_string())
                        })?
                    };
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
                (OpWidth::W64, Signedness::Unsigned) => {
                    // Unsigned 64-bit: values up to u64::MAX are valid.
                    // Store the bit-pattern as i64.
                    let value = if lit.value.is_neg {
                        return Err(Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Integer literal"),
                        )
                        .with_context("value", &format!("-{}", lit.value.value.value)));
                    } else {
                        lit.value.value.value as i64
                    };
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::RealLiteral(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::Boolean(lit) => {
            match lit.value {
                Boolean::True => emitter.emit_load_true(),
                Boolean::False => emitter.emit_load_false(),
            }
            Ok(())
        }
        ConstantKind::CharacterString(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::Duration(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::TimeOfDay(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::Date(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::DateAndTime(_) => Err(Diagnostic::todo(file!(), line!())),
        ConstantKind::BitStringLiteral(_) => Err(Diagnostic::todo(file!(), line!())),
    }
}

/// Resolves a variable reference to its variable table index.
fn resolve_variable(ctx: &CompileContext, variable: &Variable) -> Result<u16, Diagnostic> {
    match variable {
        Variable::Symbolic(symbolic) => match symbolic {
            SymbolicVariableKind::Named(named) => ctx.var_index(&named.name),
            SymbolicVariableKind::Array(array) => {
                Err(Diagnostic::todo_with_span(array.span(), file!(), line!()))
            }
            SymbolicVariableKind::Structured(structured) => Err(Diagnostic::todo_with_span(
                structured.span(),
                file!(),
                line!(),
            )),
        },
        Variable::Direct(direct) => Err(Diagnostic::todo_with_span(
            direct.position.clone(),
            file!(),
            line!(),
        )),
    }
}

/// Extracts the variable name `Id` from a variable reference, if it is a named symbolic variable.
fn resolve_variable_name(variable: &Variable) -> Option<&Id> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Named(named)) => Some(&named.name),
        _ => None,
    }
}

/// Converts a `SignedInteger` AST node to an `i64` value.
fn signed_integer_to_i64(si: &SignedInteger) -> Result<i64, Diagnostic> {
    if si.is_neg {
        let unsigned = si.value.value as i128;
        let signed = -unsigned;
        i64::try_from(signed).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &signed.to_string())
        })
    } else {
        i64::try_from(si.value.value).map_err(|_| {
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(si.value.span(), "Integer literal"),
            )
            .with_context("value", &si.value.value.to_string())
        })
    }
}

// --- Typed opcode emission helpers ---
//
// Each helper selects the correct opcode based on the operation type
// (width and/or signedness).

fn emit_truncation(emitter: &mut Emitter, type_info: VarTypeInfo) {
    match (
        type_info.op_width,
        type_info.signedness,
        type_info.storage_bits,
    ) {
        (OpWidth::W32, Signedness::Signed, 8) => emitter.emit_trunc_i8(),
        (OpWidth::W32, Signedness::Signed, 16) => emitter.emit_trunc_i16(),
        (OpWidth::W32, Signedness::Unsigned, 8) => emitter.emit_trunc_u8(),
        (OpWidth::W32, Signedness::Unsigned, 16) => emitter.emit_trunc_u16(),
        // 32-bit and 64-bit types fill their native width; no truncation needed.
        _ => {}
    }
}

fn emit_load_var(emitter: &mut Emitter, var_index: u16, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_load_var_i32(var_index),
        OpWidth::W64 => emitter.emit_load_var_i64(var_index),
    }
}

fn emit_store_var(emitter: &mut Emitter, var_index: u16, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_store_var_i32(var_index),
        OpWidth::W64 => emitter.emit_store_var_i64(var_index),
    }
}

fn emit_add(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_add_i32(),
        OpWidth::W64 => emitter.emit_add_i64(),
    }
}

fn emit_sub(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_sub_i32(),
        OpWidth::W64 => emitter.emit_sub_i64(),
    }
}

fn emit_mul(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_mul_i32(),
        OpWidth::W64 => emitter.emit_mul_i64(),
    }
}

fn emit_div(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_div_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_div_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_div_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_div_u64(),
    }
}

fn emit_mod(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_mod_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_mod_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_mod_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_mod_u64(),
    }
}

fn emit_neg(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_neg_i32(),
        OpWidth::W64 => emitter.emit_neg_i64(),
    }
}

fn emit_eq(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_eq_i32(),
        OpWidth::W64 => emitter.emit_eq_i64(),
    }
}

fn emit_ne(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_ne_i32(),
        OpWidth::W64 => emitter.emit_ne_i64(),
    }
}

fn emit_lt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_lt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_lt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_lt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_lt_u64(),
    }
}

fn emit_le(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_le_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_le_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_le_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_le_u64(),
    }
}

fn emit_gt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_gt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_gt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_gt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_gt_u64(),
    }
}

fn emit_ge(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_ge_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_ge_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_ge_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_ge_u64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::ParseOptions;
    use ironplc_parser::parse_program;

    /// Helper to parse an IEC 61131-3 program string into a Library.
    fn parse(source: &str) -> Library {
        parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap()
    }

    #[test]
    fn compile_when_simple_assignment_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(container.header.num_functions, 1);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);

        // LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0, RET_VOID
        let bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn compile_when_no_program_then_error() {
        let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK
";
        let library = parse(source);
        let result = compile(&library);

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::NotImplemented.code());
    }

    #[test]
    fn compile_when_empty_program_then_produces_ret_void() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        // Should have one function with just RET_VOID
        assert_eq!(container.header.num_functions, 1);
        let bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(bytecode, &[0xB5]); // RET_VOID only
    }

    #[test]
    fn compile_when_duplicate_constants_then_deduplicates() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := 10;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        // Should only have one constant (10 is deduplicated)
        assert_eq!(container.constant_pool.len(), 1);
    }

    #[test]
    fn compile_when_variable_to_variable_assignment_then_load_store() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.num_functions, 1);

        // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
        // y := x:  LOAD_VAR_I32 var:0, STORE_VAR_I32 var:1
        // RET_VOID
        let bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(
            bytecode,
            &[
                0x01, 0x00, 0x00, // LOAD_CONST_I32 pool:0
                0x18, 0x00, 0x00, // STORE_VAR_I32 var:0
                0x10, 0x00, 0x00, // LOAD_VAR_I32 var:0
                0x18, 0x01, 0x00, // STORE_VAR_I32 var:1
                0xB5, // RET_VOID
            ]
        );
    }

    #[test]
    fn compile_when_negative_constant_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := -5;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.constant_pool.get_i32(0).unwrap(), -5);

        // LOAD_CONST_I32 pool:0 (-5), STORE_VAR_I32 var:0, RET_VOID
        let bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn compile_when_simple_if_then_succeeds() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  IF x > 0 THEN
    x := 1;
  END_IF;
END_PROGRAM
";
        let library = parse(source);
        let result = compile(&library);

        assert!(result.is_ok());
    }
}
