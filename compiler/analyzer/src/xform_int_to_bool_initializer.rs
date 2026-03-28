//! Transform that rewrites integer literal 0/1 initializers on BOOL variables
//! into proper boolean literals (FALSE/TRUE).
//!
//! This is a vendor extension enabled by `--allow-int-to-bool-initializer`
//! (or `--dialect rusty`). It allows patterns like `debug : BOOL := 0;`
//! which are universally supported by CoDeSys, TwinCAT, RuSTy, and
//! virtually every PLC runtime.
//!
//! ## Before
//!
//! ```ignore
//! VAR
//!     debug : BOOL := 0;
//! END_VAR
//! ```
//!
//! ## After
//!
//! ```ignore
//! VAR
//!     debug : BOOL := FALSE;
//! END_VAR
//! ```
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_parser::options::CompilerOptions;

use crate::intermediate_type::IntermediateType;
use crate::type_environment::TypeEnvironment;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
    options: &CompilerOptions,
) -> Result<Library, Vec<Diagnostic>> {
    if !options.allow_int_to_bool_initializer {
        return Ok(lib);
    }
    let mut folder = IntToBoolFolder { type_environment };
    folder.fold_library(lib).map_err(|e| vec![e])
}

struct IntToBoolFolder<'a> {
    type_environment: &'a TypeEnvironment,
}

/// Returns the boolean value if the constant is an integer literal 0 or 1.
fn as_bool_value(constant: &ConstantKind) -> Option<Boolean> {
    if let ConstantKind::IntegerLiteral(lit) = constant {
        if lit.value.is_neg {
            return None;
        }
        match lit.value.value.value {
            0 => Some(Boolean::False),
            1 => Some(Boolean::True),
            _ => None,
        }
    } else {
        None
    }
}

impl Fold<Diagnostic> for IntToBoolFolder<'_> {
    fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        let mut node = VarDecl::recurse_fold(node, self)?;

        if let InitialValueAssignmentKind::Simple(ref mut si) = node.initializer {
            if let Some(ref constant) = si.initial_value {
                let is_bool = self
                    .type_environment
                    .get(&si.type_name)
                    .map(|attrs| matches!(attrs.representation, IntermediateType::Bool))
                    .unwrap_or(false);

                if is_bool {
                    if let Some(bool_val) = as_bool_value(constant) {
                        si.initial_value =
                            Some(ConstantKind::Boolean(BooleanLiteral::new(bool_val)));
                    }
                }
            }
        }

        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use ironplc_parser::options::{CompilerOptions, Dialect};

    fn apply_xform(program: &str) -> Library {
        let (library, mut context) = parse_and_resolve_types_with_context(program);
        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        apply(library, context.types_mut(), &options).unwrap()
    }

    fn get_first_var_initializer(library: &Library) -> ConstantKind {
        use ironplc_dsl::visitor::Visitor;

        struct InitFinder {
            result: Option<ConstantKind>,
        }
        impl Visitor<Diagnostic> for InitFinder {
            type Value = ();
            fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
                if let InitialValueAssignmentKind::Simple(si) = &node.initializer {
                    if let Some(ref c) = si.initial_value {
                        self.result = Some(c.clone());
                    }
                }
                Ok(())
            }
        }

        let mut finder = InitFinder { result: None };
        finder.walk(library).unwrap();
        finder.result.expect("No initializer found")
    }

    #[test]
    fn apply_when_bool_var_with_integer_one_then_rewrites_to_true() {
        let library = apply_xform("PROGRAM main VAR x : BOOL := 1; END_VAR END_PROGRAM");
        let init = get_first_var_initializer(&library);
        assert!(
            matches!(&init, ConstantKind::Boolean(b) if b.value == Boolean::True),
            "Expected Boolean(True), got {:?}",
            init
        );
    }

    #[test]
    fn apply_when_bool_var_with_integer_zero_then_rewrites_to_false() {
        let library = apply_xform("PROGRAM main VAR x : BOOL := 0; END_VAR END_PROGRAM");
        let init = get_first_var_initializer(&library);
        assert!(
            matches!(&init, ConstantKind::Boolean(b) if b.value == Boolean::False),
            "Expected Boolean(False), got {:?}",
            init
        );
    }

    #[test]
    fn apply_when_bool_var_with_integer_two_then_unchanged() {
        let library = apply_xform("PROGRAM main VAR x : BOOL := 2; END_VAR END_PROGRAM");
        let init = get_first_var_initializer(&library);
        assert!(
            matches!(&init, ConstantKind::IntegerLiteral(_)),
            "Expected IntegerLiteral unchanged, got {:?}",
            init
        );
    }

    #[test]
    fn apply_when_int_var_with_integer_one_then_unchanged() {
        let library = apply_xform("PROGRAM main VAR x : INT := 1; END_VAR END_PROGRAM");
        let init = get_first_var_initializer(&library);
        assert!(
            matches!(&init, ConstantKind::IntegerLiteral(_)),
            "Expected IntegerLiteral unchanged, got {:?}",
            init
        );
    }
}
