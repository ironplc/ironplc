//! Error types for code generation.

use std::fmt;

/// Errors that can occur during code generation.
#[derive(Debug)]
pub enum CodegenError {
    /// A variable was referenced but not declared.
    UndeclaredVariable(String),
    /// An unsupported AST construct was encountered.
    Unsupported(String),
    /// A constant value overflows the target type.
    ConstantOverflow(String),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::UndeclaredVariable(name) => {
                write!(f, "undeclared variable: {name}")
            }
            CodegenError::Unsupported(msg) => {
                write!(f, "unsupported: {msg}")
            }
            CodegenError::ConstantOverflow(msg) => {
                write!(f, "constant overflow: {msg}")
            }
        }
    }
}

impl std::error::Error for CodegenError {}
