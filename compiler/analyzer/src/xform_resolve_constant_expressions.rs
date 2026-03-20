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

/// Stored information about a global integer constant.
#[derive(Debug, Clone)]
struct ConstantInfo {
    value: u128,
}

/// Collect all global constant declarations that have integer-compatible types
/// and literal initializers, then fold the AST replacing `IntegerRef::Constant`
/// and `SignedIntegerRef::Constant` with their resolved literal values.
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
            let type_name_str = simple.type_name.to_string().to_uppercase();
            if !is_integer_type(&type_name_str) {
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

fn is_integer_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "SINT" | "INT" | "DINT" | "LINT" | "USINT" | "UINT" | "UDINT" | "ULINT"
    )
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::parse_program;

    fn parse(src: &str) -> Library {
        parse_program(
            src,
            &FileId::default(),
            &ironplc_parser::options::ParseOptions {
                allow_top_level_var_global: true,
                allow_constant_type_params: true,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn apply_when_constant_in_string_length_then_resolved() {
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

        let result = apply(lib);
        match &result {
            Ok(_) => {}
            Err(diagnostics) => {
                for d in diagnostics {
                    eprintln!("Diagnostic: {:?}", d);
                }
            }
        }
        assert!(result.is_ok());
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
}
