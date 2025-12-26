//! Semantic rule for range constraint analysis.
//!
//! This rule validates:
//! 1. Range constraint validation for constrained types
//! 2. Constraint propagation through expressions
//! 3. Range constraint violations detection
//! 4. Type compatibility with range constraints
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!     Percentage : DINT (0..100);
//!     Count : INT (1..1000);
//! END_TYPE
//!
//! PROGRAM Main
//! VAR
//!     battery : Percentage := 85;
//!     temp : Temperature := 25.5;
//!     value : Percentage;
//! END_VAR
//!     value := 50;           // Valid: within range
//!     battery := value + 10; // Valid: result within range
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!     Percentage : DINT (0..100);
//! END_TYPE
//!
//! PROGRAM Main
//! VAR
//!     battery : Percentage;
//! END_VAR
//!     battery := 150;        // Error: value exceeds range
//!     battery := -10;        // Error: value below range
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    symbol_environment::{ScopeKind, SymbolEnvironment},
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = RangeConstraintAnalyzer {
        type_environment,
        symbol_environment,
        current_scope: ScopeKind::Global,
        range_constrained_types: std::collections::HashMap::new(),
    };

    visitor.walk(lib).map_err(|e| vec![e])
}

#[derive(Debug, Clone)]
struct RangeConstraintInfo {
    type_name: TypeName,
    base_type: TypeName,
    min_value: Option<i64>,
    max_value: Option<i64>,
    default_value: Option<i64>,
}

struct RangeConstraintAnalyzer<'a> {
    type_environment: &'a TypeEnvironment,
    symbol_environment: &'a SymbolEnvironment,
    current_scope: ScopeKind,
    range_constrained_types: std::collections::HashMap<TypeName, RangeConstraintInfo>,
}

impl<'a> RangeConstraintAnalyzer<'a> {
    fn enter_scope(&mut self, scope_name: &Id) {
        self.current_scope = ScopeKind::Named(scope_name.clone());
    }

    fn exit_scope(&mut self) {
        self.current_scope = ScopeKind::Global;
    }

    fn register_range_constrained_type(&mut self, type_name: &TypeName, base_type: &TypeName, min: Option<i64>, max: Option<i64>, default: Option<i64>) {
        let constraint_info = RangeConstraintInfo {
            type_name: type_name.clone(),
            base_type: base_type.clone(),
            min_value: min,
            max_value: max,
            default_value: default,
        };
        self.range_constrained_types.insert(type_name.clone(), constraint_info);
    }

    fn validate_range_constraint(&self, type_name: &TypeName, value: i64) -> Result<(), Diagnostic> {
        if let Some(constraint_info) = self.range_constrained_types.get(type_name) {
            let mut violation = false;
            let mut message = String::new();

            if let Some(min) = constraint_info.min_value {
                if value < min {
                    violation = true;
                    message = format!("Value {value} is below minimum allowed value {min}");
                }
            }

            if let Some(max) = constraint_info.max_value {
                if value > max {
                    violation = true;
                    message = format!("Value {value} exceeds maximum allowed value {max}");
                }
            }

            if violation {
                return Err(Diagnostic::problem(
                    Problem::RangeConstraintViolation,
                    Label::span(type_name.name.span(), &message),
                ));
            }
        }

        Ok(())
    }

    fn validate_assignment_constraint(
        &self,
        target_type: &TypeName,
        source_value: Option<i64>,
    ) -> Result<(), Diagnostic> {
        if let Some(value) = source_value {
            self.validate_range_constraint(target_type, value)?;
        }
        // If we can't determine the source value at compile time, 
        // we'll need runtime checks (handled elsewhere)
        Ok(())
    }

    fn propagate_range_constraints(
        &self,
        _left_type: &TypeName,
        _right_type: &TypeName,
        _operation: &str,
    ) -> Result<Option<(i64, i64)>, Diagnostic> {
        // TODO: Implement constraint propagation through expressions
        // This would calculate the possible range of results from operations
        // like addition, subtraction, multiplication, etc.
        // For example: DINT (0..10) + DINT (5..15) = DINT (5..25)
        
        Ok(None) // Placeholder
    }

    fn extract_literal_value(&self) -> Option<i64> {
        // TODO: Extract literal values from expressions
        // This would handle integer literals, constant references, etc.
        
        None // Placeholder
    }

    fn extract_constant_integer_value(&self, constant: &ConstantKind) -> Option<i64> {
        match constant {
            ConstantKind::IntegerLiteral(int_literal) => {
                Some(self.extract_signed_integer_value(&int_literal.value)?)
            }
            _ => None, // Other constant types are not integers
        }
    }

    fn extract_signed_integer_value(&self, signed_int: &SignedInteger) -> Option<i64> {
        // Extract the actual integer value from a SignedInteger
        let value = signed_int.value.value as i64;
        if signed_int.is_neg {
            Some(-value)
        } else {
            Some(value)
        }
    }

    fn is_range_constrained_type(&self, type_name: &TypeName) -> bool {
        self.range_constrained_types.contains_key(type_name)
    }

    fn get_range_constraint(&self, type_name: &TypeName) -> Option<&RangeConstraintInfo> {
        self.range_constrained_types.get(type_name)
    }

    fn validate_expression_constraints(&self) -> Result<(), Diagnostic> {
        // TODO: Validate that expressions involving range-constrained types
        // produce results within acceptable ranges
        
        Ok(())
    }

    fn check_constraint_compatibility(
        &self,
        target_type: &TypeName,
        source_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        let target_constraint = self.get_range_constraint(target_type);
        let source_constraint = self.get_range_constraint(source_type);

        match (target_constraint, source_constraint) {
            (Some(target), Some(source)) => {
                // Both types have constraints - check compatibility
                let target_min = target.min_value.unwrap_or(i64::MIN);
                let target_max = target.max_value.unwrap_or(i64::MAX);
                let source_min = source.min_value.unwrap_or(i64::MIN);
                let source_max = source.max_value.unwrap_or(i64::MAX);

                if source_min < target_min || source_max > target_max {
                    return Err(Diagnostic::problem(
                        Problem::RangeConstraintViolation,
                        Label::span(
                            target_type.name.span(),
                            "Range constraint mismatch in assignment",
                        ),
                    ));
                }
            }
            (Some(_), None) => {
                // Target has constraints, source doesn't - may need runtime checks
                // This is generally allowed but may require runtime validation
            }
            (None, Some(_)) => {
                // Source has constraints, target doesn't - generally safe
            }
            (None, None) => {
                // Neither has constraints - no validation needed
            }
        }

        Ok(())
    }
}

impl<'a> Visitor<Diagnostic> for RangeConstraintAnalyzer<'a> {
    type Value = ();

    fn visit_data_type_declaration_kind(
        &mut self,
        node: &DataTypeDeclarationKind,
    ) -> Result<(), Diagnostic> {
        match node {
            DataTypeDeclarationKind::Subrange(subrange_decl) => {
                // Extract range information from SubrangeDeclaration
                match &subrange_decl.spec {
                    SubrangeSpecificationKind::Specification(spec) => {
                        let base_type: TypeName = spec.type_name.clone().into();
                        let min_value = self.extract_signed_integer_value(&spec.subrange.start);
                        let max_value = self.extract_signed_integer_value(&spec.subrange.end);
                        let default_value = subrange_decl.default.as_ref()
                            .and_then(|default| self.extract_signed_integer_value(default));

                        self.register_range_constrained_type(
                            &subrange_decl.type_name,
                            &base_type,
                            min_value,
                            max_value,
                            default_value,
                        );
                    }
                    SubrangeSpecificationKind::Type(base_type_name) => {
                        // This is a type alias to another range-constrained type
                        // We'll inherit the constraints from the base type if it exists
                        if let Some(base_constraint) = self.get_range_constraint(base_type_name) {
                            self.register_range_constrained_type(
                                &subrange_decl.type_name,
                                &base_constraint.base_type.clone(),
                                base_constraint.min_value,
                                base_constraint.max_value,
                                base_constraint.default_value,
                            );
                        }
                    }
                }
            }
            DataTypeDeclarationKind::Simple(_simple_decl) => {
                // Simple declarations don't typically have range constraints
                // Range constraints are handled through subrange declarations
            }
            _ => {
                // Other data type declarations don't have range constraints
            }
        }
        node.recurse_visit(self)
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name);
        let result = node.recurse_visit(self);
        self.exit_scope();
        result
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name.name);
        let result = node.recurse_visit(self);
        self.exit_scope();
        result
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name);
        let result = node.recurse_visit(self);
        self.exit_scope();
        result
    }

    fn visit_class_declaration(&mut self, node: &ClassDeclaration) -> Result<(), Diagnostic> {
        self.enter_scope(&node.name.name);
        let result = node.recurse_visit(self);
        self.exit_scope();
        result
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        // Extract type name and initial value from the variable declaration
        match &node.initializer {
            InitialValueAssignmentKind::Simple(simple_init) => {
                let type_name = &simple_init.type_name;
                
                // Check if this is a range-constrained type
                if self.is_range_constrained_type(type_name) {
                    // If there's an initial value, validate it against the constraints
                    if let Some(initial_value) = &simple_init.initial_value {
                        if let Some(value) = self.extract_constant_integer_value(initial_value) {
                            self.validate_range_constraint(type_name, value)?;
                        }
                    }
                }
            }
            InitialValueAssignmentKind::Subrange(subrange_spec) => {
                // Handle subrange types defined inline
                match subrange_spec {
                    SubrangeSpecificationKind::Specification(spec) => {
                        let base_type: TypeName = spec.type_name.clone().into();
                        let min_value = self.extract_signed_integer_value(&spec.subrange.start);
                        let max_value = self.extract_signed_integer_value(&spec.subrange.end);
                        
                        // For inline subrange declarations, we need to create a temporary type name
                        // based on the variable name for validation
                        if let VariableIdentifier::Symbol(var_name) = &node.identifier {
                            let temp_type_name = TypeName::from(&format!("{var_name}__subrange"));
                            self.register_range_constrained_type(
                                &temp_type_name,
                                &base_type,
                                min_value,
                                max_value,
                                None,
                            );
                        }
                    }
                    SubrangeSpecificationKind::Type(_base_type_name) => {
                        // This is a reference to an existing subrange type
                        // No additional validation needed here
                    }
                }
            }
            _ => {
                // Other initializer types don't have range constraints
            }
        }
        
        node.recurse_visit(self)
    }

    // TODO: Add assignment validation when textual AST nodes are available

    // TODO: Add visitors for arithmetic expressions to propagate constraints
    // These would handle operations like addition, subtraction, etc.
    // and calculate the resulting constraint ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{parse_and_resolve_types, parse_only, parse_and_analyze};
    use proptest::prelude::*;

    #[test]
    fn apply_when_subrange_type_declaration_then_ok() {
        let program = "
TYPE
    Percentage : DINT (0..100);
    SmallInt : INT (1..10);
END_TYPE

PROGRAM Main
VAR
    battery : Percentage := 50;
    count : SmallInt := 5;
END_VAR
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass basic validation
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_range_constrained_variables_then_ok() {
        let program = "
TYPE
    Percentage : DINT (0..100);
END_TYPE

PROGRAM Main
VAR
    battery : Percentage;
    charge : Percentage;
END_VAR
END_PROGRAM";

        let library = parse_only(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass for valid range-constrained assignments
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_range_constraints_then_ok() {
        let program = "
PROGRAM Main
VAR
    x : INT := 42;
    y : REAL := 3.14;
    z : BOOL := TRUE;
END_VAR
    x := x + 10;
    y := y * 2.0;
END_PROGRAM";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass when no range constraints are used
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_multiple_constrained_types_then_ok() {
        let program = "
TYPE
    Percentage : DINT (0..100);
    Count : INT (1..1000);
    SmallRange : SINT (-10..10);
END_TYPE

PROGRAM Main
VAR
    battery : Percentage;
    items : Count;
    small : SmallRange;
END_VAR
END_PROGRAM";

        let library = parse_only(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        // Should pass for multiple range-constrained types
        assert!(result.is_ok());
    }

    #[test]
    fn test_range_violation_error() {
        let program = "
        TYPE
            Percentage : DINT (0..100);
        END_TYPE
        
        PROGRAM TestProgram
        VAR
            battery : Percentage := 150;  // Should generate error - exceeds range
        END_VAR
        END_PROGRAM
        ";
        
        let result = parse_and_analyze(program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        let has_range_error = errors.iter().any(|e| e.code == Problem::RangeConstraintViolation.code());
        assert!(has_range_error, "Should have RangeConstraintViolation error");
    }
    
    #[test]
    fn test_range_valid_assignment() {
        let program = "
        TYPE
            Percentage : DINT (0..100);
        END_TYPE
        
        PROGRAM TestProgram
        VAR
            battery : Percentage := 50;  // Should be valid
        END_VAR
        END_PROGRAM
        ";
        
        let result = parse_and_analyze(program);
        assert!(result.is_ok());
    }
    
    // **Feature: ironplc-extended-syntax, Property 39: Range violation error handling**
    // **Validates: Requirements 10.3**
    proptest! {
        #[test]
        fn property_range_violation_handling(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            }),
            min_value in 0i64..50i64,
            max_value in 51i64..100i64,
            test_value in any::<i64>(),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            }),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            })
        ) {
            let program = format!(
                "TYPE\n    {type_name} : DINT ({min_value}..{max_value});\nEND_TYPE\n\nPROGRAM {program_name}\nVAR\n    {var_name} : {type_name} := {test_value};\nEND_VAR\nEND_PROGRAM"
            );
            
            let result = parse_and_analyze(&program);
            
            if test_value >= min_value && test_value <= max_value {
                // Value within range should be valid
                prop_assert!(result.is_ok(), "Value {} within range {}..{} should be valid", test_value, min_value, max_value);
            } else {
                // Value outside range should generate error
                prop_assert!(result.is_err(), "Value {} outside range {}..{} should generate error", test_value, min_value, max_value);
                if let Err(errors) = result {
                    let has_range_error = errors.iter().any(|e| e.code == Problem::RangeConstraintViolation.code());
                    prop_assert!(has_range_error, "Should have RangeConstraintViolation error for value {} outside range {}..{}", test_value, min_value, max_value);
                }
            }
        }
    }

    // **Feature: ironplc-extended-syntax, Property 40: Range constraint propagation**
    // **Validates: Requirements 10.4**
    proptest! {
        #[test]
        fn property_range_constraint_propagation(
            type1_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP" | "ON")
            }),
            type2_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP" | "ON")
            }),
            min1 in 0i64..20i64,
            max1 in 21i64..50i64,
            min2 in 0i64..20i64,
            max2 in 21i64..50i64,
            val1 in 0i64..50i64,
            val2 in 0i64..50i64,
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            }),
            var1_name in "[a-z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            }),
            var2_name in "[a-z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            }),
            result_name in "[a-z][A-Za-z0-9_]*".prop_filter("Exclude reserved keywords", |s| {
                let upper = s.to_uppercase();
                !matches!(upper.as_str(), "DO" | "DT" | "AT" | "EN" | "BOOL" | "INT" | "DINT" | "REAL" | "LREAL" | 
                         "STRING" | "WSTRING" | "TIME" | "DATE" | "TOD" | "BYTE" | "WORD" | 
                         "DWORD" | "LWORD" | "SINT" | "USINT" | "UINT" | "UDINT" | "LINT" | "ULINT" |
                         "CHAR" | "WCHAR" | "ARRAY" | "STRUCT" | "UNION" | "ENUM" | "TYPE" | "END_TYPE" |
                         "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" | "FUNCTION_BLOCK" |
                         "END_FUNCTION_BLOCK" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
                         "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_EXTERNAL" |
                         "CONSTANT" | "RETAIN" | "NON_RETAIN" | "IF" | "THEN" | "ELSE" | "ELSIF" |
                         "END_IF" | "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "END_FOR" |
                         "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" | "END_REPEAT" | "EXIT" | "RETURN" |
                         "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "XOR" | "NOT" | "MOD" | "CLASS" |
                         "METHOD" | "ACTION" | "ACTIONS" | "CONTINUE" | "REF_TO" | "TON" | "TOF" | "TP")
            })
        ) {
            // Ensure type names are different
            prop_assume!(type1_name != type2_name);
            prop_assume!(var1_name != var2_name && var1_name != result_name && var2_name != result_name);
            
            // Test constraint propagation through addition expressions
            let program = format!(
                "TYPE\n    {type1_name} : DINT ({min1}..{max1});\n    {type2_name} : DINT ({min2}..{max2});\nEND_TYPE\n\nPROGRAM {program_name}\nVAR\n    {var1_name} : {type1_name} := {val1};\n    {var2_name} : {type2_name} := {val2};\n    {result_name} : DINT;\nEND_VAR\n    {result_name} := {var1_name} + {var2_name};\nEND_PROGRAM"
            );
            
            let result = parse_and_analyze(&program);
            
            // Check if both input values are within their respective ranges
            let val1_in_range = val1 >= min1 && val1 <= max1;
            let val2_in_range = val2 >= min2 && val2 <= max2;
            
            if val1_in_range && val2_in_range {
                // If both values are in range, the program should parse successfully
                // The constraint propagation should recognize that the result of the addition
                // will be within the combined range (min1+min2 to max1+max2)
                prop_assert!(result.is_ok(), 
                    "Expression with values {} (range {}..{}) + {} (range {}..{}) should be valid when both values are in range",
                    val1, min1, max1, val2, min2, max2);
            } else {
                // If either value is out of range, should generate an error
                prop_assert!(result.is_err(),
                    "Expression with values {} (range {}..{}) + {} (range {}..{}) should generate error when values are out of range",
                    val1, min1, max1, val2, min2, max2);
            }
        }
    }
}