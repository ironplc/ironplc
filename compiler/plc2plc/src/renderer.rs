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
    indents: usize,
}

impl LibraryRenderer {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            indents: 0,
        }
    }

    fn write(&mut self, val: &str) {
        if self.buffer.ends_with('\n') {
            self.buffer.push_str("   ".repeat(self.indents).as_str());
        }
        self.buffer.push_str(val);
    }

    fn write_ws(&mut self, val: &str) {
        if self.buffer.ends_with('\n') {
            self.buffer.push_str("   ".repeat(self.indents).as_str());
        } else if !self.buffer.ends_with(' ') {
            self.buffer.push(' ');
        }
        self.buffer.push_str(val);
    }

    fn newline(&mut self) {
        self.buffer.push('\n');
    }

    fn indent(&mut self) {
        self.indents += 1;
    }

    fn outdent(&mut self) {
        self.indents -= 1;
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

        self.indent();
        node.recurse_visit(self)?;
        self.outdent();
        self.newline();

        self.write_ws("END_TYPE");
        self.newline();
        Ok(())
    }

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        self.write_ws(":");

        self.visit_enumerated_specification_init(&node.spec_init)
    }

    fn visit_enumerated_specification_init(
        &mut self,
        node: &EnumeratedSpecificationInit,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_enumerated_specification_kind(&node.spec)?;

        if let Some(default) = &node.default {
            self.write_ws(":=");
            self.visit_enumerated_value(default)?;
        }

        Ok(())
    }

    fn visit_enumerated_specification_values(
        &mut self,
        node: &EnumeratedSpecificationValues,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("(");

        let mut it = node.values.iter().peekable();
        while let Some(val) = it.next() {
            self.visit_enumerated_value(val)?;
            if it.peek().is_some() {
                self.write_ws(",");
            }
        }

        self.write_ws(")");
        Ok(())
    }

    // 2.3.3.1
    fn visit_array_subranges(&mut self, node: &ArraySubranges) -> Result<Self::Value, Diagnostic> {
        self.write_ws("ARRAY");

        for range in node.ranges.iter() {
            self.visit_subrange(range)?;
            self.write_ws(", ");
        }

        self.write_ws("OF");

        self.visit_id(&node.type_name)
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

        self.indent();
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
        self.outdent();

        self.write_ws("END_VAR");
        self.newline();
        Ok(())
    }

    // 2.4.3.1
    fn visit_address_assignment(
        &mut self,
        node: &AddressAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        let mut address = String::from("%");

        let loc = match &node.location {
            LocationPrefix::I => 'I',
            LocationPrefix::Q => 'Q',
            LocationPrefix::M => 'M',
        };
        address.push(loc);

        let size = match &node.size {
            // TODO
            SizePrefix::Unspecified => '*',
            SizePrefix::Nil => todo!(),
            SizePrefix::X => 'X',
            SizePrefix::B => 'B',
            SizePrefix::W => 'W',
            SizePrefix::D => 'D',
            SizePrefix::L => 'L',
        };
        address.push(size);

        let location: String = node
            .address
            .iter()
            .map(|&id| id.to_string() + ".")
            .collect();
        address.push_str(location.trim_end_matches('.'));

        self.write_ws(address.as_str());

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
        self.visit_id(&node.name)?;
        self.newline();

        self.indent();
        for var in node.variables.iter() {
            self.visit_var_decl(var)?;
        }

        node.body.recurse_visit(self)?;
        self.outdent();
        self.newline();

        self.write_ws("END_FUNCTION_BLOCK");
        self.newline();
        Ok(())
    }

    // 2.5.3
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

    // 2.7.1
    fn visit_resource_declaration(
        &mut self,
        node: &dsl::configuration::ResourceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("RESOURCE");
        self.visit_id(&node.name)?;
        self.write_ws("ON");
        self.visit_id(&node.resource)?;
        self.newline();

        self.indent();
        for task in node.tasks.iter() {
            self.visit_task_configuration(task)?;
        }

        for program in node.programs.iter() {
            self.visit_program_configuration(program)?;
        }

        for var in node.global_vars.iter() {
            self.visit_var_decl(var)?;
        }

        self.outdent();

        self.write_ws("END_RESOURCE");
        self.newline();
        Ok(())
    }

    // 2.7.2
    fn visit_program_configuration(
        &mut self,
        node: &dsl::configuration::ProgramConfiguration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("PROGRAM");
        self.visit_id(&node.name)?;

        if let Some(task) = &node.task_name {
            self.write_ws("WITH");
            self.visit_id(task)?;
        }

        self.write_ws(":");
        self.visit_id(&node.type_name)?;

        self.write_ws(";");
        self.newline();

        Ok(())
    }

    // 2.7.2
    fn visit_configuration_declaration(
        &mut self,
        node: &dsl::configuration::ConfigurationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("CONFIGURATION");
        self.visit_id(&node.name)?;
        self.newline();

        self.indent();
        for res in node.resource_decl.iter() {
            self.visit_resource_declaration(res)?;
        }
        self.outdent();

        self.write_ws("END_CONFIGURATION");
        self.newline();
        Ok(())
    }

    // 2.7.2
    fn visit_task_configuration(
        &mut self,
        node: &dsl::configuration::TaskConfiguration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("TASK");

        self.visit_id(&node.name)?;
        self.write_ws("(");

        if let Some(_interval) = node.interval {
            self.write_ws("INTERNAL");
            self.write_ws(":=");
            // TODO visit duration
            self.write_ws(",");
        }

        self.write_ws("PRIORITY");
        self.write_ws(":=");
        self.write_ws(node.priority.to_string().as_str());

        self.write_ws(")");

        self.write_ws(";");
        self.newline();
        Ok(())
    }

    // 3.3.2.1
    fn visit_assignment(
        &mut self,
        node: &dsl::textual::Assignment,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_variable(&node.target)?;
        self.write_ws(":=");
        self.visit_expr_kind(&node.value)?;

        self.write_ws(";");
        self.newline();
        Ok(())
    }
}
