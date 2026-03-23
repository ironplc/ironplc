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

use crate::{
    intermediate_type::IntermediateType, result::SemanticResult, semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleBitAccessRange {
        type_environment: context.types(),
        var_initializers: HashMap::new(),
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
    /// Maps variable names to their declared initializers within the current scope.
    var_initializers: HashMap<Id, InitialValueAssignmentKind>,
    diagnostics: Vec<Diagnostic>,
}

impl RuleBitAccessRange<'_> {
    fn collect_var_types(&mut self, variables: &[VarDecl]) {
        for var in variables {
            if let VariableIdentifier::Symbol(id) = &var.identifier {
                self.var_initializers
                    .insert(id.clone(), var.initializer.clone());
            }
        }
    }

    fn check_bit_access(&mut self, node: &BitAccessVariable) {
        // Resolve the type of the variable being bit-accessed
        let resolved_type = match resolve_variable_type(
            &node.variable,
            &self.var_initializers,
            self.type_environment,
        ) {
            Some(t) => t,
            None => return,
        };

        let bit_width = match resolved_type.size_in_bytes() {
            Some(bytes) => bytes as u128 * 8,
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
                            "Bit index {} is out of range. Valid range is 0..{} for type",
                            index,
                            bit_width - 1,
                        ),
                    ),
                )
                .with_context("index", &index.to_string())
                .with_context("max_bit", &(bit_width - 1).to_string()),
            );
        }
    }
}

/// Resolves the `IntermediateType` for a named variable from its initializer.
fn resolve_initializer_type(
    init: &InitialValueAssignmentKind,
    type_env: &TypeEnvironment,
) -> Option<IntermediateType> {
    match init {
        InitialValueAssignmentKind::Simple(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::LateResolvedType(tn) => {
            Some(type_env.get(tn)?.representation.clone())
        }
        InitialValueAssignmentKind::Structure(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::Array(ai) => match &ai.spec {
            SpecificationKind::Named(tn) => Some(type_env.get(tn)?.representation.clone()),
            SpecificationKind::Inline(subranges) => {
                let element_type = type_env.get(&subranges.type_name)?.representation.clone();
                Some(IntermediateType::Array {
                    element_type: Box::new(element_type),
                    dimensions: vec![],
                })
            }
        },
        _ => None,
    }
}

/// Resolves the `IntermediateType` for a `SymbolicVariableKind` by walking through
/// struct field accesses and array subscripts to find the type of the leaf expression.
fn resolve_variable_type(
    kind: &SymbolicVariableKind,
    var_initializers: &HashMap<Id, InitialValueAssignmentKind>,
    type_env: &TypeEnvironment,
) -> Option<IntermediateType> {
    match kind {
        SymbolicVariableKind::Named(named) => {
            let init = var_initializers.get(&named.name)?;
            resolve_initializer_type(init, type_env)
        }
        SymbolicVariableKind::Structured(structured) => {
            let record_type =
                resolve_variable_type(&structured.record, var_initializers, type_env)?;
            find_struct_field_type(&record_type, &structured.field)
        }
        SymbolicVariableKind::Array(array) => {
            let array_type =
                resolve_variable_type(&array.subscripted_variable, var_initializers, type_env)?;
            match array_type {
                IntermediateType::Array { element_type, .. } => Some(*element_type),
                _ => None,
            }
        }
        SymbolicVariableKind::BitAccess(bit_access) => {
            resolve_variable_type(&bit_access.variable, var_initializers, type_env)
        }
        SymbolicVariableKind::Deref(deref) => {
            resolve_variable_type(&deref.variable, var_initializers, type_env)
        }
    }
}

/// Finds the type of a field within a structure or function block type.
fn find_struct_field_type(
    parent_type: &IntermediateType,
    field_name: &Id,
) -> Option<IntermediateType> {
    let fields = match parent_type {
        IntermediateType::Structure { fields } => fields,
        IntermediateType::FunctionBlock { fields, .. } => fields,
        _ => return None,
    };
    fields
        .iter()
        .find(|f| f.name == *field_name)
        .map(|f| f.field_type.clone())
}

impl Visitor<Diagnostic> for RuleBitAccessRange<'_> {
    type Value = ();

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.var_initializers.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_initializers.clear();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.var_initializers.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_initializers.clear();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.var_initializers.clear();
        self.collect_var_types(&node.variables);
        let ret = node.recurse_visit(self);
        self.var_initializers.clear();
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

    // --- Struct field bit access ---

    #[test]
    fn apply_when_struct_field_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "TYPE
    MyStruct : STRUCT
        field1 : BYTE;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_field_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "TYPE
    MyStruct : STRUCT
        field1 : BYTE;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.8;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_word_field_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "TYPE
    MyStruct : STRUCT
        field1 : WORD;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_word_field_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "TYPE
    MyStruct : STRUCT
        field1 : WORD;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.16;
END_FUNCTION_BLOCK",
        );
    }

    // --- Array element bit access ---

    #[test]
    fn apply_when_array_element_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF BYTE;
    y : BOOL;
END_VAR
    y := arr[0].7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_element_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF BYTE;
    y : BOOL;
END_VAR
    y := arr[0].8;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_word_element_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF WORD;
    y : BOOL;
END_VAR
    y := arr[1].15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_word_element_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF WORD;
    y : BOOL;
END_VAR
    y := arr[1].16;
END_FUNCTION_BLOCK",
        );
    }

    // --- Bit access in FUNCTION (not FUNCTION_BLOCK) ---

    #[test]
    fn apply_when_function_dint_bit_access_then_ok() {
        assert_bit_access_ok(
            "FUNCTION FOO : INT
VAR_INPUT
    A : DINT;
END_VAR
    IF A.0 THEN
        FOO := 1;
    END_IF;
END_FUNCTION

PROGRAM test_bit_func
VAR
    result : INT;
END_VAR
    result := FOO(A := 5);
END_PROGRAM",
        );
    }
}
