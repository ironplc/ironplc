//! The block a function block `EXTENDS`, in the symbol environment.

use crate::{
    semantic_context::SemanticContext,
    symbol_environment::{ScopeKind, ScopePath},
    test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options},
};
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::SelfRefKind;

fn resolve(program: &str) -> SemanticContext {
    parse_and_resolve_types_with_options(program, &fb_inheritance_options()).1
}

/// The scope of the method `method` of the block `block`.
fn method_scope(block: &str, method: &str) -> ScopeKind {
    ScopeKind::Named(ScopePath::new(vec![Id::from(block), Id::from(method)]))
}

#[test]
fn apply_when_base_declared_then_super_names_it() {
    let context = resolve(
        "
FUNCTION_BLOCK FB_Base
VAR x : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_A EXTENDS FB_Base
METHOD Run
END_METHOD
END_FUNCTION_BLOCK",
    );
    let symbols = context.symbols();

    assert_eq!(
        Some(TypeName::from("FB_Base")),
        symbols.self_type(&method_scope("FB_A", "Run"), SelfRefKind::Super)
    );
    assert!(symbols
        .inherited_member(&Id::from("x"), &Id::from("FB_A"))
        .is_some());
}

#[test]
fn apply_when_base_not_declared_then_super_names_no_block() {
    let context = resolve(
        "
FUNCTION_BLOCK FB_A EXTENDS FB_Missing
METHOD Run
END_METHOD
END_FUNCTION_BLOCK",
    );

    assert_eq!(
        None,
        context
            .symbols()
            .self_type(&method_scope("FB_A", "Run"), SelfRefKind::Super)
    );
}

#[test]
fn apply_when_base_is_structure_then_its_fields_are_not_inherited() {
    let context = resolve(
        "
TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE
FUNCTION_BLOCK FB_A EXTENDS Pair
END_FUNCTION_BLOCK",
    );
    let symbols = context.symbols();

    assert_eq!(
        None,
        symbols.self_type(
            &ScopeKind::Named(ScopePath::new(vec![Id::from("FB_A")])),
            SelfRefKind::Super
        )
    );
    assert!(symbols
        .inherited_member(&Id::from("f"), &Id::from("FB_A"))
        .is_none());
}
