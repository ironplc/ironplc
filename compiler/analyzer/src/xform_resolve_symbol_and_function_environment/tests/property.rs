//! Properties in the symbol environment.

use super::{analyzed_codes_with, resolve_with_methods};
use crate::{
    symbol_environment::{ScopeKind, ScopePath, SymbolKind},
    test_helpers::fb_inheritance_options,
};
use ironplc_dsl::core::Id;
use ironplc_problems::Problem;

#[test]
fn apply_when_get_and_set_declare_same_local_then_ok() {
    let codes = analyzed_codes_with(
        "
FUNCTION_BLOCK FB_Axis
VAR
    _pos : INT;
END_VAR
PROPERTY Position : INT
GET
VAR
    tmp : INT;
END_VAR
    tmp := _pos;
    Position := tmp;
END_GET
SET
VAR
    tmp : INT;
END_VAR
    tmp := Position;
    _pos := tmp;
END_SET
END_PROPERTY
END_FUNCTION_BLOCK",
        &fb_inheritance_options(),
    );

    assert!(!codes.contains(&Problem::SymbolDeclDuplicated.code().to_string()));
}

#[test]
fn apply_when_property_declared_then_symbol_in_block_scope() {
    let (symbols, _) = resolve_with_methods(
        "
FUNCTION_BLOCK FB_Axis
PROPERTY Position : INT
GET
    Position := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
    );

    let block = ScopeKind::Named(ScopePath::from(Id::from("FB_Axis")));
    let symbol = symbols.find(&Id::from("Position"), &block).unwrap();
    assert_eq!(SymbolKind::Property, symbol.kind);
}

#[test]
fn apply_when_property_and_variable_share_name_then_repeated_declaration() {
    let codes = analyzed_codes_with(
        "
FUNCTION_BLOCK FB_Axis
VAR
    Position : INT;
END_VAR
PROPERTY Position : INT
GET
    Position := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK",
        &fb_inheritance_options(),
    );

    assert!(
        codes.contains(&Problem::SymbolDeclDuplicated.code().to_string()),
        "{codes:?}"
    );
}
