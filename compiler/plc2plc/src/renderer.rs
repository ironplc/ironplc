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

    fn write_char(&mut self, val: char) {
        self.buffer.push(val);
    }

    fn write_ws(&mut self, val: &str) {
        if !self.buffer.ends_with(' ') {
            self.buffer.push(' ');
        }
        self.buffer.push_str(val);
    }

    fn newline(&mut self) {
        self.buffer.push('\n');
    }
}

impl Visitor<Diagnostic> for LibraryRenderer {
    type Value = ();

    fn visit_id(&mut self, node: &Id) -> Result<Self::Value, Diagnostic> {
        // TODO this is the wrong case
        self.write_ws(node.original().as_str());
        Ok(())
    }

    fn visit_integer(&mut self, node: &Integer) -> Result<Self::Value, Diagnostic> {
        self.write_ws(node.value.to_string().as_str());
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
        self.write_ws("TYPE");
        self.newline();

        node.recurse_visit(self)?;

        self.write_ws("END_TYPE");
        self.newline();
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

    // 2.4.3
    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        self.newline();

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
        self.write_ws(var_type);

        match node.qualifier {
            DeclarationQualifier::Unspecified => {}
            DeclarationQualifier::Constant => self.write_ws("CONSTANT"),
            DeclarationQualifier::Retain => self.write_ws("RETAIN"),
            DeclarationQualifier::NonRetain => self.write_ws("NON_RETAIN"),
        }

        self.newline();

        match &node.identifier {
            VariableIdentifier::Symbol(id) => {
                self.visit_id(id)?;
            }
            VariableIdentifier::Direct(direct) => {
                self.visit_direct_variable_identifier(direct)?;
            }
        }

        self.write_ws(":");
        self.visit_initial_value_assignment_kind(&node.initializer)?;

        self.write(";");
        self.newline();

        self.write_ws("END_VAR");
        self.newline();
        Ok(())
    }

    // 2.4.3.1
    fn visit_address_assignment(
        &mut self,
        node: &AddressAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_char('%');

        let loc = match &node.location {
            LocationPrefix::I => 'I',
            LocationPrefix::Q => 'Q',
            LocationPrefix::M => 'M',
        };
        self.write_char(loc);

        let size = match &node.size {
            // TODO
            SizePrefix::Unspecified => ' ',
            SizePrefix::Nil => todo!(),
            SizePrefix::X => 'X',
            SizePrefix::B => 'B',
            SizePrefix::W => 'W',
            SizePrefix::D => 'D',
            SizePrefix::L => 'L',
        };
        self.write_char(size);

        for idx in &node.address {
            self.write(idx.to_string().as_str());
        }

        Ok(())
    }

    // 2.4.3.2
    fn visit_simple_initializer(
        &mut self,
        node: &SimpleInitializer,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws(&node.type_name.to_string());

        match &node.initial_value {
            Some(iv) => {
                self.write_ws(":=");
                self.visit_constant_kind(iv)?;
            }
            None => {}
        }

        Ok(())
    }

    fn visit_direct_variable_identifier(
        &mut self,
        node: &DirectVariableIdentifier,
    ) -> Result<Self::Value, Diagnostic> {
        match &node.name {
            Some(name) => self.visit_id(name)?,
            None => {}
        }

        self.write_ws("AT");

        self.visit_address_assignment(&node.address_assignment)?;

        Ok(())
    }

    // 2.5.1
    fn visit_function_declaration(
        &mut self,
        _node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("FUNCTION");

        self.write_ws("END_FUNCTION");
        self.newline();
        Ok(())
    }

    // 2.5.1
    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("FUNCTION_BLOCK");

        node.name.recurse_visit(self)?;
        self.newline();

        for var in node.variables.iter() {
            var.recurse_visit(self)?;
        }

        node.body.recurse_visit(self)?;

        self.write_ws("END_FUNCTION_BLOCK");
        self.newline();
        Ok(())
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("PROGRAM");

        self.visit_id(&node.type_name)?;
        self.newline();

        for var in node.variables.iter() {
            self.visit_var_decl(var)?;
        }

        node.body.recurse_visit(self)?;

        self.write_ws("END_PROGRAM");
        self.newline();
        Ok(())
    }
}
