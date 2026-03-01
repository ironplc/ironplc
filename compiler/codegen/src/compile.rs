//! Compiles an IEC 61131-3 AST into a bytecode container.
//!
//! This module walks the AST produced by the parser/analyzer and generates
//! bytecode that the IronPLC VM can execute. The initial implementation
//! supports a minimal subset of the language:
//!
//! - PROGRAM declarations with INT variables
//! - Assignment statements
//! - Integer literal constants
//! - Boolean literal constants (TRUE, FALSE)
//! - Binary Add, Sub, Mul, Div, Mod, and Pow operators
//! - Unary Neg and Not operators
//! - Comparison operators (=, <>, <, <=, >, >=)
//! - Boolean operators (AND, OR, XOR, NOT)
//! - Variable references (named symbolic variables)
//! - IF/ELSIF/ELSE statements
//! - WHILE, FOR, and REPEAT iteration statements

use std::collections::HashMap;

use ironplc_container::{opcode, Container, ContainerBuilder};
use ironplc_dsl::common::{
    Boolean, ConstantKind, FunctionBlockBodyKind, Library, LibraryElementKind, ProgramDeclaration,
    VarDecl,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    CompareOp, ExprKind, Operator, Statements, StmtKind, SymbolicVariableKind, UnaryOp, Variable,
};
use ironplc_problems::Problem;

use crate::emit::Emitter;

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
        builder = builder.add_i32_constant(*constant);
    }

    builder = builder.add_function(0, bytecode, max_stack_depth, num_locals);

    Ok(builder.build())
}

/// Tracks state during compilation of a single program.
struct CompileContext {
    /// Maps variable identifiers to their variable table indices.
    variables: HashMap<Id, u16>,
    /// Ordered list of i32 constants added to the constant pool.
    constants: Vec<i32>,
}

impl CompileContext {
    fn new() -> Self {
        CompileContext {
            variables: HashMap::new(),
            constants: Vec::new(),
        }
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

    /// Adds an i32 constant to the pool and returns its index.
    fn add_i32_constant(&mut self, value: i32) -> u16 {
        // Check if this constant already exists.
        for (i, existing) in self.constants.iter().enumerate() {
            if *existing == value {
                return i as u16;
            }
        }
        let index = self.constants.len() as u16;
        self.constants.push(value);
        index
    }
}

/// Assigns variable table indices for all variable declarations.
fn assign_variables(ctx: &mut CompileContext, declarations: &[VarDecl]) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            let index = ctx.variables.len() as u16;
            ctx.variables.insert(id.clone(), index);
        }
    }
    Ok(())
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
            // Compile the right-hand side expression (pushes value onto stack).
            compile_expr(emitter, ctx, &assignment.value)?;

            // Store into the target variable.
            let var_index = resolve_variable(ctx, &assignment.target)?;
            emitter.emit_store_var_i32(var_index);
            Ok(())
        }
        StmtKind::FbCall(fb_call) => {
            Err(Diagnostic::todo_with_span(fb_call.span(), file!(), line!()))
        }
        StmtKind::If(if_stmt) => compile_if(emitter, ctx, if_stmt),
        StmtKind::Case(case_stmt) => Err(Diagnostic::todo_with_span(
            expr_span(&case_stmt.selector),
            file!(),
            line!(),
        )),
        StmtKind::For(for_stmt) => compile_for(emitter, ctx, for_stmt),
        StmtKind::While(while_stmt) => compile_while(emitter, ctx, while_stmt),
        StmtKind::Repeat(repeat_stmt) => compile_repeat(emitter, ctx, repeat_stmt),
        StmtKind::Return => Err(Diagnostic::todo(file!(), line!())),
        StmtKind::Exit => Err(Diagnostic::todo(file!(), line!())),
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

    // Compile the IF condition.
    compile_expr(emitter, ctx, &if_stmt.expr)?;

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
        compile_expr(emitter, ctx, &elsif.expr)?;
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
    compile_expr(emitter, ctx, &while_stmt.condition)?;
    emitter.emit_jmp_if_not(end_label);
    compile_stmts(emitter, ctx, &while_stmt.body)?;
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

    emitter.bind_label(loop_label);
    compile_stmts(emitter, ctx, &repeat_stmt.body)?;
    compile_expr(emitter, ctx, &repeat_stmt.until)?;
    emitter.emit_jmp_if_not(loop_label);

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

    // Determine step sign.
    let step_sign = match &for_stmt.step {
        None => StepSign::Positive,
        Some(step_expr) => match try_constant_sign(step_expr) {
            Some(sign) => sign,
            None => return Err(Diagnostic::todo(file!(), line!())),
        },
    };

    // Initialize: compile(from), STORE_VAR control
    compile_expr(emitter, ctx, &for_stmt.from)?;
    emitter.emit_store_var_i32(var_index);

    let loop_label = emitter.create_label();
    let body_label = emitter.create_label();
    let end_label = emitter.create_label();

    // LOOP: check termination condition
    emitter.bind_label(loop_label);
    emitter.emit_load_var_i32(var_index);
    compile_expr(emitter, ctx, &for_stmt.to)?;
    match step_sign {
        StepSign::Positive => emitter.emit_gt_i32(),
        StepSign::Negative => emitter.emit_lt_i32(),
    }
    emitter.emit_jmp_if_not(body_label);
    emitter.emit_jmp(end_label);

    // BODY:
    emitter.bind_label(body_label);
    compile_stmts(emitter, ctx, &for_stmt.body)?;

    // Increment: LOAD_VAR control, compile(step), ADD_I32, STORE_VAR control
    emitter.emit_load_var_i32(var_index);
    match &for_stmt.step {
        Some(step_expr) => compile_expr(emitter, ctx, step_expr)?,
        None => {
            let one_index = ctx.add_i32_constant(1);
            emitter.emit_load_const_i32(one_index);
        }
    }
    emitter.emit_add_i32();
    emitter.emit_store_var_i32(var_index);
    emitter.emit_jmp(loop_label);

    // END:
    emitter.bind_label(end_label);

    Ok(())
}

/// Compiles an expression, leaving the result on the stack.
fn compile_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &ExprKind,
) -> Result<(), Diagnostic> {
    match expr {
        ExprKind::Const(constant) => compile_constant(emitter, ctx, constant),
        ExprKind::Variable(variable) => {
            let var_index = resolve_variable(ctx, variable)?;
            emitter.emit_load_var_i32(var_index);
            Ok(())
        }
        ExprKind::BinaryOp(binary) => {
            compile_expr(emitter, ctx, &binary.left)?;
            compile_expr(emitter, ctx, &binary.right)?;
            match binary.op {
                Operator::Add => {
                    emitter.emit_add_i32();
                    Ok(())
                }
                Operator::Sub => {
                    emitter.emit_sub_i32();
                    Ok(())
                }
                Operator::Mul => {
                    emitter.emit_mul_i32();
                    Ok(())
                }
                Operator::Div => {
                    emitter.emit_div_i32();
                    Ok(())
                }
                Operator::Mod => {
                    emitter.emit_mod_i32();
                    Ok(())
                }
                Operator::Pow => {
                    emitter.emit_builtin(opcode::builtin::EXPT_I32);
                    Ok(())
                }
            }
        }
        ExprKind::UnaryOp(unary) => match unary.op {
            UnaryOp::Neg => {
                // Constant folding: if the operand is an integer literal,
                // emit the negated constant directly.
                if let ExprKind::Const(ConstantKind::IntegerLiteral(lit)) = &unary.term {
                    let unsigned = lit.value.value.value as i128;
                    let signed = -unsigned;
                    let value = i32::try_from(signed).map_err(|_| {
                        Diagnostic::problem(
                            Problem::ConstantOverflow,
                            Label::span(lit.value.value.span(), "Integer literal"),
                        )
                        .with_context("value", &format!("-{}", unsigned))
                    })?;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                    Ok(())
                } else {
                    compile_expr(emitter, ctx, &unary.term)?;
                    emitter.emit_neg_i32();
                    Ok(())
                }
            }
            UnaryOp::Not => {
                compile_expr(emitter, ctx, &unary.term)?;
                emitter.emit_bool_not();
                Ok(())
            }
        },
        ExprKind::LateBound(late_bound) => {
            // LateBound values are unresolved identifiers from the parser.
            // Without the analyzer pass, variable references on the RHS
            // of assignments appear as LateBound. Treat them as variable
            // references.
            let var_index = ctx.var_index(&late_bound.value)?;
            emitter.emit_load_var_i32(var_index);
            Ok(())
        }
        ExprKind::Expression(inner) => compile_expr(emitter, ctx, inner),
        ExprKind::Compare(compare) => {
            compile_expr(emitter, ctx, &compare.left)?;
            compile_expr(emitter, ctx, &compare.right)?;
            match compare.op {
                CompareOp::Eq => emitter.emit_eq_i32(),
                CompareOp::Ne => emitter.emit_ne_i32(),
                CompareOp::Lt => emitter.emit_lt_i32(),
                CompareOp::Gt => emitter.emit_gt_i32(),
                CompareOp::LtEq => emitter.emit_le_i32(),
                CompareOp::GtEq => emitter.emit_ge_i32(),
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
) -> Result<(), Diagnostic> {
    match constant {
        ConstantKind::IntegerLiteral(lit) => {
            let span = lit.value.value.span();
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

/// Extracts a source span from an expression for error reporting.
///
/// Attempts to find the most specific span available in the expression tree.
/// Falls back to `SourceSpan::default()` for expressions that lack span info.
fn expr_span(expr: &ExprKind) -> SourceSpan {
    match expr {
        ExprKind::Variable(Variable::Symbolic(sym)) => sym.span(),
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => lit.value.value.span(),
        ExprKind::LateBound(late) => late.value.span(),
        ExprKind::Function(func) => func.name.span(),
        ExprKind::EnumeratedValue(e) => e.span(),
        ExprKind::BinaryOp(binary) => expr_span(&binary.left),
        ExprKind::UnaryOp(unary) => expr_span(&unary.term),
        ExprKind::Compare(compare) => expr_span(&compare.left),
        ExprKind::Expression(inner) => expr_span(inner),
        _ => SourceSpan::default(),
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
    x : INT;
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
    x : INT;
    y : INT;
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
    x : INT;
    y : INT;
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
    x : INT;
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
    x : INT;
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
