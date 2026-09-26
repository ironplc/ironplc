//! Semantic rule that a constant fits the type it is stored into.
//!
//! A declared type states the values a variable can hold. `USINT` holds 0
//! through 255, so `300` is not a value it can take, and storing one there is
//! a mistake rather than a request for the 44 that two's-complement
//! truncation would leave behind. Nothing in the source says the value
//! changes, so the compiler says it instead.
//!
//! The type is pushed down through operators to the literals beneath them,
//! which is how the backend compiles them: one operation type covers both
//! operands, so `b := 300 + 0` stores the same wrapped value as `b := 300`
//! and is diagnosed the same way.
//!
//! Bit string *types* are deliberately not checked. `BYTE` and `WORD` are
//! patterns rather than magnitudes, and wrapping one is a legitimate thing
//! for a program to want.
//!
//! A constant is checked wherever it is stored: an assignment, a variable's
//! initial value, the elements of an array, structure or function block
//! instance initializer, the default of a structure field or type
//! declaration, and an argument passed to a function or function block
//! input, against the type of the parameter it binds to.
//!
//! How a literal was spelled makes no difference: `16#1FF` is 511 whichever
//! radix it was written in, and 511 is not a `USINT`. The radix does not
//! survive parsing in any case.
//!
//! A prefixed literal states its own type, and is checked against that type
//! as well: `INT#40000` is not an `INT` whatever it is stored into, so
//! `d : DINT := INT#40000` is reported even though 40000 fits a `DINT`. The
//! same by-value reasoning covers the radix form: `INT#16#FFFF` is 65535 and
//! an `INT`, and no `INT` is 65535. A pattern that is meant to wrap is
//! spelled with a bit-string prefix (`WORD#16#FFFF`), which is not checked.
//!
//! An untyped real literal takes its type from where it is used, so one
//! stored into a `REAL` must be a value a `REAL` can represent. That is
//! reported as the real literal problem `rule_real_literal_range` reports for
//! a `REAL#` literal, rather than as an overflow: it is the literal's type,
//! not the variable's, that the value falls outside.
//!
//! See section 2.2.1.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       count : USINT := 255;
//!       total : SINT;
//!       pattern : BYTE;
//!    END_VAR
//!    total := -128;
//!    pattern := 300;      (* a bit string wraps by design *)
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       count : USINT := 300;   (* USINT holds 0..255 *)
//!       total : SINT;
//!       wide : DINT;
//!       ratio : REAL;
//!    END_VAR
//!    total := 200;               (* SINT holds -128..127 *)
//!    count := 255 + 1;           (* the operator does not widen the type *)
//!    wide := INT#40000;          (* not an INT, whatever wide is *)
//!    ratio := 1.0E30 * 1.0E30;   (* 1.0E60 is not a REAL *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    function_environment::FunctionEnvironment,
    intermediate_type::{ByteSized, FunctionBlockVarType, IntermediateType},
    result::SemanticResult,
    rule_real_literal_range,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
    value_range,
    variable_type::{self, Declarations, Declared},
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleConstantRange {
            type_environment: context.types(),
            function_environment: context.functions(),
            // `Declarations::new` opens the base scope, where declarations
            // made outside any POU land. Opening another here would leave the
            // stack unbalanced when the table drops.
            declarations: Declarations::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleConstantRange<'a> {
    type_environment: &'a TypeEnvironment,
    /// The signature of every function, which states its parameters' types.
    function_environment: &'a FunctionEnvironment,
    /// The declared type of every variable in scope.
    declarations: Declarations<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleConstantRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The value of an integer literal, or `None` when it is too large to be one.
///
/// A literal beyond `i128` cannot be stored in any IEC 61131-3 type, so the
/// caller reports it against whatever range it was checked against.
fn literal_value(literal: &IntegerLiteral) -> Option<i128> {
    let magnitude = i128::try_from(literal.value.value.value).ok()?;
    Some(if literal.value.is_neg {
        -magnitude
    } else {
        magnitude
    })
}

impl RuleConstantRange<'_> {
    /// Reports `constant` when the type it is stored into cannot hold it.
    fn check_constant(&mut self, constant: &ConstantKind, expected: &IntermediateType) {
        // Every integer literal arrives here as a value, whatever radix it
        // was written in. A `ConstantKind` that is neither an integer nor a
        // real -- a duration, a string -- has no range to check.
        match constant {
            ConstantKind::IntegerLiteral(literal) => {
                if let Some(range) = value_range::of(expected) {
                    self.check_literal(literal, range);
                }
            }
            ConstantKind::RealLiteral(literal) => self.check_real_literal(literal, expected),
            _ => {}
        }
    }

    /// Reports an untyped real `literal` stored into a `REAL` that cannot
    /// hold it.
    ///
    /// An untyped literal takes its type from where it is used, so `1.0E300`
    /// -- or `1.0E30 * 1.0E30` once folded -- stored into a `REAL` is a `REAL`
    /// literal, and not one a `REAL` can represent. That is the same problem
    /// `rule_real_literal_range` reports for `REAL#1.0E300`, and it is
    /// reported the same way.
    ///
    /// A prefixed literal states its own type, which that rule checks, and a
    /// value beyond every real type is reported there too.
    fn check_real_literal(&mut self, literal: &RealLiteral, expected: &IntermediateType) {
        let IntermediateType::Real {
            size: ByteSized::B32,
        } = expected
        else {
            return;
        };
        if literal.data_type.is_some()
            || !literal.value.is_finite()
            || (literal.value as f32).is_finite()
        {
            return;
        }
        self.diagnostics.push(rule_real_literal_range::out_of_range(
            literal,
            RealTypeName::REAL,
        ));
    }

    /// Reports `literal` when the type named by its prefix cannot hold it.
    ///
    /// `INT#40000` says the value is an `INT`, and no `INT` is 40000, so the
    /// literal contradicts itself whatever it is stored into. That is a
    /// different question from `check_constant`'s, which takes its range from
    /// the destination: the two are asked independently, so a literal that
    /// fits neither is reported once for each.
    fn check_prefixed_literal(&mut self, literal: &IntegerLiteral) {
        let Some(prefix) = &literal.data_type else {
            return;
        };
        let Some(attributes) = self
            .type_environment
            .get(&TypeName::from_id(&prefix.as_id()))
        else {
            return;
        };
        if let Some(range) = value_range::of(&attributes.representation) {
            self.check_literal(literal, range);
        }
    }

    /// Reports `literal` when its value is outside `range`.
    fn check_literal(&mut self, literal: &IntegerLiteral, range: (i128, i128)) {
        let (minimum, maximum) = range;
        let value = literal_value(literal);
        if value.is_some_and(|value| value >= minimum && value <= maximum) {
            return;
        }

        // A literal too large for `i128` has no printable value of its own,
        // so it is reported by the magnitude the source spelled.
        let reported = value.map_or_else(
            || format!("-{}", literal.value.value.value),
            |value| value.to_string(),
        );
        self.report_out_of_range(literal.value.value.span(), &reported, range);
    }

    /// Reports the value spelled `reported` as outside `range`.
    ///
    /// Every out-of-range constant is reported the same way, whatever
    /// context it was found in, so that the range that decides the outcome
    /// is the only thing that varies between reports.
    fn report_out_of_range(&mut self, span: SourceSpan, reported: &String, range: (i128, i128)) {
        let (minimum, maximum) = range;
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ConstantOverflow,
                Label::span(
                    span,
                    format!("Value must be in the range {minimum} to {maximum}"),
                ),
            )
            .with_context("value", reported)
            .with_context("minimum", &minimum.to_string())
            .with_context("maximum", &maximum.to_string()),
        );
    }

    /// Pushes `expected` down to the literals within `expr`.
    ///
    /// The walk follows the operators the backend compiles at one operation
    /// type, and stops at anything that introduces a type of its own: a
    /// function's arguments are checked against its parameters when the
    /// call is visited, and a variable carries its own declaration.
    ///
    /// A negated literal needs no handling here. Constant folding turns
    /// `-200` into one signed literal before any rule runs, so a `Neg` that
    /// survives has an operand this walk would descend into anyway.
    fn check_expr(&mut self, expr: &Expr, expected: &IntermediateType) {
        match &expr.kind {
            ExprKind::Const(constant) => self.check_constant(constant, expected),
            ExprKind::BinaryOp(binary) => {
                self.check_expr(&binary.left, expected);
                self.check_expr(&binary.right, expected);
            }
            ExprKind::UnaryOp(unary) => self.check_expr(&unary.term, expected),
            ExprKind::Expression(inner) => self.check_expr(inner, expected),
            _ => {}
        }
    }

    /// The type an assignment writes through `target`.
    ///
    /// This is the value written, not the variable selected from: `x.3 := v`
    /// writes a `BOOL` and `w.%B1 := v` writes a byte, whatever `x` and `w`
    /// are declared as.
    fn assignment_target_type(&self, target: &Variable) -> Option<IntermediateType> {
        let Variable::Symbolic(kind) = target else {
            // A directly represented variable (`%IW0`) has no declaration to
            // take a range from.
            return None;
        };
        match kind {
            SymbolicVariableKind::BitAccess(_) => Some(IntermediateType::Bool),
            SymbolicVariableKind::PartialAccess(partial) => Some(IntermediateType::Bytes {
                size: match partial.size {
                    PartialAccessSize::Byte => ByteSized::B8,
                    PartialAccessSize::Word => ByteSized::B16,
                    PartialAccessSize::DWord => ByteSized::B32,
                    PartialAccessSize::LWord => ByteSized::B64,
                },
            }),
            _ => variable_type::of(kind, &self.declarations, self.type_environment),
        }
    }

    /// Checks the constants a declaration's initializer stores against the
    /// declared type.
    ///
    /// Every kind of initializer that can hold a constant is checked: a
    /// simple initial value, the elements of an array initializer, and the
    /// element values of a function block instance initializer, each against
    /// the type of the place it initializes.
    fn check_initializer(&mut self, initializer: &InitialValueAssignmentKind) {
        match initializer {
            InitialValueAssignmentKind::Simple(simple) => {
                if let Some(constant) = &simple.initial_value {
                    if let Some(declared) = self.representation_of(&simple.type_name) {
                        self.check_constant(constant, &declared);
                    }
                }
            }
            InitialValueAssignmentKind::Array(array) => {
                if let Some(declared) =
                    variable_type::resolve_initializer(initializer, self.type_environment)
                {
                    self.check_array_elements(&array.initial_values, &declared);
                }
            }
            InitialValueAssignmentKind::FunctionBlock(function_block) => {
                self.check_named_elements(&function_block.type_name, &function_block.init);
            }
            // A structure initializer is a `StructureInitializationDeclaration`,
            // which the visitor reaches on its own, in a `VAR` block and a
            // `TYPE` block alike. The remaining kinds hold no numeric constant
            // (a string, an enumerated value, a reference), take their range
            // from a subrange that `rule_range_limits` checks, or pass
            // arguments to a constructor rather than store values.
            _ => {}
        }
    }

    /// The representation of the type `type_name` names.
    fn representation_of(&self, type_name: &TypeName) -> Option<IntermediateType> {
        self.type_environment
            .get(type_name)
            .map(|attributes| attributes.representation.clone())
    }

    /// Checks structure element initializers against the fields of the type
    /// `type_name` names.
    fn check_named_elements(&mut self, type_name: &TypeName, elements: &[StructureElementInit]) {
        if let Some(declared) = self.representation_of(type_name) {
            self.check_element_inits(elements, &declared);
        }
    }

    /// Checks structure element initializers against the fields of
    /// `declared`, a structure or function block type.
    fn check_element_inits(
        &mut self,
        elements: &[StructureElementInit],
        declared: &IntermediateType,
    ) {
        for element in elements {
            if let Some(field) = variable_type::struct_field_type(declared, &element.name) {
                self.check_struct_value(&element.init, &field);
            }
        }
    }

    /// Checks the value a structure element initializer stores into a field
    /// of type `expected`.
    fn check_struct_value(
        &mut self,
        value: &StructInitialValueAssignmentKind,
        expected: &IntermediateType,
    ) {
        match value {
            StructInitialValueAssignmentKind::Constant(constant) => {
                self.check_constant(constant, expected)
            }
            StructInitialValueAssignmentKind::Array(elements) => {
                self.check_array_elements(elements, expected)
            }
            StructInitialValueAssignmentKind::Structure(elements) => {
                self.check_element_inits(elements, expected)
            }
            StructInitialValueAssignmentKind::Expression(expr) => self.check_expr(expr, expected),
            StructInitialValueAssignmentKind::EnumeratedValue(_)
            | StructInitialValueAssignmentKind::LateBound(_) => {}
        }
    }

    /// Checks the elements of an array initializer against the element type
    /// of `declared`.
    ///
    /// An array initializer lists the elements flat whatever the array's
    /// shape, so an array whose elements are themselves arrays is checked
    /// against the innermost element type.
    fn check_array_elements(
        &mut self,
        elements: &[ArrayInitialElementKind],
        declared: &IntermediateType,
    ) {
        // Anything but an array has no element type to check against.
        let IntermediateType::Array { element_type, .. } = declared else {
            return;
        };
        let mut element_type = element_type.as_ref();
        while let IntermediateType::Array {
            element_type: inner,
            ..
        } = element_type
        {
            element_type = inner;
        }
        for element in elements {
            self.check_array_element(element, element_type);
        }
    }

    /// Checks one array initializer element, including every repetition of
    /// a repeated one (`2(300)`), against `expected`.
    fn check_array_element(
        &mut self,
        element: &ArrayInitialElementKind,
        expected: &IntermediateType,
    ) {
        match element {
            ArrayInitialElementKind::Constant(constant) => self.check_constant(constant, expected),
            ArrayInitialElementKind::Repeated(repeated) => {
                if let Some(inner) = repeated.init.as_ref() {
                    self.check_array_element(inner, expected);
                }
            }
            ArrayInitialElementKind::EnumValue(_) => {}
        }
    }

    /// Checks each input argument of a function call against the type of the
    /// parameter it binds to.
    ///
    /// A generic parameter (`ANY_NUM`) is not a type in the environment and
    /// states no range, so its argument is not checked.
    fn check_function_arguments(&mut self, node: &Function) {
        let Some(signature) = self.function_environment.get(&node.name) else {
            return;
        };
        for (param, arg) in signature.bind_inputs(&node.param_assignment) {
            if param.is_reference {
                continue;
            }
            if let Some(expected) = self.representation_of(&param.param_type) {
                self.check_expr(arg, &expected);
            }
        }
    }

    /// Checks each input argument of a function block call against the type
    /// of the input it binds to: a named argument by name among the
    /// `VAR_INPUT` and `VAR_IN_OUT` variables, a positional one by position
    /// among the `VAR_INPUT` variables.
    fn check_fb_call_arguments(&mut self, node: &FbCall) {
        let Some(declared) = self.declarations.find(&node.var_name) else {
            return;
        };
        let TypeReference::Named(type_name) = declared.type_reference() else {
            return;
        };
        let Some(IntermediateType::FunctionBlock { fields, .. }) =
            self.representation_of(&type_name)
        else {
            return;
        };

        let mut positional = fields
            .iter()
            .filter(|field| field.var_type == Some(FunctionBlockVarType::Input));
        for param in &node.params {
            let (field, arg) = match param {
                ParamAssignmentKind::PositionalInput(input) => (positional.next(), &input.expr),
                ParamAssignmentKind::NamedInput(input) => (
                    fields.iter().find(|field| {
                        field.name == input.name
                            && matches!(
                                field.var_type,
                                Some(FunctionBlockVarType::Input | FunctionBlockVarType::InOut)
                            )
                    }),
                    &input.expr,
                ),
                ParamAssignmentKind::Output(_) => continue,
            };
            if let Some(field) = field {
                self.check_expr(arg, &field.field_type);
            }
        }
    }

    /// Checks a comparison's literals against the type of the other side.
    ///
    /// `IF c = 200` compares at `c`'s type, so a literal that `c` can never
    /// hold makes the comparison unsatisfiable rather than false.
    fn check_compare(&mut self, compare: &CompareExpr) {
        if let Some(left) = self.type_environment.representation_of_expr(&compare.left) {
            self.check_expr(&compare.right, left);
        }
        if let Some(right) = self.type_environment.representation_of_expr(&compare.right) {
            self.check_expr(&compare.left, right);
        }
    }

    /// Checks a `CASE` label against the selector's type.
    ///
    /// A label the selector can never equal selects a group that can never
    /// run.
    fn check_case(&mut self, node: &Case) {
        let Some(selector) = self.type_environment.representation_of_expr(&node.selector) else {
            return;
        };
        let Some((minimum, maximum)) = value_range::of(selector) else {
            return;
        };

        let labels: Vec<&SignedInteger> = node
            .statement_groups
            .iter()
            .flat_map(|group| group.selectors.iter())
            .filter_map(|selection| match selection {
                CaseSelectionKind::SignedInteger(value) => Some(value),
                // A subrange label's bounds are not checked against the
                // selector type here; `rule_range_limits` checks their order.
                // A bit-string label is a pattern.
                _ => None,
            })
            .collect();

        for label in labels {
            let value = match i128::try_from(label.value.value) {
                Ok(magnitude) if label.is_neg => -magnitude,
                Ok(magnitude) => magnitude,
                Err(_) => continue,
            };
            if value < minimum || value > maximum {
                self.report_out_of_range(
                    label.value.span(),
                    &value.to_string(),
                    (minimum, maximum),
                );
            }
        }
    }
}

impl Visitor<Infallible> for RuleConstantRange<'_> {
    type Value = ();

    /// Opens a declaration's scope.
    ///
    /// Every kind contributes the same thing -- a frame its own declarations
    /// go into -- but the match stays exhaustive so that a new kind of scope
    /// has to say so rather than silently sharing the enclosing
    /// declaration's frame.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        match node {
            ScopeNode::Function(_)
            | ScopeNode::FunctionBlock(_)
            | ScopeNode::Program(_)
            | ScopeNode::Method(_) => self.declarations.enter(),
        }
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.declarations.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        self.declarations
            .add_if(node.identifier.symbolic_id(), Declared::of(node));

        self.check_initializer(&node.initializer);

        node.recurse_visit(self)
    }

    /// Every integer literal passes through here, wherever it appears, so a
    /// prefixed one is checked against its own type in an initializer, an
    /// operand, a comparison or a function argument alike.
    fn visit_integer_literal(&mut self, node: &IntegerLiteral) -> Result<(), Infallible> {
        self.check_prefixed_literal(node);
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        // A write through a reference stores into whatever the reference
        // points at, which this rule cannot see.
        if !node.deref {
            if let Some(target) = self.assignment_target_type(&node.target) {
                self.check_expr(&node.value, &target);
            }
        }
        node.recurse_visit(self)
    }

    /// A type alias's default (`R : REAL := 1.0E300`) is checked against the
    /// type it aliases.
    fn visit_simple_declaration(&mut self, node: &SimpleDeclaration) -> Result<(), Infallible> {
        self.check_initializer(&node.spec_and_init);
        node.recurse_visit(self)
    }

    /// A structure field's default is checked against the field's type.
    fn visit_structure_element_declaration(
        &mut self,
        node: &StructureElementDeclaration,
    ) -> Result<(), Infallible> {
        self.check_initializer(&node.init);
        node.recurse_visit(self)
    }

    fn visit_array_declaration(&mut self, node: &ArrayDeclaration) -> Result<(), Infallible> {
        if let Some(declared) = self.representation_of(&node.type_name) {
            self.check_array_elements(&node.init, &declared);
        }
        node.recurse_visit(self)
    }

    fn visit_structure_initialization_declaration(
        &mut self,
        node: &StructureInitializationDeclaration,
    ) -> Result<(), Infallible> {
        self.check_named_elements(&node.type_name, &node.elements_init);
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
        self.check_function_arguments(node);
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.check_fb_call_arguments(node);
        node.recurse_visit(self)
    }

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<(), Infallible> {
        self.check_compare(node);
        node.recurse_visit(self)
    }

    fn visit_case(&mut self, node: &Case) -> Result<(), Infallible> {
        self.check_case(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests;
