//! Compiles an IEC 61131-3 AST into a bytecode container.
//!
//! This module walks the AST produced by the parser/analyzer and generates
//! bytecode that the IronPLC VM can execute. The initial implementation
//! supports a minimal subset of the language:
//!
//! - PROGRAM declarations with INT variables
//! - Assignment statements
//! - Integer literal constants
//! - Binary Add operator
//! - Variable references (named symbolic variables)

use std::collections::HashMap;

use ironplc_container::{Container, ContainerBuilder};
use ironplc_dsl::common::{
    ConstantKind, FunctionBlockBodyKind, Library, LibraryElementKind, ProgramDeclaration, VarDecl,
};
use ironplc_dsl::textual::{
    ExprKind, Operator, Statements, StmtKind, SymbolicVariableKind, UnaryOp, Variable,
};

use crate::emit::Emitter;
use crate::error::CodegenError;

/// Compiles a library into a bytecode container.
///
/// Finds the first PROGRAM declaration in the library and compiles it
/// into a container suitable for execution by the VM.
///
/// Returns an error if no program is found or if the program contains
/// unsupported constructs.
pub fn compile(library: &Library) -> Result<Container, CodegenError> {
    let program = find_program(library)?;
    compile_program(program)
}

/// Finds the first PROGRAM declaration in the library.
fn find_program(library: &Library) -> Result<&ProgramDeclaration, CodegenError> {
    for element in &library.elements {
        if let LibraryElementKind::ProgramDeclaration(program) = element {
            return Ok(program);
        }
    }
    Err(CodegenError::Unsupported(
        "no PROGRAM declaration found".to_string(),
    ))
}

/// Compiles a single PROGRAM into a container.
fn compile_program(program: &ProgramDeclaration) -> Result<Container, CodegenError> {
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
    let bytecode = emitter.bytecode();
    let max_stack_depth = emitter.max_stack_depth();

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
    /// Maps variable names (lowercase) to their variable table indices.
    variables: HashMap<String, u16>,
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

    /// Looks up a variable index by name.
    fn var_index(&self, name: &str) -> Result<u16, CodegenError> {
        self.variables
            .get(&name.to_lowercase())
            .copied()
            .ok_or_else(|| CodegenError::UndeclaredVariable(name.to_string()))
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
fn assign_variables(
    ctx: &mut CompileContext,
    declarations: &[VarDecl],
) -> Result<(), CodegenError> {
    for decl in declarations {
        let name = decl.identifier.to_string().to_lowercase();
        let index = ctx.variables.len() as u16;
        ctx.variables.insert(name, index);
    }
    Ok(())
}

/// Compiles a function block body.
fn compile_body(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    body: &FunctionBlockBodyKind,
) -> Result<(), CodegenError> {
    match body {
        FunctionBlockBodyKind::Statements(statements) => {
            compile_statements(emitter, ctx, statements)
        }
        FunctionBlockBodyKind::Empty => Ok(()),
        FunctionBlockBodyKind::Sfc(_) => Err(CodegenError::Unsupported(
            "SFC bodies are not yet supported".to_string(),
        )),
    }
}

/// Compiles a sequence of statements.
fn compile_statements(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    statements: &Statements,
) -> Result<(), CodegenError> {
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
) -> Result<(), CodegenError> {
    match stmt {
        StmtKind::Assignment(assignment) => {
            // Compile the right-hand side expression (pushes value onto stack).
            compile_expr(emitter, ctx, &assignment.value)?;

            // Store into the target variable.
            let var_index = resolve_variable(ctx, &assignment.target)?;
            emitter.emit_store_var_i32(var_index);
            Ok(())
        }
        _ => Err(CodegenError::Unsupported(format!(
            "statement type: {stmt:?}"
        ))),
    }
}

/// Compiles an expression, leaving the result on the stack.
fn compile_expr(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &ExprKind,
) -> Result<(), CodegenError> {
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
                _ => Err(CodegenError::Unsupported(format!(
                    "binary operator: {:?}",
                    binary.op
                ))),
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
                        CodegenError::ConstantOverflow(format!(
                            "integer literal -{} overflows i32",
                            unsigned
                        ))
                    })?;
                    let pool_index = ctx.add_i32_constant(value);
                    emitter.emit_load_const_i32(pool_index);
                    Ok(())
                } else {
                    Err(CodegenError::Unsupported(
                        "unary negation of non-constant expressions".to_string(),
                    ))
                }
            }
            _ => Err(CodegenError::Unsupported(format!(
                "unary operator: {:?}",
                unary.op
            ))),
        },
        ExprKind::LateBound(late_bound) => {
            // LateBound values are unresolved identifiers from the parser.
            // Without the analyzer pass, variable references on the RHS
            // of assignments appear as LateBound. Treat them as variable
            // references.
            let name = late_bound.value.to_string();
            let var_index = ctx.var_index(&name)?;
            emitter.emit_load_var_i32(var_index);
            Ok(())
        }
        ExprKind::Expression(inner) => compile_expr(emitter, ctx, inner),
        _ => Err(CodegenError::Unsupported(format!("expression: {expr:?}"))),
    }
}

/// Compiles a constant literal, pushing it onto the stack.
fn compile_constant(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    constant: &ConstantKind,
) -> Result<(), CodegenError> {
    match constant {
        ConstantKind::IntegerLiteral(lit) => {
            let value = if lit.value.is_neg {
                let unsigned = lit.value.value.value as i128;
                let signed = -unsigned;
                i32::try_from(signed).map_err(|_| {
                    CodegenError::ConstantOverflow(format!(
                        "integer literal {} overflows i32",
                        signed
                    ))
                })?
            } else {
                i32::try_from(lit.value.value.value).map_err(|_| {
                    CodegenError::ConstantOverflow(format!(
                        "integer literal {} overflows i32",
                        lit.value.value.value
                    ))
                })?
            };
            let pool_index = ctx.add_i32_constant(value);
            emitter.emit_load_const_i32(pool_index);
            Ok(())
        }
        _ => Err(CodegenError::Unsupported(format!(
            "constant type: {constant:?}"
        ))),
    }
}

/// Resolves a variable reference to its variable table index.
fn resolve_variable(ctx: &CompileContext, variable: &Variable) -> Result<u16, CodegenError> {
    match variable {
        Variable::Symbolic(symbolic) => match symbolic {
            SymbolicVariableKind::Named(named) => ctx.var_index(&named.name.to_string()),
            _ => Err(CodegenError::Unsupported(format!(
                "variable kind: {symbolic:?}"
            ))),
        },
        Variable::Direct(_) => Err(CodegenError::Unsupported(
            "direct (hardware) variables are not yet supported".to_string(),
        )),
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
    }

    #[test]
    fn compile_when_add_expression_then_produces_add_bytecode() {
        let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
        assert_eq!(container.constant_pool.get_i32(1).unwrap(), 32);
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
    fn compile_when_chain_of_additions_then_correct_bytecode() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1 + 2 + 3;
END_PROGRAM
";
        let library = parse(source);
        let container = compile(&library).unwrap();

        // Should have 3 constants: 1, 2, 3
        assert_eq!(container.constant_pool.len(), 3);
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 1);
        assert_eq!(container.constant_pool.get_i32(1).unwrap(), 2);
        assert_eq!(container.constant_pool.get_i32(2).unwrap(), 3);
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
    }
}
