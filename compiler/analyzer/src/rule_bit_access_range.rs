//! Semantic rule that checks bit access indices are within the valid
//! range for the variable's declared type.
//!
//! See section B.1.4.2.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB1
//!    VAR
//!       myWord : WORD;
//!       myBool : BOOL;
//!    END_VAR
//!    myBool := myWord.0;
//!    myBool := myWord.15;
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB1
//!    VAR
//!       myByte : BYTE;
//!       myBool : BOOL;
//!    END_VAR
//!    myBool := myByte.8;
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashMap;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleBitAccessRange {
        type_environment: context.types(),
        var_types: HashMap::new(),
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleBitAccessRange<'a> {
    type_environment: &'a crate::type_environment::TypeEnvironment,
    /// Maps variable names to their declared type names within the current scope.
    var_types: HashMap<Id, TypeName>,
    diagnostics: Vec<Diagnostic>,
}

impl RuleBitAccessRange<'_> {
    fn collect_var_types(&mut self, variables: &[VarDecl]) {
        for var in variables {
            if let Some(type_name) = extract_type_name(&var.initializer) {
                if let VariableIdentifier::Symbol(id) = &var.identifier {
                    self.var_types.insert(id.clone(), type_name);
                }
            }
        }
    }

    fn check_bit_access(&mut self, node: &BitAccessVariable) {
        // Extract the innermost named variable
        let var_name = match innermost_named_variable(&node.variable) {
            Some(name) => name,
            None => return,
        };

        // Look up the declared type
        let type_name = match self.var_types.get(var_name) {
            Some(tn) => tn,
            None => return,
        };

        // Get type attributes and determine bit width
        let type_attrs = match self.type_environment.get(type_name) {
            Some(attrs) => attrs,
            None => return,
        };

        let bit_width = match type_attrs.representation.size_in_bytes() {
            Some(bytes) => u128::from(bytes) * 8,
            None => return,
        };

        let index = node.index.value;
        if index >= bit_width {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::BitAccessOutOfRange,
                    Label::span(
                        node.index.span(),
                        format!(
                            "Bit index {} is out of range. Valid range is 0..{} for type {}",
                            index,
                            bit_width - 1,
                            type_name
                        ),
                    ),
                )
                .with_context("index", &index.to_string())
                .with_context("type", &type_name.to_string())
                .with_context("max_bit", &(bit_width - 1).to_string()),
            );
        }
    }
}

/// Extracts the type name from an initializer declaration.
fn extract_type_name(init: &InitialValueAssignmentKind) -> Option<TypeName> {
    match init {
        InitialValueAssignmentKind::Simple(si) => Some(si.type_name.clone()),
        InitialValueAssignmentKind::LateResolvedType(tn) => Some(tn.clone()),
        _ => None,
    }
}

/// Extracts the innermost named variable from a (possibly nested) symbolic variable.
fn innermost_named_variable(kind: &SymbolicVariableKind) -> Option<&Id> {
    match kind {
        SymbolicVariableKind::Named(named) => Some(&named.name),
        SymbolicVariableKind::Array(array) => innermost_named_variable(&array.subscripted_variable),
        SymbolicVariableKind::Structured(structured) => {
            innermost_named_variable(&structured.record)
        }
        SymbolicVariableKind::BitAccess(bit_access) => {
            innermost_named_variable(&bit_access.variable)
        }
    }
}

impl Visitor<Diagnostic> for RuleBitAccessRange<'_> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.var_types.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_types.clear();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.var_types.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_types.clear();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.var_types.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_types.clear();
        ret
    }

    fn visit_bit_access_variable(&mut self, node: &BitAccessVariable) -> Result<(), Diagnostic> {
        self.check_bit_access(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::ParseOptions, parse_program};

    use super::*;

    fn assert_bit_access_ok(program: &str) {
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok(), "Expected OK but got: {:?}", result);
    }

    fn assert_bit_access_err(program: &str) {
        let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
        let result = analyze(&[&library]);
        let (_library, context) = result.unwrap();
        assert!(
            context.has_diagnostics(),
            "Expected diagnostics but got none"
        );
    }

    // --- BYTE (8 bits): valid range 0..7 ---

    #[test]
    fn apply_when_byte_bit_0_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : BYTE;
    y : BOOL;
END_VAR
    y := x.0;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_byte_bit_7_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : BYTE;
    y : BOOL;
END_VAR
    y := x.7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_byte_bit_8_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : BYTE;
    y : BOOL;
END_VAR
    y := x.8;
END_FUNCTION_BLOCK",
        );
    }

    // --- WORD (16 bits): valid range 0..15 ---

    #[test]
    fn apply_when_word_bit_15_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : WORD;
    y : BOOL;
END_VAR
    y := x.15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_word_bit_16_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : WORD;
    y : BOOL;
END_VAR
    y := x.16;
END_FUNCTION_BLOCK",
        );
    }

    // --- DWORD (32 bits): valid range 0..31 ---

    #[test]
    fn apply_when_dword_bit_31_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : DWORD;
    y : BOOL;
END_VAR
    y := x.31;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_dword_bit_32_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : DWORD;
    y : BOOL;
END_VAR
    y := x.32;
END_FUNCTION_BLOCK",
        );
    }

    // --- LWORD (64 bits): valid range 0..63 ---

    #[test]
    fn apply_when_lword_bit_63_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : LWORD;
    y : BOOL;
END_VAR
    y := x.63;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_lword_bit_64_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : LWORD;
    y : BOOL;
END_VAR
    y := x.64;
END_FUNCTION_BLOCK",
        );
    }

    // --- SINT (8 bits): valid range 0..7 ---

    #[test]
    fn apply_when_sint_bit_7_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : SINT;
    y : BOOL;
END_VAR
    y := x.7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_sint_bit_8_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : SINT;
    y : BOOL;
END_VAR
    y := x.8;
END_FUNCTION_BLOCK",
        );
    }

    // --- INT (16 bits): valid range 0..15 ---

    #[test]
    fn apply_when_int_bit_15_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x.15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_int_bit_16_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x.16;
END_FUNCTION_BLOCK",
        );
    }

    // --- DINT (32 bits): valid range 0..31 ---

    #[test]
    fn apply_when_dint_bit_31_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : DINT;
    y : BOOL;
END_VAR
    y := x.31;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_dint_bit_32_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : DINT;
    y : BOOL;
END_VAR
    y := x.32;
END_FUNCTION_BLOCK",
        );
    }

    // --- LINT (64 bits): valid range 0..63 ---

    #[test]
    fn apply_when_lint_bit_63_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : LINT;
    y : BOOL;
END_VAR
    y := x.63;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_lint_bit_64_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : LINT;
    y : BOOL;
END_VAR
    y := x.64;
END_FUNCTION_BLOCK",
        );
    }

    // --- USINT (8 bits): valid range 0..7 ---

    #[test]
    fn apply_when_usint_bit_7_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : USINT;
    y : BOOL;
END_VAR
    y := x.7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_usint_bit_8_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : USINT;
    y : BOOL;
END_VAR
    y := x.8;
END_FUNCTION_BLOCK",
        );
    }

    // --- UINT (16 bits): valid range 0..15 ---

    #[test]
    fn apply_when_uint_bit_15_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : UINT;
    y : BOOL;
END_VAR
    y := x.15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_uint_bit_16_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : UINT;
    y : BOOL;
END_VAR
    y := x.16;
END_FUNCTION_BLOCK",
        );
    }

    // --- UDINT (32 bits): valid range 0..31 ---

    #[test]
    fn apply_when_udint_bit_31_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : UDINT;
    y : BOOL;
END_VAR
    y := x.31;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_udint_bit_32_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : UDINT;
    y : BOOL;
END_VAR
    y := x.32;
END_FUNCTION_BLOCK",
        );
    }

    // --- ULINT (64 bits): valid range 0..63 ---

    #[test]
    fn apply_when_ulint_bit_63_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : ULINT;
    y : BOOL;
END_VAR
    y := x.63;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_ulint_bit_64_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : ULINT;
    y : BOOL;
END_VAR
    y := x.64;
END_FUNCTION_BLOCK",
        );
    }

    // --- Bit access on assignment target ---

    #[test]
    fn apply_when_bit_access_target_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : WORD;
    y : BOOL;
END_VAR
    x.0 := y;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_bit_access_target_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : BYTE;
    y : BOOL;
END_VAR
    x.8 := y;
END_FUNCTION_BLOCK",
        );
    }
}
