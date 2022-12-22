//! Semantic rule that variables declared with the CONSTANT
//! storage class must have initial values.
//!
//! ## Passes
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_EXTERNAL CONSTANT
//!       ResetCounterValue : INT = 1;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! ## Fails
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_EXTERNAL CONSTANT
//!       ResetCounterValue : INT = 1;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! ## Todo
//!
//! I don't know if it is possible to have an external
//! reference where one part declares the value and another
//! references the value (and still be constant).
use ironplc_dsl::{dsl::*, visitor::Visitor};

pub fn apply(lib: &Library) -> Result<(), String> {
    let mut visitor = RuleConstantVarsInitialized {};
    visitor.walk(&lib)
}

struct RuleConstantVarsInitialized {}

impl Visitor<String> for RuleConstantVarsInitialized {
    type Value = ();

    fn visit_var_init_decl(&mut self, decl: &VarInitDecl) -> Result<(), String> {
        match decl.storage_class {
            StorageClass::Constant => match decl.initializer {
                Some(_) => {}
                None => {
                    return Err(format!(
                        "Variable is constant but does not define value {} ",
                        decl.name
                    ))
                }
            },
            _ => {}
        }

        Ok(Self::Value::default())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::ast::Id;
    use ironplc_dsl::dsl::*;

    use super::*;

    fn make_declaration_with_var(var: VarInitDecl) -> Library {
        Library {
            elems: vec![LibraryElement::FunctionBlockDeclaration(
                FunctionBlockDeclaration {
                    name: Id::from("CALLEE"),
                    inputs: vec![var],
                    outputs: vec![],
                    inouts: vec![],
                    vars: vec![],
                    externals: vec![],
                    body: FunctionBlockBody::stmts(vec![]),
                },
            )],
        }
    }

    #[test]
    fn apply_when_missing_initializer_then_error() {
        let lib = make_declaration_with_var(VarInitDecl {
            name: Id::from("name"),
            storage_class: StorageClass::Constant,
            initializer: None,
        });

        let result = apply(&lib);

        assert_eq!(true, result.is_err())
    }

    #[test]
    fn apply_when_missing_initializer_then_ok() {
        let lib = make_declaration_with_var(VarInitDecl {
            name: Id::from("name"),
            storage_class: StorageClass::Constant,
            initializer: Some(TypeInitializer::simple(
                "INT",
                Initializer::Simple(Constant::IntegerLiteral(1)),
            )),
        });

        let result = apply(&lib);

        assert_eq!(true, result.is_ok())
    }
}
