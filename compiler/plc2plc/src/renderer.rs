//! Output writer for IEC61131-3 language elements. The writer transform
//! a parsed library into text.
//!
//! The writer is useful for debugging the parser and to understand the
//! internal representation.

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::{diagnostic::Diagnostic, visitor::Visitor};

pub fn apply(lib: &Library) -> Result<String, Vec<Diagnostic>> {
    let mut visitor = LibraryRenderer::new();
    visitor
        .walk(lib)
        .map(|_| visitor.buffer)
        .map_err(|e| vec![e])
}

struct LibraryRenderer {
    buffer: String,
}

impl LibraryRenderer {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn write(&mut self, val: &str) {
        self.buffer.push_str(val);
    }
}

impl Visitor<Diagnostic> for LibraryRenderer {
    type Value = ();

    fn visit_id(&mut self, node: &Id) -> Result<Self::Value, Diagnostic> {
        self.write(node.original.as_str());
        Ok(())
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        let var_type = match node.var_type {
            VariableType::Var => "VAR",
            VariableType::VarTemp => "VAR_TEMP",
            VariableType::Input => "VAR_INPUT",
            VariableType::Output => "VAR_OUTPUT",
            VariableType::InOut => "VAR_IN_OUT",
            VariableType::External => "VAR_EXTERNAL",
            VariableType::Global => "VAR_GLOBAL",
            VariableType::Access => "VAR_ACCESS",
        };
        self.write(var_type);

        match node.qualifier {
            DeclarationQualifier::Unspecified => {}
            DeclarationQualifier::Constant => self.write("CONSTANT"),
            DeclarationQualifier::Retain => self.write("RETAIN"),
            DeclarationQualifier::NonRetain => self.write("NON_RETAIN"),
        }

        self.write("\n");

        match &node.identifier {
            VariableIdentifier::Symbol(id) => {
                id.recurse_visit(self)?;
            }
            VariableIdentifier::Direct(direct) => {
                direct.recurse_visit(self)?;
            }
        }

        node.recurse_visit(self)?;
        self.write(";\n");

        self.write("END_VAR\n");
        Ok(())
    }

    // 2.1.2.
    fn visit_signed_integer(
        &mut self,
        node: &ironplc_dsl::common::SignedInteger,
    ) -> Result<Self::Value, Diagnostic> {
        if node.is_neg {
            self.write("-");
        }
        self.visit_integer(&node.value)
    }

    // 2.3.3.1
    fn visit_data_type_declaration_kind(
        &mut self,
        node: &DataTypeDeclarationKind,
    ) -> Result<Self::Value, Diagnostic> {
        self.write("TYPE\n");

        node.recurse_visit(self)?;

        self.write("END_TYPE\n");
        Ok(())
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        node.type_name.recurse_visit(self)?;

        node.spec_init.recurse_visit(self)
    }

    // 2.4.2.1
    fn visit_subrange(
        &mut self,
        node: &ironplc_dsl::common::Subrange,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_signed_integer(&node.start)?;
        self.write("..");
        self.visit_signed_integer(&node.end)
    }
}
