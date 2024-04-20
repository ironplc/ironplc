//! Output writer for IEC61131-3 language elements. The writer transform
//! a parsed library into text.
//!
//! The writer is useful for debugging the parser and to understand the
//! internal representation.

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::{diagnostic::Diagnostic, visitor::Visitor};
use paste::paste;

/// Defines a macro for creating a comma separated list of items where
/// each item in the list is created by visiting the item.
macro_rules! visit_comma_separated {
    ($self:ident, $iter:expr, $struct_name:ident) => {
        paste! {
            {
                let mut it = $iter.peekable();
                while let Some(item) = it.next() {
                    $self.[<visit_ $struct_name:snake>](item)?;
                    if it.peek().is_some() {
                        $self.write_ws(",");
                    }
                }
            }
        }
    };
}

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

    fn write_char(&mut self, val: char) {
        self.buffer.push(val);
    }

    fn write(&mut self, val: &str) {
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

    fn visit_real_literal(&mut self, node: &RealLiteral) -> Result<Self::Value, Diagnostic> {
        let mut val = String::new();
        if let Some(data_type) = &node.data_type {
            val.push_str(data_type.as_id().original());
            val.push('#');
        }
        val.push_str(node.value.to_string().as_str());

        self.write_ws(val.as_str());
        Ok(())
    }

    fn visit_boolean_literal(&mut self, node: &BooleanLiteral) -> Result<Self::Value, Diagnostic> {
        let val = match node.value {
            Boolean::True => "BOOL#TRUE",
            Boolean::False => "BOOL#FALSE",
        };
        self.write_ws(val);
        Ok(())
    }

    fn visit_character_string_literal(
        &mut self,
        node: &CharacterStringLiteral,
    ) -> Result<Self::Value, Diagnostic> {
        // TODO this may not be right
        let mut val = String::from("'");
        let s: String = node.value.iter().collect();
        val.push_str(s.as_str());
        val.push('\'');
        self.write_ws(&val);
        Ok(())
    }

    fn visit_duration_literal(
        &mut self,
        node: &DurationLiteral,
    ) -> Result<Self::Value, Diagnostic> {
        // Always write out as milliseconds. The largest unit is allowed to be "out of range"
        let val = format!("TIME#{}ms", node.value.whole_milliseconds());
        self.write_ws(val.as_str());
        Ok(())
    }

    fn visit_time_of_day_literal(
        &mut self,
        _node: &TimeOfDayLiteral,
    ) -> Result<Self::Value, Diagnostic> {
        // TODO
        self.write_ws("TIME_OF_DAY#12:00:00.00");
        Ok(())
    }

    fn visit_date_literal(&mut self, _node: &DateLiteral) -> Result<Self::Value, Diagnostic> {
        // TODO
        self.write_ws("DATE#2000-01-01");
        Ok(())
    }

    fn visit_date_and_time_literal(
        &mut self,
        _node: &DateAndTimeLiteral,
    ) -> Result<Self::Value, Diagnostic> {
        // TODO
        self.write_ws("DATE_AND_TIME#2000-01-01-12:00:00.00");
        Ok(())
    }

    fn visit_bit_string_literal(
        &mut self,
        node: &BitStringLiteral,
    ) -> Result<Self::Value, Diagnostic> {
        let mut s = String::new();
        if let Some(data_type) = &node.data_type {
            s.push_str(data_type.as_id().to_string().as_str());
            s.push('#');
        }
        s.push_str(&node.value.value.to_string());

        self.write_ws(&s);
        Ok(())
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
        self.write_ws(";");
        self.outdent();
        self.newline();

        self.write_ws("END_TYPE");
        self.newline();
        Ok(())
    }

    fn visit_late_bound_declaration(
        &mut self,
        node: &LateBoundDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.data_type_name)?;

        self.write_ws(":");

        self.visit_id(&node.base_type_name)
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

        visit_comma_separated!(self, node.values.iter(), EnumeratedValue);

        self.write_ws(")");
        Ok(())
    }

    fn visit_subrange_declaration(
        &mut self,
        node: &SubrangeDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        self.write_ws(":");

        self.visit_subrange_specification_kind(&node.spec)?;

        if let Some(init) = &node.default {
            self.write_ws(":=");
            self.visit_signed_integer(init)?;
        }

        Ok(())
    }

    fn visit_subrange_specification(
        &mut self,
        node: &SubrangeSpecification,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name.as_id())?;
        self.write_ws("(");
        self.visit_subrange(&node.subrange)?;
        self.write_ws(")");

        Ok(())
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        self.write_ws(":");

        self.write_ws("STRUCT");

        self.indent();
        self.newline();
        for item in node.elements.iter() {
            self.visit_structure_element_declaration(item)?;
            self.write_ws(";");
            self.newline();
        }
        self.outdent();

        self.write_ws("END_STRUCT");

        Ok(())
    }

    fn visit_structure_element_declaration(
        &mut self,
        node: &StructureElementDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.name)?;
        self.write_ws(":");
        self.visit_initial_value_assignment_kind(&node.init)
    }

    // 2.3.3.1
    fn visit_structure_initialization_declaration(
        &mut self,
        node: &StructureInitializationDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        if !node.elements_init.is_empty() {
            self.write_ws(":=");
            self.write_ws("(");

            visit_comma_separated!(self, node.elements_init.iter(), StructureElementInit);

            self.write_ws(")");
        }

        Ok(())
    }

    // 2.3.3.1
    fn visit_structure_element_init(
        &mut self,
        node: &StructureElementInit,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.name)?;

        self.write_ws(":=");

        self.visit_struct_initial_value_assignment_kind(&node.init)
    }

    fn visit_array_declaration(
        &mut self,
        node: &ArrayDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        self.write_ws(":");

        self.visit_array_specification_kind(&node.spec)?;

        Ok(())
    }

    // 2.3.3.1
    fn visit_string_declaration(
        &mut self,
        node: &StringDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        self.write_ws(":");

        let typ = match node.width {
            StringKind::String => "STRING",
            StringKind::WString => "WSTRING",
        };
        self.write_ws(typ);

        self.write_ws("[");
        self.visit_integer(&node.length)?;
        self.write_ws("]");

        if let Some(init) = &node.init {
            let char = match node.width {
                StringKind::String => "\"",
                StringKind::WString => "'",
            };

            self.write_ws(":=");

            self.write(char);
            self.write(init);
            self.write(char)
        }

        Ok(())
    }

    fn visit_array_specification_kind(
        &mut self,
        node: &ArraySpecificationKind,
    ) -> Result<Self::Value, Diagnostic> {
        match &node {
            ArraySpecificationKind::Type(id) => self.visit_id(id)?,
            ArraySpecificationKind::Subranges(subranges) => {
                self.visit_array_subranges(subranges)?;
            }
        }

        Ok(())
    }

    // 2.3.3.1
    fn visit_array_subranges(&mut self, node: &ArraySubranges) -> Result<Self::Value, Diagnostic> {
        self.write_ws("ARRAY");
        self.write_ws("[");
        visit_comma_separated!(self, node.ranges.iter(), Subrange);
        self.write_ws("]");
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

    // 2.4.3.1 and 2.4.3.2
    fn visit_string_initializer(
        &mut self,
        node: &StringInitializer,
    ) -> Result<Self::Value, Diagnostic> {
        let kw = match node.width {
            StringKind::String => "STRING",
            StringKind::WString => "WSTRING",
        };
        self.write_ws(kw);

        if let Some(len) = &node.length {
            self.write_ws("[");
            self.visit_integer(len)?;
            self.write_ws("]");
        }

        if let Some(init) = &node.initial_value {
            self.write_ws(":=");

            let quote = match node.width {
                StringKind::String => "'",
                StringKind::WString => "\"",
            };

            self.write(quote);
            for c in init.iter() {
                self.write_char(*c);
            }
            self.write(quote);
        }

        Ok(())
    }

    // 2.4.3.2
    fn visit_enumerated_values_initializer(
        &mut self,
        node: &EnumeratedValuesInitializer,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("(");
        visit_comma_separated!(self, node.values.iter(), EnumeratedValue);
        self.write_ws(")");

        if let Some(init) = &node.initial_value {
            self.write_ws(":=");

            self.visit_enumerated_value(init)?;
        }

        Ok(())
    }

    // 2.4.3.2
    fn visit_function_block_initial_value_assignment(
        &mut self,
        node: &FunctionBlockInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)
    }

    // 2.4.3.2
    fn visit_array_initial_value_assignment(
        &mut self,
        node: &ArrayInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_array_specification_kind(&node.spec)?;

        if !node.initial_values.is_empty() {
            self.write_ws(":=");

            visit_comma_separated!(self, node.initial_values.iter(), ArrayInitialElementKind);
        }

        Ok(())
    }

    fn visit_enumerated_initial_value_assignment(
        &mut self,
        node: &EnumeratedInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.type_name)?;

        if let Some(init) = &node.initial_value {
            self.write_ws(":=");
            self.visit_enumerated_value(init)?;
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
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("FUNCTION");
        self.visit_id(&node.name)?;
        self.write_ws(":");
        self.visit_id(&node.return_type)?;

        self.indent();
        for item in node.variables.iter() {
            self.visit_var_decl(item)?;
        }
        self.outdent();
        self.newline();

        self.indent();
        for stmt in node.body.iter() {
            self.visit_stmt_kind(stmt)?;
        }
        self.outdent();

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
        self.outdent();

        self.indent();
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

    // 2.6.2
    fn visit_network(&mut self, node: &dsl::sfc::Network) -> Result<Self::Value, Diagnostic> {
        self.write_ws("INITIAL_STEP");
        self.visit_id(&node.initial_step.name)?;
        self.write_ws(":");
        self.newline();

        self.write_ws("END_STEP");
        self.newline();
        self.newline();

        for elem in node.elements.iter() {
            self.visit_element_kind(elem)?;
            self.newline();
        }
        Ok(())
    }

    // 2.6.2
    fn visit_step(&mut self, node: &dsl::sfc::Step) -> Result<Self::Value, Diagnostic> {
        self.write_ws("STEP");
        self.visit_id(&node.name)?;
        self.write_ws(":");
        self.newline();

        self.indent();
        for elem in node.action_associations.iter() {
            self.visit_action_association(elem)?;
            self.newline();
        }
        self.outdent();

        self.write_ws("END_STEP");
        self.newline();
        Ok(())
    }

    // 2.6.3
    fn visit_transition(&mut self, node: &dsl::sfc::Transition) -> Result<Self::Value, Diagnostic> {
        self.write_ws("TRANSITION FROM");

        visit_comma_separated!(self, node.from.iter(), Id);

        self.write_ws("TO");

        visit_comma_separated!(self, node.to.iter(), Id);
        self.newline();

        self.indent();
        self.write_ws(":=");
        self.visit_expr_kind(&node.condition)?;
        self.write_ws(";");
        self.newline();
        self.outdent();

        self.write_ws("END_TRANSITION");
        self.newline();
        Ok(())
    }

    // 2.6.3
    fn visit_action(&mut self, node: &dsl::sfc::Action) -> Result<Self::Value, Diagnostic> {
        self.write_ws("ACTION");

        self.visit_id(&node.name)?;
        self.write_ws(":");
        self.newline();

        self.indent();
        self.visit_function_block_body_kind(&node.body)?;
        self.outdent();

        self.write_ws("END_ACTION");
        self.newline();
        Ok(())
    }

    // 2.6.4
    fn visit_action_association(
        &mut self,
        node: &dsl::sfc::ActionAssociation,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.name)?;

        self.write_ws("(");

        if let Some(qualifier) = &node.qualifier {
            self.write_ws(qualifier.to_string().as_str());
            if !node.indicators.is_empty() {
                self.write_ws(",");
            }
        }

        visit_comma_separated!(self, node.indicators.iter(), Id);
        self.write_ws(");");

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

    // 3.2.3
    fn visit_fb_call(&mut self, node: &dsl::textual::FbCall) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.var_name)?;

        self.write_ws("(");
        visit_comma_separated!(self, node.params.iter(), ParamAssignmentKind);
        self.write_ws(")");

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

    fn visit_compare_expr(
        &mut self,
        node: &dsl::textual::CompareExpr,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("(");
        self.visit_expr_kind(&node.left)?;

        let op = match node.op {
            dsl::textual::CompareOp::Or => "OR",
            dsl::textual::CompareOp::Xor => "XOR",
            dsl::textual::CompareOp::And => "AND",
            dsl::textual::CompareOp::Eq => "=",
            dsl::textual::CompareOp::Ne => "<>",
            dsl::textual::CompareOp::Lt => "<",
            dsl::textual::CompareOp::Gt => ">",
            dsl::textual::CompareOp::LtEq => "<=",
            dsl::textual::CompareOp::GtEq => ">=",
        };
        self.write_ws(op);

        self.visit_expr_kind(&node.right)?;
        self.write_ws(")");
        Ok(())
    }

    fn visit_binary_expr(
        &mut self,
        node: &dsl::textual::BinaryExpr,
    ) -> Result<Self::Value, Diagnostic> {
        self.write_ws("(");
        self.visit_expr_kind(&node.left)?;

        let op = match node.op {
            dsl::textual::Operator::Add => "+",
            dsl::textual::Operator::Sub => "-",
            dsl::textual::Operator::Mul => "*",
            dsl::textual::Operator::Div => "/",
            dsl::textual::Operator::Mod => "MOD",
            dsl::textual::Operator::Pow => "**",
        };
        self.write_ws(op);

        self.visit_expr_kind(&node.right)?;
        self.write_ws(")");

        Ok(())
    }

    fn visit_unary_expr(
        &mut self,
        node: &dsl::textual::UnaryExpr,
    ) -> Result<Self::Value, Diagnostic> {
        let op = match node.op {
            dsl::textual::UnaryOp::Neg => "-",
            dsl::textual::UnaryOp::Not => "NOT",
        };
        self.write_ws(op);

        self.visit_expr_kind(&node.term)
    }

    fn visit_function(&mut self, node: &dsl::textual::Function) -> Result<Self::Value, Diagnostic> {
        self.visit_id(&node.name)?;

        self.write_ws("(");
        visit_comma_separated!(self, node.param_assignment.iter(), ParamAssignmentKind);
        self.write_ws(")");

        Ok(())
    }

    fn visit_repeat(&mut self, node: &dsl::textual::Repeat) -> Result<Self::Value, Diagnostic> {
        self.write_ws("REPEAT");
        self.newline();

        self.indent();
        for item in node.body.iter() {
            self.visit_stmt_kind(item)?;
        }
        self.outdent();

        self.write_ws("UNTIL");
        self.visit_expr_kind(&node.until)?;
        self.newline();

        self.write_ws("END_REPEAT");
        self.write_ws(";");
        self.newline();
        self.newline();

        Ok(())
    }

    fn visit_if(&mut self, node: &dsl::textual::If) -> Result<Self::Value, Diagnostic> {
        self.write_ws("IF");
        self.visit_expr_kind(&node.expr)?;
        self.write_ws("THEN");
        self.newline();

        self.indent();
        for item in node.body.iter() {
            self.visit_stmt_kind(item)?;
        }
        self.outdent();

        for item in node.else_ifs.iter() {
            self.visit_else_if(item)?;
        }

        if !node.else_body.is_empty() {
            self.write_ws("ELSE");
            self.newline();

            self.indent();
            for item in node.else_body.iter() {
                self.visit_stmt_kind(item)?;
            }
            self.outdent();
        }

        self.write_ws("END_IF");
        self.write_ws(";");
        self.newline();
        self.newline();
        Ok(())
    }

    fn visit_else_if(&mut self, node: &dsl::textual::ElseIf) -> Result<Self::Value, Diagnostic> {
        self.write_ws("ELSIF");
        self.visit_expr_kind(&node.expr)?;
        self.write_ws("THEN");
        self.newline();

        self.indent();
        for item in node.body.iter() {
            self.visit_stmt_kind(item)?;
        }
        self.outdent();

        Ok(())
    }

    fn visit_case(&mut self, node: &dsl::textual::Case) -> Result<Self::Value, Diagnostic> {
        self.write_ws("CASE");
        self.visit_expr_kind(&node.selector)?;
        self.write_ws("OF");
        self.newline();

        self.indent();
        for item in node.statement_groups.iter() {
            self.visit_case_statement_group(item)?;
        }

        if !node.else_body.is_empty() {
            self.write_ws("ELSE");
            self.newline();
            self.indent();

            for item in node.else_body.iter() {
                self.visit_stmt_kind(item)?;
            }

            self.outdent();
        }
        self.outdent();

        self.write_ws("END_CASE");
        self.write_ws(";");
        self.newline();
        self.newline();

        Ok(())
    }

    fn visit_case_statement_group(
        &mut self,
        node: &dsl::textual::CaseStatementGroup,
    ) -> Result<Self::Value, Diagnostic> {
        for selector in node.selectors.iter() {
            self.visit_case_selection_kind(selector)?;
            self.write_ws(":");
            self.newline();
        }

        self.indent();

        if node.statements.is_empty() {
            self.write_ws("(* empty *)");
            self.write_ws(";");
            self.newline();
        } else {
            for item in node.statements.iter() {
                self.visit_stmt_kind(item)?;
            }
        }
        self.outdent();

        Ok(())
    }

    fn visit_for(&mut self, node: &dsl::textual::For) -> Result<Self::Value, Diagnostic> {
        self.write_ws("FOR");
        self.visit_id(&node.control)?;
        self.write_ws(":=");
        self.visit_expr_kind(&node.from)?;
        self.write_ws("TO");
        self.visit_expr_kind(&node.to)?;

        if let Some(by) = &node.step {
            self.write_ws("BY");
            self.visit_expr_kind(by)?;
        }

        self.write_ws("DO");
        self.newline();

        self.indent();
        for item in node.body.iter() {
            self.visit_stmt_kind(item)?;
        }
        self.outdent();

        self.write_ws("END_FOR");
        self.write_ws(";");
        self.newline();
        self.newline();
        Ok(())
    }

    fn visit_while(&mut self, node: &dsl::textual::While) -> Result<Self::Value, Diagnostic> {
        self.write_ws("WHILE");
        self.visit_expr_kind(&node.condition)?;
        self.write_ws("DO");
        self.newline();

        self.indent();
        for item in node.body.iter() {
            self.visit_stmt_kind(item)?;
        }
        self.outdent();

        self.write_ws("END_WHILE");
        self.write_ws(";");
        self.newline();
        self.newline();
        Ok(())
    }

    fn visit_array_variable(
        &mut self,
        node: &dsl::textual::ArrayVariable,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_symbolic_variable_kind(&node.subscripted_variable)?;

        self.write_ws("[");
        visit_comma_separated!(self, node.subscripts.iter(), ExprKind);
        self.write_ws("]");

        Ok(())
    }

    fn visit_structured_variable(
        &mut self,
        node: &dsl::textual::StructuredVariable,
    ) -> Result<Self::Value, Diagnostic> {
        self.visit_symbolic_variable_kind(&node.record)?;
        self.write_ws(".");
        self.visit_id(&node.field)
    }
}
