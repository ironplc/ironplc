//! A table for types. The table maintains contexts and a mapping
//! of the name of the type to information about that type.
//!
//! Types are always at a global context.

use std::collections::HashSet;

use ironplc_dsl::{
    common::{Library, TypeName},
    diagnostic::Diagnostic,
    visitor::Visitor,
};

pub fn apply(lib: &Library) -> Result<TypeTable, Vec<Diagnostic>> {
    let mut type_table = TypeTable::new();

    // Walk through the library to discover the types
    let _ = type_table.walk(lib);

    Ok(type_table)
}

#[derive(Debug)]
pub struct TypeTable {
    referenced_types: HashSet<TypeName>,
}

impl TypeTable {
    fn new() -> Self {
        Self {
            referenced_types: HashSet::new(),
        }
    }
}

impl Visitor<()> for TypeTable {
    type Value = ();

    fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, ()> {
        self.referenced_types
            .insert(TypeName::from(node.name.lower_case.as_str()));
        node.recurse_visit(self)
    }
}
