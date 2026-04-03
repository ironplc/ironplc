//! Transform that resolves constant references in type parameters.
//!
//! When a constant identifier is used in place of an integer literal in
//! positions like STRING lengths or array bounds (e.g., `STRING[MY_CONST]`),
//! this pass substitutes the constant's value so downstream analysis sees
//! only concrete integer literals.
//!
//! This is a vendor extension — the IEC 61131-3 standard requires integer
//! literals in these positions. The `--allow-constant-type-params` flag
//! controls whether unresolved references are accepted or rejected.

use std::collections::HashMap;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_problems::Problem;

/// Stored information about an integer constant.
#[derive(Debug, Clone)]
struct ConstantInfo {
    value: u128,
}

/// Collect global and POU-local constant declarations that have integer-compatible
/// types and literal initializers, then fold the AST replacing `IntegerRef::Constant`
/// and `SignedIntegerRef::Constant` with their resolved literal values.
///
/// Global constants are collected upfront and available everywhere. POU-local
/// constants (VAR CONSTANT inside FUNCTION, FUNCTION_BLOCK, or PROGRAM) are
/// scoped — they are only visible within their declaring POU.
pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    let constants = collect_constants(&lib);

    let mut resolver = ConstantResolver {
        constants,
        diagnostics: vec![],
    };

    let result = resolver.fold_library(lib).map_err(|e| vec![e]);

    if !resolver.diagnostics.is_empty() {
        return Err(resolver.diagnostics);
    }

    result
}

/// Scan the library for global constant declarations with integer values.
fn collect_constants(lib: &Library) -> HashMap<String, ConstantInfo> {
    let mut constants = HashMap::new();

    for element in &lib.elements {
        match element {
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                collect_from_var_decls(decls, &mut constants);
            }
            LibraryElementKind::ConfigurationDeclaration(config) => {
                collect_from_var_decls(&config.global_var, &mut constants);
                for resource in &config.resource_decl {
                    collect_from_var_decls(&resource.global_vars, &mut constants);
                }
            }
            _ => {}
        }
    }

    constants
}

fn collect_from_var_decls(decls: &[VarDecl], constants: &mut HashMap<String, ConstantInfo>) {
    for decl in decls {
        if decl.qualifier != DeclarationQualifier::Constant {
            continue;
        }

        let name = match &decl.identifier {
            VariableIdentifier::Symbol(id) => id.clone(),
            VariableIdentifier::Direct(d) => match &d.name {
                Some(name) => name.clone(),
                None => continue,
            },
        };

        // Check if the type is integer-compatible and has an integer initializer
        if let Some(value) = extract_integer_value(&decl.initializer) {
            constants.insert(name.to_string().to_uppercase(), ConstantInfo { value });
        }
    }
}

/// Extract an integer value from an initializer, if it has one.
fn extract_integer_value(init: &InitialValueAssignmentKind) -> Option<u128> {
    match init {
        InitialValueAssignmentKind::Simple(simple) => {
            // Check that the type is integer-compatible
            let is_integer = ElementaryTypeName::try_from(&simple.type_name.name)
                .map(|t| t.is_integer())
                .unwrap_or(false);
            if !is_integer {
                return None;
            }

            match &simple.initial_value {
                Some(ConstantKind::IntegerLiteral(lit)) => {
                    if lit.value.is_neg {
                        // Negative values can't be used for string lengths or array sizes
                        // but we'll store them anyway for array lower bounds
                        None
                    } else {
                        Some(lit.value.value.value)
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

struct ConstantResolver {
    constants: HashMap<String, ConstantInfo>,
    diagnostics: Vec<Diagnostic>,
}

impl<E> Fold<E> for ConstantResolver {
    fn fold_integer_ref(&mut self, node: IntegerRef) -> Result<IntegerRef, E> {
        match node {
            IntegerRef::Literal(_) => Ok(node),
            IntegerRef::Constant(ref id) => {
                let name = id.to_string().to_uppercase();
                match self.constants.get(&name) {
                    Some(info) => Ok(IntegerRef::Literal(Integer {
                        value: info.value,
                        span: id.span(),
                    })),
                    None => {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::UndefinedConstantReference,
                            Label::span(id.span(), format!("Constant '{}' is not defined", id)),
                        ));
                        // Return the node as-is; the diagnostic will cause an error
                        Ok(node)
                    }
                }
            }
        }
    }

    fn fold_signed_integer_ref(&mut self, node: SignedIntegerRef) -> Result<SignedIntegerRef, E> {
        match node {
            SignedIntegerRef::Literal(_) => Ok(node),
            SignedIntegerRef::Constant(ref id) => {
                let name = id.to_string().to_uppercase();
                match self.constants.get(&name) {
                    Some(info) => Ok(SignedIntegerRef::Literal(SignedInteger {
                        value: Integer {
                            value: info.value,
                            span: id.span(),
                        },
                        is_neg: false,
                    })),
                    None => {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::UndefinedConstantReference,
                            Label::span(id.span(), format!("Constant '{}' is not defined", id)),
                        ));
                        Ok(node)
                    }
                }
            }
        }
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, E> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }

    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, E> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, E> {
        let saved = self.constants.clone();
        collect_from_var_decls(&node.variables, &mut self.constants);
        let result = node.recurse_fold(self);
        self.constants = saved;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::parse_program;
    use ironplc_test::cast;

    fn parse(src: &str) -> Library {
        parse_program(
            src,
            &FileId::default(),
            &ironplc_parser::options::CompilerOptions {
                allow_top_level_var_global: true,
                allow_constant_type_params: true,
                ..Default::default()
            },
        )
        .unwrap()
    }

    /// Find the first VarDecl with the given name from a POU.
    fn find_var_decl<'a>(lib: &'a Library, var_name: &str) -> &'a VarDecl {
        for element in &lib.elements {
            let vars = match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => &fb.variables,
                LibraryElementKind::FunctionDeclaration(f) => &f.variables,
                LibraryElementKind::ProgramDeclaration(p) => &p.variables,
                _ => continue,
            };
            for var in vars {
                if var.identifier.to_string().eq_ignore_ascii_case(var_name) {
                    return var;
                }
            }
        }
        panic!("Variable '{}' not found", var_name);
    }

    /// Extract the string length from a variable's initializer.
    fn get_string_length(var: &VarDecl) -> u128 {
        let s = cast!(&var.initializer, InitialValueAssignmentKind::String);
        let lit = cast!(s.length.as_ref().unwrap(), IntegerRef::Literal);
        lit.value
    }

    /// Extract array subranges from a variable's initializer.
    fn get_array_subranges(var: &VarDecl) -> &[Subrange] {
        let arr = cast!(&var.initializer, InitialValueAssignmentKind::Array);
        let sub = cast!(&arr.spec, ArraySpecificationKind::Inline);
        &sub.ranges
    }

    fn signed_integer_ref_value(r: &SignedIntegerRef) -> (bool, u128) {
        let si = cast!(r, SignedIntegerRef::Literal);
        (si.is_neg, si.value.value)
    }

    #[test]
    fn apply_when_constant_in_string_length_then_resolves_to_value() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                STRING_LENGTH : INT := 250;
            END_VAR
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[STRING_LENGTH];
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "STR");
        assert_eq!(get_string_length(var), 250);
    }

    #[test]
    fn apply_when_constant_in_array_bounds_then_resolves_to_value() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                ARRAY_SIZE : INT := 10;
            END_VAR
            FUNCTION_BLOCK fb1
            VAR_INPUT
                ARR : ARRAY[1..ARRAY_SIZE] OF INT;
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "ARR");
        let subranges = get_array_subranges(var);
        assert_eq!(subranges.len(), 1);
        assert_eq!(signed_integer_ref_value(&subranges[0].start), (false, 1));
        assert_eq!(signed_integer_ref_value(&subranges[0].end), (false, 10));
    }

    #[test]
    fn apply_when_non_integer_type_constant_then_not_collected() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                MY_REAL : REAL := 3.14;
            END_VAR
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[MY_REAL];
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let result = apply(lib);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_undefined_constant_then_error() {
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[UNDEFINED_CONST];
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let result = apply(lib);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_multiple_constants_then_all_resolved() {
        let lib = parse(
            "
            VAR_GLOBAL CONSTANT
                LEN : UINT := 100;
                SIZE : DINT := 5;
            END_VAR
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[LEN];
                ARR : ARRAY[1..SIZE] OF INT;
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let lib = apply(lib).unwrap();
        assert_eq!(get_string_length(find_var_decl(&lib, "STR")), 100);

        let subranges = get_array_subranges(find_var_decl(&lib, "ARR"));
        assert_eq!(signed_integer_ref_value(&subranges[0].end), (false, 5));
    }

    #[test]
    fn apply_when_var_global_not_constant_then_error() {
        let lib = parse(
            "
            VAR_GLOBAL
                NOT_A_CONST : INT := 100;
            END_VAR
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[NOT_A_CONST];
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let result = apply(lib);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_no_constants_referenced_then_unchanged() {
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR_INPUT
                STR : STRING[50];
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "STR");
        assert_eq!(get_string_length(var), 50);
    }

    #[test]
    fn apply_when_fb_local_constant_in_array_bounds_then_resolves() {
        let lib = parse(
            "
            FUNCTION_BLOCK MY_FIFO
            VAR
                fifo : ARRAY[0..n] OF DWORD;
            END_VAR
            VAR CONSTANT
                n : INT := 16;
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "fifo");
        let subranges = get_array_subranges(var);
        assert_eq!(subranges.len(), 1);
        assert_eq!(signed_integer_ref_value(&subranges[0].start), (false, 0));
        assert_eq!(signed_integer_ref_value(&subranges[0].end), (false, 16));
    }

    #[test]
    fn apply_when_function_local_constant_in_string_length_then_resolves() {
        let lib = parse(
            "
            FUNCTION my_func : INT
            VAR_INPUT
                STR : STRING[MAX_LEN];
            END_VAR
            VAR CONSTANT
                MAX_LEN : INT := 80;
            END_VAR
            my_func := 0;
            END_FUNCTION
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "STR");
        assert_eq!(get_string_length(var), 80);
    }

    #[test]
    fn apply_when_program_local_constant_in_array_bounds_then_resolves() {
        let lib = parse(
            "
            PROGRAM my_prog
            VAR
                buf : ARRAY[1..SIZE] OF INT;
            END_VAR
            VAR CONSTANT
                SIZE : INT := 32;
            END_VAR
            END_PROGRAM
        ",
        );

        let lib = apply(lib).unwrap();
        let var = find_var_decl(&lib, "buf");
        let subranges = get_array_subranges(var);
        assert_eq!(subranges.len(), 1);
        assert_eq!(signed_integer_ref_value(&subranges[0].start), (false, 1));
        assert_eq!(signed_integer_ref_value(&subranges[0].end), (false, 32));
    }

    #[test]
    fn apply_when_fb_local_constant_not_visible_in_other_fb_then_error() {
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR CONSTANT
                LOCAL_SIZE : INT := 10;
            END_VAR
            END_FUNCTION_BLOCK
            FUNCTION_BLOCK fb2
            VAR
                arr : ARRAY[1..LOCAL_SIZE] OF INT;
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let result = apply(lib);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_fb_var_not_constant_then_error() {
        let lib = parse(
            "
            FUNCTION_BLOCK fb1
            VAR
                NOT_CONST : INT := 10;
                arr : ARRAY[1..NOT_CONST] OF INT;
            END_VAR
            END_FUNCTION_BLOCK
        ",
        );

        let result = apply(lib);
        assert!(result.is_err());
    }
}
