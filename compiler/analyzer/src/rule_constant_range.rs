//! Semantic rule that a constant is a value of its type.
//!
//! A type states the values it holds. `USINT` holds 0 through 255, so `300`
//! is not a value it can take, and storing one there is a mistake rather than
//! a request for the 44 that two's-complement truncation would leave behind.
//! Nothing in the source says the value changes, so the compiler says it
//! instead.
//!
//! Every integer type and every bit string has a range: `SINT` holds -128 to
//! 127, `BYTE` 0 to 255 and `DWORD` 0 to 4294967295. A bit string wraps at run
//! time, but a constant is not a run-time value: `BYTE#256` is not a byte
//! (ADR-0053).
//!
//! How a literal was spelled makes no difference: `16#1FF` is 511 whichever
//! radix it was written in, and 511 is not a `USINT`. The radix does not
//! survive parsing in any case.
//!
//! See section 2.2.1.
//!
//! ## A literal's own type
//!
//! Every integer and bit-string literal is a value of its own type. A prefixed
//! literal names it: `INT#40000` is not an `INT` whatever it is stored into,
//! and `INT#16#FFFF` is 65535, which no `INT` is. An untyped literal takes its
//! type from its context (ADR-0028), and `xform_insert_implicit_conversions`
//! records that type on the literal, so the rule reads it: the `300` of
//! `x := 300` on a `USINT`, of `ADD(s, 300)` on a `SINT` and of
//! `FOR s := 0 TO 300` are each a `USINT` or a `SINT`, and so is a shift
//! count, a condition's literal or a generic argument, at whatever type the
//! pass recorded for it.
//!
//! A literal the pass converts to another type must be a value of that type
//! too, as must a literal operand of `AND`, `OR` or `XOR`, which operates at
//! the type of the operator although no conversion is recorded for it.
//!
//! An untyped real literal recorded as a `REAL` must be a value a `REAL` can
//! represent. That is reported as the real literal problem
//! `rule_real_literal_range` reports for a `REAL#` literal, rather than as an
//! overflow.
//!
//! ## The program as written
//!
//! `stages::analyze` runs this rule after the conversion pass, on the library
//! the pass returns, so that it can read the types the pass records. Three
//! checks ask about the program as written instead. Each reads an operand's
//! type through the `ImplicitConversion` the pass wrapped it in, and does not
//! take a type the pass inferred for an untyped literal (`ExprType::Inferred`)
//! as one the program wrote (ADR-0056):
//!
//! * A constant stored directly in a place must be a value of the place's
//!   declared type: an assignment, including one through a reference
//!   (`p^ := 300` on a `REF_TO SINT`), a variable's initial value, the elements of
//!   an array, structure or function block instance initializer, the default
//!   of a structure field or type declaration, and an argument passed to a
//!   function or function block input. The type recorded for a literal can be
//!   wider than the place: a subrange is recorded at its base type, and the
//!   input of a standard function block and a write through a reference at
//!   the default slot type.
//! * A literal compared with an operand of another type must be a value that
//!   operand can hold: `DINT#300 < s` on a `SINT` can never be true, although
//!   the pass converts `s` to `DINT`.
//! * A `CASE` label must be a value the selector can hold. An untyped literal
//!   selector has the type the pass recorded for it.
//!
//! Each literal is reported once, for the first check it fails: its own type
//! before the program as written.
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
//!    pattern := BYTE#16#FF;
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
//!       pattern : BYTE;
//!    END_VAR
//!    total := 200;               (* SINT holds -128..127 *)
//!    count := 255 + 1;           (* the operator does not widen the type *)
//!    total := ADD(total, 300);   (* ADD computes at SINT here *)
//!    wide := INT#40000;          (* not an INT, whatever wide is *)
//!    pattern := BYTE#256;        (* BYTE holds 0..255 *)
//!    ratio := 1.0E30 * 1.0E30;   (* 1.0E60 is not a REAL *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::{FileId, Id, Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::collections::HashSet;
use std::convert::Infallible;

use crate::{
    function_environment::FunctionEnvironment,
    result::SemanticResult,
    rule_real_literal_range,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::{ByteSized, FunctionBlockVarType, SemanticType},
    symbol_environment::ScopeTracker,
    type_environment::TypeEnvironment,
    value_range, variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut rule = RuleConstantRange {
        context,
        type_environment: context.types(),
        function_environment: context.functions(),
        scope: ScopeTracker::default(),
        diagnostics: Vec::new(),
        reported: HashSet::new(),
    };
    // Each literal's own type first, so that a literal that is not a value
    // of its own type is reported against it rather than against the place
    // it is stored in.
    let Ok(()) = OwnTypes { rule: &mut rule }.walk(lib);
    run_rule(rule, lib)
}

struct RuleConstantRange<'a> {
    context: &'a SemanticContext,
    type_environment: &'a TypeEnvironment,
    /// The signature of every function, which states its parameters' types.
    function_environment: &'a FunctionEnvironment,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
    /// Where each literal already reported is, so that no literal is reported
    /// twice. A `SourceSpan` compares equal to every other, so the position
    /// is kept instead.
    reported: HashSet<(FileId, usize, usize)>,
}

impl DiagnosticVisitor for RuleConstantRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The value a sign and a magnitude spell, or `None` when it is too large to
/// be one.
///
/// A literal beyond `i128` cannot be stored in any IEC 61131-3 type, so the
/// caller reports it against whatever range it was checked against.
fn signed_value(is_neg: bool, magnitude: u128) -> Option<i128> {
    let magnitude = i128::try_from(magnitude).ok()?;
    Some(if is_neg { -magnitude } else { magnitude })
}

/// The type of `expr` as the program wrote it: the type of the operand a
/// conversion wraps, and none for a type the pass inferred from the context,
/// which the program did not state (see the module doc).
fn type_as_written<'t>(types: &'t TypeEnvironment, expr: &Expr) -> Option<&'t SemanticType> {
    match (&expr.kind, &expr.expr_type) {
        (ExprKind::ImplicitConversion(inner), _) => type_as_written(types, inner),
        (_, Some(ExprType::Inferred(_))) => None,
        _ => types.representation_of_expr(expr),
    }
}

impl RuleConstantRange<'_> {
    /// Reports `constant` when `expected`, the type of the place it is
    /// stored in or of the operand it is compared with, cannot hold it.
    fn check_constant(&mut self, constant: &ConstantKind, expected: &SemanticType) {
        // Every integer literal arrives here as a value, whatever radix it
        // was written in. A `ConstantKind` that is neither an integer nor a
        // real -- a duration, a string -- has no range to check.
        match constant {
            ConstantKind::IntegerLiteral(literal) => {
                if let Some(range) = value_range::of(expected) {
                    self.check_literal(literal, range);
                }
            }
            ConstantKind::BitStringLiteral(literal) => {
                if let Some(range) = value_range::of(expected) {
                    self.check_bit_string(literal, range);
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
    /// -- or `1.0E30 * 1.0E30` once folded -- recorded as or stored into a
    /// `REAL` is a `REAL` literal, and not one a `REAL` can represent. That
    /// is the same problem `rule_real_literal_range` reports for
    /// `REAL#1.0E300`, and it is reported the same way.
    ///
    /// A prefixed literal states its own type, which that rule checks, and a
    /// value beyond every real type is reported there too.
    fn check_real_literal(&mut self, literal: &RealLiteral, expected: &SemanticType) {
        let SemanticType::Real {
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
        self.report(
            &literal.span,
            rule_real_literal_range::out_of_range(literal, RealTypeName::REAL),
        );
    }

    /// Reports `literal` when the type named by its prefix cannot hold it.
    ///
    /// `INT#40000` says the value is an `INT`, and no `INT` is 40000, so the
    /// literal contradicts itself whatever it is stored into.
    fn check_prefixed_literal(&mut self, literal: &IntegerLiteral) {
        let Some(prefix) = &literal.data_type else {
            return;
        };
        if let Some(range) = self.range_named(&prefix.as_id()) {
            self.check_literal(literal, range);
        }
    }

    /// Reports `literal` when the bit string named by its prefix cannot hold
    /// it: `BYTE#256` is not a byte.
    fn check_prefixed_bit_string(&mut self, literal: &BitStringLiteral) {
        let Some(prefix) = &literal.data_type else {
            return;
        };
        if let Some(range) = self.range_named(&prefix.as_id()) {
            self.check_bit_string(literal, range);
        }
    }

    /// The range of the type named `name`.
    fn range_named(&self, name: &Id) -> Option<(i128, i128)> {
        let attributes = self.type_environment.get(&TypeName::from_id(name))?;
        value_range::of(&attributes.representation)
    }

    /// The range of the type recorded for `expr`.
    fn range_of_expr(&self, expr: &Expr) -> Option<(i128, i128)> {
        value_range::of(self.type_environment.representation_of_expr(expr)?)
    }

    /// Reports `literal` when its value is outside `range`.
    fn check_literal(&mut self, literal: &IntegerLiteral, range: (i128, i128)) {
        self.check_signed(&literal.value, range);
    }

    /// Reports the bit-string `literal` when its value is outside `range`.
    fn check_bit_string(&mut self, literal: &BitStringLiteral, range: (i128, i128)) {
        self.check_magnitude(literal.value.span(), false, literal.value.value, range);
    }

    /// Reports an untyped `constant` when the type the conversion pass
    /// recorded for it, on `expr`, cannot hold it.
    fn check_recorded_type(&mut self, constant: &ConstantKind, expr: &Expr) {
        match constant {
            ConstantKind::IntegerLiteral(literal) if literal.data_type.is_none() => {
                if let Some(range) = self.range_of_expr(expr) {
                    self.check_literal(literal, range);
                }
            }
            ConstantKind::BitStringLiteral(literal) if literal.data_type.is_none() => {
                if let Some(range) = self.range_of_expr(expr) {
                    self.check_bit_string(literal, range);
                }
            }
            ConstantKind::RealLiteral(literal) => {
                if let Some(recorded) = self.type_environment.representation_of_expr(expr) {
                    self.check_real_literal(literal, recorded);
                }
            }
            _ => {}
        }
    }

    /// Reports `operand` when it is a literal and the type `operation`
    /// computes at cannot hold it.
    fn check_operand(&mut self, operand: &Expr, operation: &Expr) {
        let Some(range) = self.range_of_expr(operation) else {
            return;
        };
        match &operand.kind {
            ExprKind::Const(ConstantKind::IntegerLiteral(literal)) => {
                self.check_literal(literal, range)
            }
            ExprKind::Const(ConstantKind::BitStringLiteral(literal)) => {
                self.check_bit_string(literal, range)
            }
            ExprKind::Expression(inner) => self.check_operand(inner, operation),
            _ => {}
        }
    }

    /// Reports the value that `is_neg` and `magnitude` spell at `span` when
    /// it is outside `range`.
    fn check_magnitude(
        &mut self,
        span: SourceSpan,
        is_neg: bool,
        magnitude: u128,
        range: (i128, i128),
    ) {
        let (minimum, maximum) = range;
        let value = signed_value(is_neg, magnitude);
        if value.is_some_and(|value| value >= minimum && value <= maximum) {
            return;
        }

        // A literal too large for `i128` has no printable value of its own,
        // so it is reported by the magnitude the source spelled.
        let sign = if is_neg { "-" } else { "" };
        let reported =
            value.map_or_else(|| format!("{sign}{magnitude}"), |value| value.to_string());
        self.report_out_of_range(span, &reported, range);
    }

    /// Reports the value spelled `reported` as outside `range`.
    ///
    /// Every out-of-range constant is reported the same way, whatever
    /// context it was found in, so that the range that decides the outcome
    /// is the only thing that varies between reports.
    fn report_out_of_range(&mut self, span: SourceSpan, reported: &String, range: (i128, i128)) {
        let (minimum, maximum) = range;
        let diagnostic = Diagnostic::problem(
            Problem::ConstantOverflow,
            Label::span(
                span.clone(),
                format!("Value must be in the range {minimum} to {maximum}"),
            ),
        )
        .with_context("value", reported)
        .with_context("minimum", &minimum.to_string())
        .with_context("maximum", &maximum.to_string());
        self.report(&span, diagnostic);
    }

    /// Records `diagnostic` for the literal at `span`, unless that literal
    /// was already reported.
    fn report(&mut self, span: &SourceSpan, diagnostic: Diagnostic) {
        if self
            .reported
            .insert((span.file_id.clone(), span.start, span.end))
        {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Checks the constant `expr` stores, when it is one, against
    /// `expected`: a literal alone, in parentheses, negated, or as the
    /// program wrote it inside the conversion the pass wrapped it in.
    ///
    /// The operand of any other operator is not what is stored: the
    /// operator computes a value of its own, at a type the pass records on
    /// the operand and the operand's own type check reads.
    fn check_expr(&mut self, expr: &Expr, expected: &SemanticType) {
        match &expr.kind {
            ExprKind::Const(constant) => self.check_constant(constant, expected),
            ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => {
                self.check_expr(&unary.term, expected)
            }
            ExprKind::Expression(inner) | ExprKind::ImplicitConversion(inner) => {
                self.check_expr(inner, expected)
            }
            _ => {}
        }
    }

    /// The type an assignment writes through `target`.
    ///
    /// This is the value written, not the variable selected from: `x.3 := v`
    /// writes a `BOOL` and `w.%B1 := v` writes a byte, whatever `x` and `w`
    /// are declared as.
    fn assignment_target_type(&self, target: &Variable) -> Option<SemanticType> {
        let Variable::Symbolic(kind) = target else {
            // A directly represented variable (`%IW0`) has no declaration to
            // take a range from.
            return None;
        };
        match kind {
            SymbolicVariableKind::BitAccess(_) => Some(SemanticType::Bool),
            SymbolicVariableKind::PartialAccess(partial) => Some(SemanticType::Bytes {
                size: match partial.size {
                    PartialAccessSize::Byte => ByteSized::B8,
                    PartialAccessSize::Word => ByteSized::B16,
                    PartialAccessSize::DWord => ByteSized::B32,
                    PartialAccessSize::LWord => ByteSized::B64,
                },
            }),
            _ => variable_type::of(kind, self.context, &self.scope.current()),
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
    fn representation_of(&self, type_name: &TypeName) -> Option<SemanticType> {
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
    fn check_element_inits(&mut self, elements: &[StructureElementInit], declared: &SemanticType) {
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
        expected: &SemanticType,
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
        declared: &SemanticType,
    ) {
        // Anything but an array has no element type to check against.
        let SemanticType::Array { element_type, .. } = declared else {
            return;
        };
        let mut element_type = element_type.as_ref();
        while let SemanticType::Array {
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
    fn check_array_element(&mut self, element: &ArrayInitialElementKind, expected: &SemanticType) {
        match element {
            ArrayInitialElementKind::Constant(constant) => self.check_constant(constant, expected),
            ArrayInitialElementKind::Repeated(repeated) => {
                if let Some(inner) = repeated.init.as_ref() {
                    self.check_array_element(inner, expected);
                }
            }
            ArrayInitialElementKind::EnumValue(_)
            | ArrayInitialElementKind::Structure(_)
            | ArrayInitialElementKind::Expression(_) => {}
        }
    }

    /// Checks each input argument of a function call against the type of the
    /// parameter it binds to.
    ///
    /// A generic parameter (`ANY_NUM`) is not a type in the environment and
    /// states no range. Its argument is checked by its own type instead: the
    /// type the call computes at, which the pass records on it.
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
        let Some(SemanticType::FunctionBlock { fields, .. }) =
            variable_type::declared(&node.var_name, self.context, &self.scope.current())
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
    /// hold makes the comparison unsatisfiable rather than false. A logical
    /// or bitwise operator (`AND`, `OR`, `XOR`) is not a comparison: it
    /// computes at a type of its own, which its operands' own type checks
    /// read.
    fn check_compare(&mut self, compare: &CompareExpr) {
        if !compare.op.is_comparison() {
            return;
        }
        if let Some(left) = type_as_written(self.type_environment, &compare.left) {
            self.check_expr(&compare.right, left);
        }
        if let Some(right) = type_as_written(self.type_environment, &compare.right) {
            self.check_expr(&compare.left, right);
        }
    }

    /// Checks every `CASE` label against the selector's type.
    ///
    /// A label the selector can never equal selects a group that can never
    /// run. Each label is a value, whatever radix it was written in:
    /// `16#FFFFFFFF` is 4294967295, which no `DINT` is, rather than a bit
    /// pattern that happens to read as -1 at the selector's width. A
    /// subrange label's bounds are values too, each compared against the
    /// selector; `rule_range_limits` checks their order.
    ///
    /// An untyped literal selector (`CASE 5 OF`) has no type as written, so
    /// its labels are compared at the type the pass recorded for it.
    fn check_case(&mut self, node: &Case) {
        let Some(selector) = type_as_written(self.type_environment, &node.selector)
            .or_else(|| self.type_environment.representation_of_expr(&node.selector))
        else {
            return;
        };
        let Some(range) = value_range::of(selector) else {
            return;
        };

        for selection in node
            .statement_groups
            .iter()
            .flat_map(|group| group.selectors.iter())
        {
            match selection {
                CaseSelectionKind::SignedInteger(value) => self.check_signed(value, range),
                CaseSelectionKind::BitStringLiteral(literal) => {
                    self.check_magnitude(literal.value.span(), false, literal.value.value, range)
                }
                CaseSelectionKind::Subrange(subrange) => {
                    // A bound that names a constant has no value here.
                    for bound in [&subrange.start, &subrange.end] {
                        if let Some(value) = bound.as_signed_integer() {
                            self.check_signed(value, range);
                        }
                    }
                }
                // An enumerated value is not an integer; a selector with a
                // range is not an enumeration.
                CaseSelectionKind::EnumeratedValue(_) => {}
            }
        }
    }

    /// Reports the signed integer `value` when it is outside `range`.
    fn check_signed(&mut self, value: &SignedInteger, range: (i128, i128)) {
        self.check_magnitude(value.value.span(), value.is_neg, value.value.value, range);
    }
}

impl Visitor<Infallible> for RuleConstantRange<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        self.check_initializer(&node.initializer);

        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
        let target = self.assignment_target_type(&node.target);
        // `p^ := v` stores into the variable `p` references, a value of the
        // type `p` refers to. The pass records the literal at the default
        // slot type, so only this check sees that type.
        let stored = match node.deref {
            true => target.and_then(|target| target.referenced_type().cloned()),
            false => target,
        };
        if let Some(stored) = stored {
            self.check_expr(&node.value, &stored);
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

/// The walk that checks each literal against its own type: its prefix, or
/// the type the conversion pass recorded for it.
struct OwnTypes<'r, 'a> {
    rule: &'r mut RuleConstantRange<'a>,
}

impl Visitor<Infallible> for OwnTypes<'_, '_> {
    type Value = ();

    fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
        // The literals within first, so that a literal is reported against
        // its own type before the type of an operation on it.
        node.recurse_visit(self)?;
        match &node.kind {
            ExprKind::Const(constant) => self.rule.check_recorded_type(constant, node),
            ExprKind::ImplicitConversion(inner) => self.rule.check_operand(inner, node),
            _ => {}
        }
        Ok(())
    }

    /// Every integer literal passes through here, wherever it appears, so a
    /// prefixed one is checked against its own type in an initializer, an
    /// operand, a comparison or a function argument alike.
    fn visit_integer_literal(&mut self, node: &IntegerLiteral) -> Result<(), Infallible> {
        self.rule.check_prefixed_literal(node);
        node.recurse_visit(self)
    }

    fn visit_bit_string_literal(&mut self, node: &BitStringLiteral) -> Result<(), Infallible> {
        self.rule.check_prefixed_bit_string(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
