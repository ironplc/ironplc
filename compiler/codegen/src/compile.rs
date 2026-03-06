//! Compiles an IEC 61131-3 AST into a bytecode container.
//!
//! This module walks the AST produced by the parser/analyzer and generates
//! bytecode that the IronPLC VM can execute.
//!
//! # Supported constructs
//!
//! - PROGRAM declarations with all 8 IEC 61131-3 integer types
//!   (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT),
//!   2 floating-point types (REAL, LREAL),
//!   5 bit string types (BOOL, BYTE, WORD, DWORD, LWORD)
//! - Assignment statements with truncation for narrow types
//! - Integer literal constants (i32 and i64)
//! - Real literal constants (f32 and f64)
//! - Boolean literal constants (TRUE, FALSE)
//! - Bit string literal constants (BYTE, WORD, DWORD, LWORD prefixed)
//! - Binary Add, Sub, Mul, Div, Mod, and Pow operators
//! - Unary Neg and Not operators
//! - Comparison operators (=, <>, <, <=, >, >=)
//! - Boolean operators (AND, OR, XOR, NOT) with logical semantics
//! - Bitwise operators (AND, OR, XOR, NOT) for bit string types (BYTE, WORD, DWORD, LWORD)
//! - Bit shift/rotate functions (SHL, SHR, ROL, ROR) for bit string types
//! - MUX (multiplexer) function with variable arity (2..16 inputs)
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
use ironplc_dsl::core::{FileId, Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CaseSelectionKind, CompareOp, Expr, ExprKind, Function, Operator, ParamAssignmentKind,
    Statements, StmtKind, SymbolicVariableKind, UnaryOp, Variable,
};
use ironplc_problems::Problem;

use crate::emit::Emitter;

/// The native operation width used for arithmetic and comparisons.
#[derive(Clone, Copy, PartialEq)]
enum OpWidth {
    /// 32-bit integer operations (for SINT, INT, DINT, USINT, UINT, UDINT).
    W32,
    /// 64-bit integer operations (for LINT, ULINT).
    W64,
    /// 32-bit float operations (for REAL).
    F32,
    /// 64-bit float operations (for LREAL).
    F64,
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

/// A constant in the pool: integer or float, 32-bit or 64-bit.
enum PoolConstant {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
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
    Err(Diagnostic::problem(
        Problem::NoProgramDeclaration,
        Label::file(
            FileId::default(),
            "Source does not contain a PROGRAM declaration",
        ),
    ))
}

/// Compiles a single PROGRAM into a container.
///
/// Always emits two functions:
/// - Function 0: init (load constants + store variables, called once by VM)
/// - Function 1: scan (program body, called every scan cycle)
///
/// When no initial values exist, the init function is a single RET_VOID.
fn compile_program(program: &ProgramDeclaration) -> Result<Container, Diagnostic> {
    let mut ctx = CompileContext::new();

    // Assign variable indices for all declared variables.
    assign_variables(&mut ctx, &program.variables)?;

    // Emit bytecode for variable initial values into the init emitter.
    let mut init_emitter = Emitter::new();
    emit_initial_values(&mut init_emitter, &mut ctx, &program.variables)?;
    init_emitter.emit_ret_void();

    // Compile the program body into the scan emitter.
    let mut scan_emitter = Emitter::new();
    compile_body(&mut scan_emitter, &mut ctx, &program.body)?;
    scan_emitter.emit_ret_void();

    // Build the container.
    let num_variables = ctx.variables.len() as u16;
    let num_locals = num_variables;

    let mut builder = ContainerBuilder::new().num_variables(num_variables);

    // Add constants to the pool.
    for constant in &ctx.constants {
        match constant {
            PoolConstant::I32(v) => builder = builder.add_i32_constant(*v),
            PoolConstant::I64(v) => builder = builder.add_i64_constant(*v),
            PoolConstant::F32(v) => builder = builder.add_f32_constant(*v),
            PoolConstant::F64(v) => builder = builder.add_f64_constant(*v),
        }
    }

    // Function 0: init, Function 1: scan
    let init_stack = init_emitter.max_stack_depth();
    let init_bytecode = init_emitter.bytecode();
    builder = builder.add_function(0, init_bytecode, init_stack, num_locals);

    let scan_stack = scan_emitter.max_stack_depth();
    let scan_bytecode = scan_emitter.bytecode();
    builder = builder.add_function(1, scan_bytecode, scan_stack, num_locals);

    builder = builder.init_function_id(0).entry_function_id(1);

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

    /// Adds an f32 constant to the pool and returns its index.
    fn add_f32_constant(&mut self, value: f32) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::F32(v) = existing {
                if v.to_bits() == value.to_bits() {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::F32(value));
        index
    }

    /// Adds an f64 constant to the pool and returns its index.
    fn add_f64_constant(&mut self, value: f64) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if let PoolConstant::F64(v) = existing {
                if v.to_bits() == value.to_bits() {
                    return i as u16;
                }
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(PoolConstant::F64(value));
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

/// Emits bytecode to initialize variables that have declared initial values.
///
/// For each variable with a `SimpleInitializer` containing an integer literal,
/// emits load-constant + truncate (if narrow) + store-variable instructions.
fn emit_initial_values(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    declarations: &[VarDecl],
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
                if let Some(constant) = &simple.initial_value {
                    let var_index = ctx.var_index(id)?;
                    let type_info = ctx.var_type_info(id);
                    let op_type = type_info
                        .map(|ti| (ti.op_width, ti.signedness))
                        .unwrap_or(DEFAULT_OP_TYPE);

                    compile_constant(emitter, ctx, constant, op_type)?;

                    if let Some(ti) = type_info {
                        emit_truncation(emitter, ti);
                    }

                    emit_store_var(emitter, var_index, op_type);
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
        "real" => Some(VarTypeInfo {
            op_width: OpWidth::F32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        "lreal" => Some(VarTypeInfo {
            op_width: OpWidth::F64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        "bool" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 1,
        }),
        "byte" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        "word" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        "dword" => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        "lword" => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        _ => None,
    }
}

/// Checks if a function name is a type conversion (e.g., "int_to_real").
/// Returns `Some((source_type_info, target_type_info))` if both parts are
/// recognized type names, `None` otherwise.
fn parse_type_conversion(name: &str) -> Option<(VarTypeInfo, VarTypeInfo)> {
    let upper = name.to_uppercase();
    let parts: Vec<&str> = upper.splitn(2, "_TO_").collect();
    if parts.len() != 2 {
        return None;
    }
    let source = resolve_type_name(&Id::from(parts[0]))?;
    let target = resolve_type_name(&Id::from(parts[1]))?;
    Some((source, target))
}

/// Returns the operation type from an expression's resolved type annotation.
///
/// The analyzer must have populated `expr.resolved_type`. A missing or
/// unrecognized resolved type is a compiler bug.
fn op_type(expr: &Expr) -> Result<OpType, Diagnostic> {
    let resolved = expr
        .resolved_type
        .as_ref()
        .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    let info =
        resolve_type_name(&resolved.name).ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    Ok((info.op_width, info.signedness))
}

/// Returns the storage bit width from an expression's resolved type annotation.
///
/// The analyzer must have populated `expr.resolved_type`. A missing or
/// unrecognized resolved type is a compiler bug.
fn storage_bits(expr: &Expr) -> Result<u8, Diagnostic> {
    let resolved = expr
        .resolved_type
        .as_ref()
        .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    let info =
        resolve_type_name(&resolved.name).ok_or_else(|| Diagnostic::todo(file!(), line!()))?;
    Ok(info.storage_bits)
}

/// Returns the operation type for compiling a condition expression.
///
/// For comparison operators (`>`, `<`, `=`, etc.), returns the type of the
/// left operand since the comparison's own resolved type is BOOL but we need
/// the operand type for correct signedness. For boolean combinations (AND,
/// OR, XOR), recurses into the first operand. For other expressions (bare
/// boolean variables, parenthesized expressions), returns the expression's
/// own resolved type.
fn condition_op_type(expr: &Expr) -> Result<OpType, Diagnostic> {
    match &expr.kind {
        ExprKind::Compare(compare) => match compare.op {
            CompareOp::And | CompareOp::Or | CompareOp::Xor => condition_op_type(&compare.left),
            _ => op_type(&compare.left),
        },
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Not => condition_op_type(&unary.term),
        ExprKind::Expression(inner) => condition_op_type(inner),
        _ => op_type(expr),
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
            emit_store_var(emitter, var_index, op_type);
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
        StmtKind::Exit(span) => {
            let label = ctx.current_loop_exit().ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ExitOutsideLoop,
                    Label::span(
                        span.clone(),
                        "EXIT must be inside a FOR, WHILE, or REPEAT loop",
                    ),
                )
            })?;
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
    let cond_type = condition_op_type(&if_stmt.expr)?;
    compile_expr(emitter, ctx, &if_stmt.expr, cond_type)?;

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
        let elsif_op_type = condition_op_type(&elsif.expr)?;
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
    let op_type = op_type(&case_stmt.selector)?;

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
    selector_expr: &Expr,
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
                // CASE with float types is not meaningful in IEC 61131-3.
                _ => return Err(Diagnostic::todo(file!(), line!())),
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
                // CASE with float types is not meaningful in IEC 61131-3.
                _ => return Err(Diagnostic::todo(file!(), line!())),
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
    let cond_type = condition_op_type(&while_stmt.condition)?;
    compile_expr(emitter, ctx, &while_stmt.condition, cond_type)?;
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
    let cond_type = condition_op_type(&repeat_stmt.until)?;
    compile_expr(emitter, ctx, &repeat_stmt.until, cond_type)?;
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
fn try_constant_sign(expr: &Expr) -> Option<StepSign> {
    match &expr.kind {
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
                &unary.term.kind,
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
            None => {
                return Err(Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(
                        for_stmt.control.span(),
                        "FOR loop step must be a constant expression",
                    ),
                ))
            }
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
            OpWidth::F32 => {
                let one_index = ctx.add_f32_constant(1.0);
                emitter.emit_load_const_f32(one_index);
            }
            OpWidth::F64 => {
                let one_index = ctx.add_f64_constant(1.0);
                emitter.emit_load_const_f64(one_index);
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

/// Returns the builtin opcode for a named standard library function, if known.
///
/// The `op_width` selects the correct width variant and `signedness` selects
/// the signed/unsigned variant for functions that distinguish them.
fn lookup_builtin(name: &str, op_width: OpWidth, signedness: Signedness) -> Option<u16> {
    match name.to_uppercase().as_str() {
        "EXPT" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::EXPT_I32,
            OpWidth::W64 => opcode::builtin::EXPT_I64,
            OpWidth::F32 => opcode::builtin::EXPT_F32,
            OpWidth::F64 => opcode::builtin::EXPT_F64,
        }),
        "ABS" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::ABS_I32,
            OpWidth::W64 => opcode::builtin::ABS_I64,
            OpWidth::F32 => opcode::builtin::ABS_F32,
            OpWidth::F64 => opcode::builtin::ABS_F64,
        }),
        "MIN" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MIN_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MIN_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MIN_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MIN_U64,
            (OpWidth::F32, _) => opcode::builtin::MIN_F32,
            (OpWidth::F64, _) => opcode::builtin::MIN_F64,
        }),
        "MAX" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::MAX_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::MAX_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::MAX_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::MAX_U64,
            (OpWidth::F32, _) => opcode::builtin::MAX_F32,
            (OpWidth::F64, _) => opcode::builtin::MAX_F64,
        }),
        "LIMIT" => Some(match (op_width, signedness) {
            (OpWidth::W32, Signedness::Signed) => opcode::builtin::LIMIT_I32,
            (OpWidth::W32, Signedness::Unsigned) => opcode::builtin::LIMIT_U32,
            (OpWidth::W64, Signedness::Signed) => opcode::builtin::LIMIT_I64,
            (OpWidth::W64, Signedness::Unsigned) => opcode::builtin::LIMIT_U64,
            (OpWidth::F32, _) => opcode::builtin::LIMIT_F32,
            (OpWidth::F64, _) => opcode::builtin::LIMIT_F64,
        }),
        "SEL" => Some(match op_width {
            OpWidth::W32 => opcode::builtin::SEL_I32,
            OpWidth::W64 => opcode::builtin::SEL_I64,
            OpWidth::F32 => opcode::builtin::SEL_F32,
            OpWidth::F64 => opcode::builtin::SEL_F64,
        }),
        "SQRT" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SQRT_F32),
            OpWidth::F64 => Some(opcode::builtin::SQRT_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "LN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LN_F32),
            OpWidth::F64 => Some(opcode::builtin::LN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "LOG" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::LOG_F32),
            OpWidth::F64 => Some(opcode::builtin::LOG_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "EXP" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::EXP_F32),
            OpWidth::F64 => Some(opcode::builtin::EXP_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "SIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::SIN_F32),
            OpWidth::F64 => Some(opcode::builtin::SIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "COS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::COS_F32),
            OpWidth::F64 => Some(opcode::builtin::COS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "TAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::TAN_F32),
            OpWidth::F64 => Some(opcode::builtin::TAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ASIN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ASIN_F32),
            OpWidth::F64 => Some(opcode::builtin::ASIN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ACOS" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ACOS_F32),
            OpWidth::F64 => Some(opcode::builtin::ACOS_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        "ATAN" => match op_width {
            OpWidth::F32 => Some(opcode::builtin::ATAN_F32),
            OpWidth::F64 => Some(opcode::builtin::ATAN_F64),
            OpWidth::W32 | OpWidth::W64 => None,
        },
        _ => None,
    }
}

/// Compiles an expression, leaving the result on the stack.
///
/// The `op_type` determines which width (i32/i64) and signedness to use
/// for arithmetic, comparison, and load/store instructions.
fn compile_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &Expr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    match &expr.kind {
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
                Operator::Pow => emit_pow(emitter, op_type),
            }
            Ok(())
        }
        ExprKind::UnaryOp(unary) => match unary.op {
            UnaryOp::Neg => {
                // Constant folding: if the operand is an integer literal,
                // emit the negated constant directly.
                if let ExprKind::Const(ConstantKind::IntegerLiteral(lit)) = &unary.term.kind {
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
                        OpWidth::F32 => {
                            let value = -(unsigned as f32);
                            let pool_index = ctx.add_f32_constant(value);
                            emitter.emit_load_const_f32(pool_index);
                        }
                        OpWidth::F64 => {
                            let value = -(unsigned as f64);
                            let pool_index = ctx.add_f64_constant(value);
                            emitter.emit_load_const_f64(pool_index);
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
                match op_type {
                    (OpWidth::W32, Signedness::Unsigned) => {
                        emitter.emit_bit_not_32();
                        match storage_bits(&unary.term)? {
                            8 => emitter.emit_trunc_u8(),
                            16 => emitter.emit_trunc_u16(),
                            _ => {}
                        }
                    }
                    (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_not_64(),
                    _ => emitter.emit_bool_not(),
                }
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
                CompareOp::And => emit_and(emitter, op_type),
                CompareOp::Or => emit_or(emitter, op_type),
                CompareOp::Xor => emit_xor(emitter, op_type),
            }
            Ok(())
        }
        ExprKind::EnumeratedValue(enum_val) => Err(Diagnostic::todo_with_span(
            enum_val.span(),
            file!(),
            line!(),
        )),
        ExprKind::Function(func) => compile_function_call(emitter, ctx, func, op_type),
    }
}

/// Compiles a standard library function call.
///
/// Dispatches shift/rotate functions to a width-aware handler, and other
/// known builtins to the generic lookup path.
fn compile_function_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let name = func.name.lower_case();
    match name.as_str() {
        "shl" | "shr" | "rol" | "ror" => {
            compile_shift_rotate(emitter, ctx, func, op_type, name.as_str())
        }
        "mux" => compile_mux(emitter, ctx, func, op_type),
        _ => {
            if let Some((source, target)) = parse_type_conversion(name) {
                compile_type_conversion(emitter, ctx, func, source, target)
            } else {
                compile_generic_builtin(emitter, ctx, func, op_type)
            }
        }
    }
}

/// Compiles a generic builtin function call via `lookup_builtin`.
///
/// All arguments are compiled with the same `op_type` and the function ID
/// is looked up by name.
fn compile_generic_builtin(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let func_name = func.name.original().to_uppercase();
    let func_id = lookup_builtin(&func_name, op_type.0, op_type.1)
        .ok_or_else(|| Diagnostic::todo_with_span(func.name.span(), file!(), line!()))?;

    let expected_args = opcode::builtin::arg_count(func_id) as usize;

    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != expected_args {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let is_sel = func_name == "SEL";
    for (i, arg) in args.iter().enumerate() {
        // SEL's first argument (G) is always a BOOL/integer selector,
        // even when the remaining arguments are float.
        let arg_op_type = if is_sel && i == 0 {
            DEFAULT_OP_TYPE
        } else {
            op_type
        };
        compile_expr(emitter, ctx, arg, arg_op_type)?;
    }

    emitter.emit_builtin(func_id);
    Ok(())
}

/// Compiles a MUX (multiplexer) function call.
///
/// MUX(K, IN0, IN1, ..., INn) selects one of the IN values based on the
/// integer selector K. The first argument K is always compiled as I32
/// (integer selector), while the remaining IN arguments use the caller's op_type.
///
/// The opcode encodes the number of IN arguments: `MUX_<WIDTH>_BASE + num_inputs`.
fn compile_mux(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    // Must have at least 3 args (K + 2 IN values)
    if args.len() < 3 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let num_inputs = (args.len() - 1) as u16; // subtract K

    if num_inputs > opcode::builtin::MUX_MAX_INPUTS {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    let base = match op_type.0 {
        OpWidth::W32 => opcode::builtin::MUX_I32_BASE,
        OpWidth::W64 => opcode::builtin::MUX_I64_BASE,
        OpWidth::F32 => opcode::builtin::MUX_F32_BASE,
        OpWidth::F64 => opcode::builtin::MUX_F64_BASE,
    };
    let func_id = base + num_inputs;

    // Compile K (first arg) as integer
    compile_expr(emitter, ctx, args[0], DEFAULT_OP_TYPE)?;

    // Compile IN0..INn with the caller's op_type
    for arg in &args[1..] {
        compile_expr(emitter, ctx, arg, op_type)?;
    }

    emitter.emit_builtin(func_id);
    Ok(())
}

/// Compiles a type conversion function call (e.g., INT_TO_REAL).
///
/// Unlike generic builtins, conversion functions have different source and
/// target types. The argument is compiled with the source type's OpType,
/// then a conversion opcode (if needed) transforms the value to the target
/// representation.
fn compile_type_conversion(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    source: VarTypeInfo,
    target: VarTypeInfo,
) -> Result<(), Diagnostic> {
    let source_op_type: OpType = (source.op_width, source.signedness);

    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 1 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    compile_expr(emitter, ctx, args[0], source_op_type)?;
    emit_conversion_opcode(emitter, &source, &target);
    emit_truncation(emitter, target);

    Ok(())
}

/// Emits the appropriate conversion BUILTIN opcode for the source->target
/// type transition. Does nothing for same-domain integer conversions that
/// are handled by the Slot's sign-extension and truncation.
fn emit_conversion_opcode(emitter: &mut Emitter, source: &VarTypeInfo, target: &VarTypeInfo) {
    use OpWidth::*;
    use Signedness::*;

    match (
        source.op_width,
        source.signedness,
        target.op_width,
        target.signedness,
    ) {
        // Same OpWidth: no conversion needed (truncation handles sub-width)
        (W32, _, W32, _) | (W64, _, W64, _) => {}

        // W32 signed -> W64: sign extension already in Slot, no-op
        (W32, Signed, W64, _) => {}

        // W32 unsigned -> W64: need zero-extension
        (W32, Unsigned, W64, _) => {
            emitter.emit_builtin(opcode::builtin::CONV_U32_TO_I64);
        }

        // W64 -> W32: as_i32() truncation at store time, no-op
        (W64, _, W32, _) => {}

        // Integer -> Float
        (W32, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F32),
        (W32, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I32_TO_F64),
        (W64, Signed, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F32),
        (W64, Signed, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_I64_TO_F64),
        (W32, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F32),
        (W32, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U32_TO_F64),
        (W64, Unsigned, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F32),
        (W64, Unsigned, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_U64_TO_F64),

        // Float -> Integer
        (F32, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I32),
        (F32, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_I64),
        (F64, _, W32, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I32),
        (F64, _, W64, Signed) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_I64),
        (F32, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U32),
        (F32, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_U64),
        (F64, _, W32, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U32),
        (F64, _, W64, Unsigned) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_U64),

        // Float -> Float
        (F32, _, F64, _) => emitter.emit_builtin(opcode::builtin::CONV_F32_TO_F64),
        (F64, _, F32, _) => emitter.emit_builtin(opcode::builtin::CONV_F64_TO_F32),

        // Same float width (shouldn't happen, but handle gracefully)
        (F32, _, F32, _) | (F64, _, F64, _) => {}
    }
}

/// Compiles a bit shift or rotate function call (SHL, SHR, ROL, ROR).
///
/// Expects two positional arguments: IN (value) and N (shift count).
/// Emits the appropriate BUILTIN opcode based on function name and operand width.
/// For ROL/ROR on narrow types (BYTE, WORD), emits width-specific builtins
/// to ensure bits wrap correctly within the narrow type.
fn compile_shift_rotate(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op_type: OpType,
    name: &str,
) -> Result<(), Diagnostic> {
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    if args.len() != 2 {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    // Compile IN (value) with the inferred op_type
    compile_expr(emitter, ctx, args[0], op_type)?;
    // Compile N (shift count) — always as i32 for W32, i64 for W64
    let n_op_type = match op_type.0 {
        OpWidth::W64 => (OpWidth::W64, Signedness::Signed),
        _ => DEFAULT_OP_TYPE,
    };
    compile_expr(emitter, ctx, args[1], n_op_type)?;

    // Determine storage bits for narrow-type ROL/ROR selection
    let bits = storage_bits(args[0])?;

    let func_id = match (name, op_type.0) {
        ("shl", OpWidth::W64) => opcode::builtin::SHL_I64,
        ("shl", _) => opcode::builtin::SHL_I32,
        ("shr", OpWidth::W64) => opcode::builtin::SHR_I64,
        ("shr", _) => opcode::builtin::SHR_I32,
        ("rol", OpWidth::W64) => opcode::builtin::ROL_I64,
        ("rol", _) => match bits {
            8 => opcode::builtin::ROL_U8,
            16 => opcode::builtin::ROL_U16,
            _ => opcode::builtin::ROL_I32,
        },
        ("ror", OpWidth::W64) => opcode::builtin::ROR_I64,
        ("ror", _) => match bits {
            8 => opcode::builtin::ROR_U8,
            16 => opcode::builtin::ROR_U16,
            _ => opcode::builtin::ROR_I32,
        },
        _ => {
            return Err(Diagnostic::todo_with_span(
                func.name.span(),
                file!(),
                line!(),
            ))
        }
    };

    emitter.emit_builtin(func_id);
    Ok(())
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
                (OpWidth::F32, _) => {
                    // Integer literal in float context: convert to f32.
                    let int_val = if lit.value.is_neg {
                        -(lit.value.value.value as f32)
                    } else {
                        lit.value.value.value as f32
                    };
                    let pool_index = ctx.add_f32_constant(int_val);
                    emitter.emit_load_const_f32(pool_index);
                }
                (OpWidth::F64, _) => {
                    // Integer literal in float context: convert to f64.
                    let int_val = if lit.value.is_neg {
                        -(lit.value.value.value as f64)
                    } else {
                        lit.value.value.value as f64
                    };
                    let pool_index = ctx.add_f64_constant(int_val);
                    emitter.emit_load_const_f64(pool_index);
                }
            }
            Ok(())
        }
        ConstantKind::RealLiteral(lit) => match op_type.0 {
            OpWidth::F32 => {
                let value = lit.value as f32;
                let pool_index = ctx.add_f32_constant(value);
                emitter.emit_load_const_f32(pool_index);
                Ok(())
            }
            OpWidth::F64 => {
                let pool_index = ctx.add_f64_constant(lit.value);
                emitter.emit_load_const_f64(pool_index);
                Ok(())
            }
            _ => Err(Diagnostic::todo(file!(), line!())),
        },
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
        ConstantKind::BitStringLiteral(lit) => {
            let span = lit.value.span();
            match op_type {
                (OpWidth::W32, _) => {
                    let value = u32::try_from(lit.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Bit string literal"),
                        )
                        .with_context("value", &lit.value.value.to_string())
                    })? as i32;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                }
                (OpWidth::W64, _) => {
                    let value = u64::try_from(lit.value.value).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(span.clone(), "Bit string literal"),
                        )
                        .with_context("value", &lit.value.value.to_string())
                    })? as i64;
                    let pool_index = ctx.add_i64_constant(value);
                    emitter.emit_load_const_i64(pool_index);
                }
                (OpWidth::F32, _) => {
                    let value = lit.value.value as f32;
                    let pool_index = ctx.add_f32_constant(value);
                    emitter.emit_load_const_f32(pool_index);
                }
                (OpWidth::F64, _) => {
                    let value = lit.value.value as f64;
                    let pool_index = ctx.add_f64_constant(value);
                    emitter.emit_load_const_f64(pool_index);
                }
            }
            Ok(())
        }
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
        OpWidth::F32 => emitter.emit_load_var_f32(var_index),
        OpWidth::F64 => emitter.emit_load_var_f64(var_index),
    }
}

fn emit_store_var(emitter: &mut Emitter, var_index: u16, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_store_var_i32(var_index),
        OpWidth::W64 => emitter.emit_store_var_i64(var_index),
        OpWidth::F32 => emitter.emit_store_var_f32(var_index),
        OpWidth::F64 => emitter.emit_store_var_f64(var_index),
    }
}

fn emit_add(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_add_i32(),
        OpWidth::W64 => emitter.emit_add_i64(),
        OpWidth::F32 => emitter.emit_add_f32(),
        OpWidth::F64 => emitter.emit_add_f64(),
    }
}

fn emit_sub(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_sub_i32(),
        OpWidth::W64 => emitter.emit_sub_i64(),
        OpWidth::F32 => emitter.emit_sub_f32(),
        OpWidth::F64 => emitter.emit_sub_f64(),
    }
}

fn emit_mul(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_mul_i32(),
        OpWidth::W64 => emitter.emit_mul_i64(),
        OpWidth::F32 => emitter.emit_mul_f32(),
        OpWidth::F64 => emitter.emit_mul_f64(),
    }
}

fn emit_div(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_div_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_div_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_div_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_div_u64(),
        (OpWidth::F32, _) => emitter.emit_div_f32(),
        (OpWidth::F64, _) => emitter.emit_div_f64(),
    }
}

fn emit_mod(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_mod_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_mod_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_mod_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_mod_u64(),
        // Float MOD is not supported (IEC 61131-3 MOD is integer-only).
        // The analyzer should catch this before codegen.
        (OpWidth::F32, _) | (OpWidth::F64, _) => {}
    }
}

fn emit_neg(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_neg_i32(),
        OpWidth::W64 => emitter.emit_neg_i64(),
        OpWidth::F32 => emitter.emit_neg_f32(),
        OpWidth::F64 => emitter.emit_neg_f64(),
    }
}

fn emit_pow(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_builtin(opcode::builtin::EXPT_I32),
        OpWidth::W64 => emitter.emit_builtin(opcode::builtin::EXPT_I64),
        OpWidth::F32 => emitter.emit_builtin(opcode::builtin::EXPT_F32),
        OpWidth::F64 => emitter.emit_builtin(opcode::builtin::EXPT_F64),
    }
}

fn emit_and(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_and_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_and_64(),
        _ => emitter.emit_bool_and(),
    }
}

fn emit_or(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_or_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_or_64(),
        _ => emitter.emit_bool_or(),
    }
}

fn emit_xor(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_bit_xor_32(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_bit_xor_64(),
        _ => emitter.emit_bool_xor(),
    }
}

fn emit_eq(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_eq_i32(),
        OpWidth::W64 => emitter.emit_eq_i64(),
        OpWidth::F32 => emitter.emit_eq_f32(),
        OpWidth::F64 => emitter.emit_eq_f64(),
    }
}

fn emit_ne(emitter: &mut Emitter, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => emitter.emit_ne_i32(),
        OpWidth::W64 => emitter.emit_ne_i64(),
        OpWidth::F32 => emitter.emit_ne_f32(),
        OpWidth::F64 => emitter.emit_ne_f64(),
    }
}

fn emit_lt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_lt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_lt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_lt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_lt_u64(),
        (OpWidth::F32, _) => emitter.emit_lt_f32(),
        (OpWidth::F64, _) => emitter.emit_lt_f64(),
    }
}

fn emit_le(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_le_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_le_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_le_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_le_u64(),
        (OpWidth::F32, _) => emitter.emit_le_f32(),
        (OpWidth::F64, _) => emitter.emit_le_f64(),
    }
}

fn emit_gt(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_gt_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_gt_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_gt_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_gt_u64(),
        (OpWidth::F32, _) => emitter.emit_gt_f32(),
        (OpWidth::F64, _) => emitter.emit_gt_f64(),
    }
}

fn emit_ge(emitter: &mut Emitter, op_type: OpType) {
    match op_type {
        (OpWidth::W32, Signedness::Signed) => emitter.emit_ge_i32(),
        (OpWidth::W32, Signedness::Unsigned) => emitter.emit_ge_u32(),
        (OpWidth::W64, Signedness::Signed) => emitter.emit_ge_i64(),
        (OpWidth::W64, Signedness::Unsigned) => emitter.emit_ge_u64(),
        (OpWidth::F32, _) => emitter.emit_ge_f32(),
        (OpWidth::F64, _) => emitter.emit_ge_f64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::ParseOptions;
    use ironplc_parser::parse_program;

    /// Helper to parse and analyze an IEC 61131-3 program string into a Library.
    ///
    /// Runs the analyzer's type resolution pass so that `Expr.resolved_type` is
    /// populated, which codegen requires for control flow and bitwise operations.
    fn parse(source: &str) -> Library {
        let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
        let (analyzed, _ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
        analyzed
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
        assert_eq!(container.header.num_functions, 2);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);

        // Function 0: init (RET_VOID only, no initial values)
        let init_bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(init_bytecode, &[0xB5]);

        // Function 1: scan — LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0, RET_VOID
        let scan_bytecode = container.code.get_function_bytecode(1).unwrap();
        assert_eq!(scan_bytecode, &[0x01, 0x00, 0x00, 0x18, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn compile_when_no_program_then_p4020_error() {
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
        assert_eq!(diagnostic.code, Problem::NoProgramDeclaration.code());
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

        // Should have two functions (init + scan), both just RET_VOID
        assert_eq!(container.header.num_functions, 2);
        let init_bytecode = container.code.get_function_bytecode(0).unwrap();
        assert_eq!(init_bytecode, &[0xB5]); // RET_VOID only
        let scan_bytecode = container.code.get_function_bytecode(1).unwrap();
        assert_eq!(scan_bytecode, &[0xB5]); // RET_VOID only
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
        assert_eq!(container.header.num_functions, 2);

        // Function 1 (scan):
        // x := 10: LOAD_CONST_I32 pool:0, STORE_VAR_I32 var:0
        // y := x:  LOAD_VAR_I32 var:0, STORE_VAR_I32 var:1
        // RET_VOID
        let bytecode = container.code.get_function_bytecode(1).unwrap();
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

        // Function 1 (scan): LOAD_CONST_I32 pool:0 (-5), STORE_VAR_I32 var:0, RET_VOID
        let bytecode = container.code.get_function_bytecode(1).unwrap();
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

    #[test]
    fn compile_when_exit_outside_loop_then_p4021_error() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
  EXIT;
END_PROGRAM
";
        let library = parse(source);
        let result = compile(&library);

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::ExitOutsideLoop.code());
    }

    #[test]
    fn compile_when_for_non_constant_step_then_p9999_error() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
    s : DINT;
  END_VAR
  FOR x := 1 TO 10 BY s DO
    x := x;
  END_FOR;
END_PROGRAM
";
        let library = parse(source);
        let result = compile(&library);

        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::NotImplemented.code());
    }

    #[test]
    fn compile_when_byte_variable_then_produces_container() {
        let source = "
PROGRAM main
  VAR
    x : BYTE;
  END_VAR
  x := 42;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 42);
    }

    #[test]
    fn compile_when_dword_bit_string_literal_then_loads_constant() {
        let source = "
PROGRAM main
  VAR
    x : DWORD;
  END_VAR
  x := DWORD#16#FF;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.header.num_variables, 1);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 255);
    }
}
