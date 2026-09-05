//! A declaration that owns variables must say whether it scopes them.
//!
//! This proves the guard is wired into the derive itself, which the unit
//! tests over `check_scope_declared` cannot: they would still pass if the
//! call from `recurse_macro_derive` were removed.

use dsl_macro_derive::Recurse;

struct VarDecl;

#[derive(Recurse)]
struct PouDeclaration {
    pub variables: Vec<VarDecl>,
}

fn main() {}
