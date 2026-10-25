//! Which standard function block a type is.
//!
//! A standard function block (`TON`, `CTU_DINT`, ...) is known by its type,
//! not by its spelling: a user-defined block named like one is a user-defined
//! block. The type environment records the identity on each standard block's
//! type ([`crate::TypeAttributes::standard_block`]), and a backend asks the
//! type rather than matching a name.

// The identity is the lowered program's too, which names the standard block
// a POU stands for; it is defined there and re-exported here.
pub use ironplc_ir::lowered::{CounterWidth, StandardFunctionBlock};

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::TypeName;
    use ironplc_parser::options::CompilerOptions;
    use spec_test_macro::spec_test;

    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_options;

    /// The standard block identity the type environment records for the
    /// type `name`, in a library that declares a user block `CTU_INT`.
    fn identity_of(name: &str) -> Option<StandardFunctionBlock> {
        let (_, context) = parse_and_resolve_types_with_options(
            "
FUNCTION_BLOCK CTU_INT
VAR_INPUT CU : BOOL; END_VAR
VAR_OUTPUT CV : INT; END_VAR
END_FUNCTION_BLOCK",
            &CompilerOptions::default(),
        );
        context
            .types()
            .get(&TypeName::from(name))
            .unwrap()
            .standard_block
    }

    #[spec_test(REQ_CB_analyzer_001)]
    fn standard_block_when_standard_counter_then_records_its_width() {
        assert_eq!(
            Some(StandardFunctionBlock::Ctu(CounterWidth::Int)),
            identity_of("CTU")
        );
        assert_eq!(
            Some(StandardFunctionBlock::Ctu(CounterWidth::Lint)),
            identity_of("ctu_lint")
        );
    }

    #[spec_test(REQ_CB_analyzer_002)]
    fn standard_block_when_user_block_named_like_standard_then_none() {
        assert_eq!(None, identity_of("CTU_INT"));
    }
}
