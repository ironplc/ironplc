//! Semantic rule that variables declared with the CONSTANT
//! storage class must have initial values.
//!
//! ## Passes
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR CONSTANT
//!       ResetCounterValue : INT := 1;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! ## Fails
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR CONSTANT
//!       ResetCounterValue : INT;
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
        println!("Storage class {:?}", decl.storage_class);
        match decl.storage_class {
            StorageClass::Constant => {
                println!("Initializer {:?}", decl.initializer);
                match &decl.initializer {
                    TypeInitializer::Simple { type_name: _, initial_value } => {
                        match initial_value {
                            Some(_) => {},
                            None => {
                                return Err(format!(
                                    "Variable is constant but does not define value {} ",
                                    decl.name
                                ))
                            },
                        }
                    },
                    TypeInitializer::EnumeratedValues { values: _, default } => {
                        match default {
                            Some(_) => {},
                            None => {
                                return Err(format!(
                                    "Variable is constant but does not define value {} ",
                                    decl.name
                                ))
                            },
                        }
                    },
                    TypeInitializer::EnumeratedType(type_init) => {
                        match type_init.initial_value {
                            Some(_) => {},
                            None => {
                                return Err(format!(
                                    "Variable is constant but does not define value {} ",
                                    decl.name
                                ))
                            },
                        }
                    },
                    TypeInitializer::FunctionBlock { type_name: _ } => todo!(),
                    TypeInitializer::Structure { type_name: _ } => todo!(),
                    TypeInitializer::LateResolvedType(_) => todo!(),
                }
            },
            _ => {}
        }

        Ok(Self::Value::default())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_simple_type_missing_initializer_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : INT;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err())
    }

    #[test]
    fn apply_when_enum_type_missing_initializer_then_error() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err())
    }

    #[test]
    fn apply_when_missing_initializer_then_ok() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : INT := 1;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok())
    }
}
