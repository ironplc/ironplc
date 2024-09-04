//! Primary parser for IEC 61131-3 language elements. The parser transforms
//! a list of tokens into objects.
//!
//! This parser makes some simplifying assumptions:
//! * there are no pragmas
//! * there are no ladder diagram or function block diagrams
//!
//! These assumptions just mean an earlier stage needs to remove/apply these
//! elements.
//!
//! Rules in the parser generally map 1:1 to the production rules in the formal
//! specification (Appendix B). Important exceptions are:
//! * parts of a parser rule name following two underscores (__) are variations
//!   on formal production rules
//! * parser rule names in all capital letters are not production rules
extern crate peg;

use dsl::core::SourceSpan;
use dsl::diagnostic::Diagnostic;
use dsl::diagnostic::Label;
use ironplc_problems::Problem;
use peg::parser;
use peg::Parse;
use peg::ParseElem;
use peg::RuleResult;

use crate::token::{Token, TokenType};
use ironplc_dsl::common::*;
use ironplc_dsl::configuration::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::sfc::*;
use ironplc_dsl::textual::*;
use ironplc_dsl::time::*;

// Don't use std::time::Duration because it does not allow negative values.
use time::{Date, Month, PrimitiveDateTime, Time};

/// Parses a IEC 61131-3 library into object form.
pub fn parse_library(tokens: Vec<Token>) -> Result<Vec<LibraryElementKind>, Diagnostic> {
    plc_parser::library(&SliceByRef(&tokens[..])).map_err(|e| {
        let token_index = e.location;

        let expected = Vec::from_iter(e.expected.tokens()).join(" | ");
        let actual = tokens.get(token_index - 1).unwrap();

        Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(
                actual.span.clone(),
                format!(
                    "Expected {}. Found text '{}' that matched token {}",
                    expected,
                    actual.text.replace('\n', "\\n").replace('\r', "\\r"),
                    actual.token_type.describe()
                ),
            ),
        )
    })
}

/// Defines VarDecl type without the type information (e.g. input, output).
/// Useful only as an intermediate step in the parser where we do not know
/// the specific type.
struct UntypedVarDecl {
    pub name: Id,
    pub initializer: InitialValueAssignmentKind,
}

struct IncomplVarDecl {
    pub name: Id,
    pub qualifier: DeclarationQualifier,
    pub loc: AddressAssignment,
    pub spec: VariableSpecificationKind,
}

impl From<IncomplVarDecl> for VarDecl {
    fn from(val: IncomplVarDecl) -> Self {
        let init = match val.spec {
            VariableSpecificationKind::Simple(node) => {
                InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: node,
                    initial_value: None,
                })
            }
            VariableSpecificationKind::Subrange(node) => InitialValueAssignmentKind::Subrange(node),
            VariableSpecificationKind::Enumerated(node) => match node {
                EnumeratedSpecificationKind::TypeName(ty) => {
                    InitialValueAssignmentKind::EnumeratedType(EnumeratedInitialValueAssignment {
                        type_name: ty,
                        initial_value: None,
                    })
                }
                EnumeratedSpecificationKind::Values(values) => {
                    InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
                        values: values.values,
                        initial_value: None,
                    })
                }
            },
            VariableSpecificationKind::Array(node) => {
                InitialValueAssignmentKind::Array(ArrayInitialValueAssignment {
                    spec: node,
                    initial_values: vec![],
                })
            }
            // TODO initialize the variables
            VariableSpecificationKind::Struct(node) => {
                InitialValueAssignmentKind::Structure(StructureInitializationDeclaration {
                    type_name: node.type_name,
                    elements_init: vec![],
                })
            }
            VariableSpecificationKind::String(node) => {
                InitialValueAssignmentKind::String(StringInitializer {
                    length: node.length,
                    width: StringType::String,
                    initial_value: None,
                    keyword_span: node.keyword_span,
                })
            }
            VariableSpecificationKind::WString(node) => {
                InitialValueAssignmentKind::String(StringInitializer {
                    length: node.length,
                    width: StringType::WString,
                    initial_value: None,
                    keyword_span: node.keyword_span,
                })
            }
            VariableSpecificationKind::Ambiguous(node) => {
                InitialValueAssignmentKind::LateResolvedType(node)
            }
        };

        Self {
            identifier: VariableIdentifier::Direct(DirectVariableIdentifier {
                name: Some(val.name),
                address_assignment: val.loc,
                span: SourceSpan::default(),
            }),
            var_type: VariableType::Var,
            qualifier: val.qualifier,
            initializer: init,
        }
    }
}

// Container for IO variable declarations.
//
// This is internal for the parser to help with retaining context (input,
// output, etc). In effect, the parser needs a container because we don't
// know where to put the items until much later. It is even more problematic
// because we need to return a common type but that type is not needed
// outside of the parser.
enum VarDeclarations {
    // input_declarations
    Inputs(Vec<VarDecl>),
    // output_declarations
    Outputs(Vec<VarDecl>),
    // input_output_declarations
    Inouts(Vec<VarDecl>),
    // located_var_declarations
    Located(Vec<VarDecl>),
    // var_declarations
    Var(Vec<VarDecl>),
    // external_declarations
    External(Vec<VarDecl>),
    // TODO
    // Retentive(Vec<VarDecl>),
    // NonRetentive(Vec<VarDecl>),
    // Temp(Vec<VarDecl>),
    Incomplete(Vec<IncomplVarDecl>),
    ProgramAccess(Vec<ProgramAccessDecl>),
    ConfigAccess(Vec<AccessDeclaration>),
}

impl VarDeclarations {
    // Given multiple sets of declarations, unzip them into types of
    // declarations.
    pub fn unzip(mut decls: Vec<VarDeclarations>) -> Vec<VarDecl> {
        let mut vars = Vec::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(mut i) => {
                    vars.append(&mut i);
                }
                VarDeclarations::Outputs(mut o) => {
                    vars.append(&mut o);
                }
                VarDeclarations::Inouts(mut inouts) => {
                    vars.append(&mut inouts);
                }
                VarDeclarations::Located(mut l) => {
                    vars.append(&mut l);
                }
                VarDeclarations::Var(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::External(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::Incomplete(v) => {
                    vars.append(&mut v.into_iter().map(|var| var.into()).collect());
                }
                VarDeclarations::ProgramAccess(_) => {
                    // Omitted
                }
                VarDeclarations::ConfigAccess(_) => {
                    // Omitted
                }
            }
        }

        vars
    }

    pub fn unzip_with_access(
        mut decls: Vec<VarDeclarations>,
    ) -> (Vec<VarDecl>, Vec<ProgramAccessDecl>) {
        let mut vars = Vec::new();
        let mut access_vars = Vec::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(mut i) => {
                    vars.append(&mut i);
                }
                VarDeclarations::Outputs(mut o) => {
                    vars.append(&mut o);
                }
                VarDeclarations::Inouts(mut inouts) => {
                    vars.append(&mut inouts);
                }
                VarDeclarations::Located(mut l) => {
                    vars.append(&mut l);
                }
                VarDeclarations::Var(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::External(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::Incomplete(v) => {
                    vars.append(&mut v.into_iter().map(|var| var.into()).collect());
                }
                VarDeclarations::ProgramAccess(mut v) => {
                    access_vars.append(&mut v);
                }
                VarDeclarations::ConfigAccess(_v) => {
                    // Omitted
                }
            }
        }

        (vars, access_vars)
    }

    pub fn map(
        declarations: Vec<VarDecl>,
        qualifier: Option<DeclarationQualifier>,
    ) -> Vec<VarDecl> {
        declarations
            .into_iter()
            .map(|declaration| {
                let qualifier = qualifier
                    .clone()
                    .unwrap_or(DeclarationQualifier::Unspecified);
                let mut declaration = declaration;
                declaration.qualifier = qualifier;
                declaration
            })
            .collect()
    }

    pub fn flat_map(
        declarations: Vec<Vec<UntypedVarDecl>>,
        var_type: VariableType,
        qualifier: Option<DeclarationQualifier>,
    ) -> Vec<VarDecl> {
        let declarations = declarations.into_iter().flatten();

        declarations
            .into_iter()
            .map(|declaration| {
                let qualifier = qualifier
                    .clone()
                    .unwrap_or(DeclarationQualifier::Unspecified);

                VarDecl {
                    identifier: VariableIdentifier::Symbol(declaration.name),
                    var_type: var_type.clone(),
                    qualifier,
                    initializer: declaration.initializer,
                }
            })
            .collect()
    }
}

enum StatementsOrEmpty {
    Statements(Vec<StmtKind>),
    Empty(),
}

fn flatten_statements(mut items: Vec<StatementsOrEmpty>) -> Vec<StmtKind> {
    let mut stmts = Vec::new();
    for stmt_list in items.iter_mut() {
        match stmt_list {
            StatementsOrEmpty::Statements(s) => stmts.append(s),
            StatementsOrEmpty::Empty() => {}
        }
    }
    stmts
}

enum Element {
    StructSelector(Id),
    ArraySelector(Vec<ExprKind>),
}

enum InstanceInitKind {
    FunctionBlockInit(FunctionBlockInit),
    LocatedVarInit(LocatedVarInit),
}

enum ProgramConfigurationKind {
    Source(ProgramConnectionSource),
    Sink(ProgramConnectionSink),
    FbTask(FunctionBlockTask),
}

/// The default implementation of the parsing traits for `[T]` expects `T` to be
/// `Copy`, as in the `[u8]` or simple enum cases. This wrapper exposes the
/// elements by `&T` reference, which is `Copy`.
pub struct SliceByRef<'a, T>(pub &'a [T]);

impl<'a, T> Parse for SliceByRef<'a, T> {
    type PositionRepr = usize;
    fn start(&self) -> usize {
        0
    }

    fn is_eof(&self, pos: usize) -> bool {
        pos >= self.0.len()
    }

    fn position_repr(&self, pos: usize) -> usize {
        pos
    }
}

impl<'a, T: 'a> ParseElem<'a> for SliceByRef<'a, T> {
    type Element = &'a T;

    fn parse_elem(&'a self, pos: usize) -> RuleResult<&'a T> {
        match self.0[pos..].first() {
            Some(c) => RuleResult::Matched(pos + 1, c),
            None => RuleResult::Failed,
        }
    }
}

parser! {
  grammar plc_parser<'a>() for SliceByRef<'a, Token> {

    /// Rule to enable optional tracing rule for pegviz markers that makes
    /// working with the parser easier in the terminal.
    rule traced<T>(e: rule<T>) -> T =
    &(input:[t]* {
        #[cfg(any(feature = "debug", feature = "trace"))]
        println!("[PARSER INFO_START]\nNumber of parsed tokens: {}\n[PARSER INFO_STOP]\n", input.len());
        #[cfg(any(feature = "debug", feature = "trace"))]
        println!("[PEG_INPUT_START]\n{}\n[PEG_TRACE_START]", input.iter().fold(String::new(), |s1, s2| s1 + "\n" + s2.to_string().as_str()).trim_start().to_string());
    })
    e:e()? {?
        #[cfg(feature = "trace")]
        println!("[PEG_TRACE_STOP]");
        e.ok_or("")
    }

    /// Helper rule to match a particular type of token.
    rule tok(ty: TokenType) -> &'input Token = token:[t] {?
      if token.token_type == ty {
        return Ok(token);
      }
      Err(ty.describe())
    }

    rule tok_eq(ty: TokenType, val: &'static str) -> &'input Token = token:[t] {?
      if token.token_type == ty && token.text.as_str() == val {
        return Ok(token)
      }
      Err(val)
    }

    /// Helper rule to match an Identifier with the specified text
    rule id_eq(val: &str) -> &'input Token = [t if t.token_type == TokenType::Identifier && t.text.as_str() == val]

    // peg rules for making the grammar easier to work with. These produce
    // output on matching with the name of the item
    rule period() -> () = tok(TokenType::Period) ()
    rule semicolon() -> () = tok(TokenType::Semicolon) ()
    rule comma() -> () = tok(TokenType::Comma) ()
    rule whitespace() -> () = tok(TokenType::Whitespace) {} / tok(TokenType::Newline) {}

    rule comment() -> () = tok(TokenType::Comment) ()
    rule _ = (whitespace() / comment())*

    // Lists of separated items with required ending separator
    rule periodsep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ period() _)) _ period() {v}
    rule periodsep_oneplus_no_trailing<T>(x: rule<T>) -> Vec<T> = v:(x() ++ period()) {v}
    rule periodsep_no_trailing<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ period() _)) {v}
    rule semisep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ semicolon() _)) _ semicolon() {v}
    rule semisep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ semicolon() _)) semicolon() {v}
    rule commasep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ comma() _)) comma() {v}

    // TODO this should be a list of standard function block names
    rule STANDARD_FUNCTION_BLOCK_NAME() = id_eq("END_VAR")

    pub rule library() -> Vec<LibraryElementKind> = traced(<library__impl()>)
    //pub rule library() -> Vec<LibraryElementKind> = library__impl()
    pub rule library__impl() -> Vec<LibraryElementKind> = _ decls:library_element_declaration() ** _ _ { decls.into_iter().flatten().collect() }

    // B.0 Programming model
    rule library_element_declaration() -> Vec<LibraryElementKind> =
      data_types:data_type_declaration() { data_types.into_iter().map(LibraryElementKind::DataTypeDeclaration).collect() }
      / fd:function_declaration() { vec![LibraryElementKind::FunctionDeclaration(fd)] }
      / fbd:function_block_declaration() { vec![LibraryElementKind::FunctionBlockDeclaration(fbd)] }
      / pd:program_declaration() { vec![LibraryElementKind::ProgramDeclaration(pd)] }
      / cd:configuration_declaration() { vec![LibraryElementKind::ConfigurationDeclaration(cd)] }

    // B.1.1 Letters, digits and identifier
    rule identifier() -> Id = i:tok(TokenType::Identifier) {
      Id::from(i.text.as_str())
        .with_position(i.span.clone())
    }
    rule type_name() -> Type = i:identifier() { Type::from_id(&i) }

    // B.1.2 Constants
    rule constant() -> ConstantKind =
        real:real_literal() { ConstantKind::RealLiteral(real) }
        / integer:integer_literal() { ConstantKind::IntegerLiteral(integer) }
        / c:character_string() { ConstantKind::CharacterString(CharacterStringLiteral::new(c)) }
        / duration:duration() { ConstantKind::Duration(duration) }
        / t:time_of_day() { ConstantKind::TimeOfDay(t) }
        / d:date() { ConstantKind::Date(d) }
        / date_time:date_and_time() { ConstantKind::DateAndTime(date_time) }
        / bit_string:bit_string_literal() { ConstantKind::BitStringLiteral(bit_string) }
        / boolean:boolean_literal() { ConstantKind::Boolean(boolean) }

    // B.1.2.1 Numeric literals
    // numeric_literal omitted because it only appears in constant so we do not need to create a type for it
    rule integer_literal() -> IntegerLiteral = data_type:(t:integer_type_name() tok(TokenType::Hash) {t})? value:(bi:binary_integer() { bi.into() } / oi:octal_integer() { oi.into() } / hi:hex_integer() { hi.into() } / si:signed_integer() { si }) { IntegerLiteral { value, data_type } }
    rule signed_integer__positive() -> SignedInteger = tok(TokenType::Plus)? digits:tok(TokenType::Digits) {? SignedInteger::positive(digits.text.as_str()) }
    rule signed_integer__negative() -> SignedInteger = tok(TokenType::Minus) digits:tok(TokenType::Digits) {? SignedInteger::negative(digits.text.as_str()) }
    rule signed_integer() -> SignedInteger = signed_integer__positive() / signed_integer__negative()
    rule integer__string() -> &'input str = n:tok(TokenType::Digits) { n.text.as_str() }
    rule integer__string_simplified() -> String = n:integer__string() { n.to_string().chars().filter(|c| c.is_ascii_digit()).collect() }
    // TODO
    rule integer() -> Integer = n:integer__string() {? Integer::new(n, SourceSpan::default()) }
    rule binary_integer() -> Integer =  n:tok(TokenType::BinDigits) {? Integer::try_binary(n.text.as_str()) }
    rule octal_integer() -> Integer = n:tok(TokenType::OctDigits) {? Integer::try_octal(n.text.as_str()) }
    rule hex_integer() -> Integer = n:tok(TokenType::HexDigits) {? Integer::try_hex(n.text.as_str()) }
    rule real_literal() -> RealLiteral = tn:(t:real_type_name() tok(TokenType::Hash) {t})? sign:(tok(TokenType::Minus) { -1.0 }/ tok(TokenType::Plus) { 1.0 })? literal:(fp:tok(TokenType::FloatingPoint) {fp.text.as_str()} / fp:tok(TokenType::FixedPoint) {fp.text.as_str()} ){?
      let sign = sign.unwrap_or(1.0);
      RealLiteral::try_parse(literal, tn).map(|node| {
        RealLiteral {
          value: node.value * sign,
          data_type: node.data_type,
        }
      })
    }
    // TODO remove the exponent rule - unused based on tokenization
    rule exponent() -> i128 = (id_eq("E") / id_eq("e")) sign:(tok(TokenType::Plus) { 1 } / tok(TokenType::Minus) { -1 })? whole:tok(TokenType::Digits) {?
      let sign: i128 = sign.unwrap_or(1);
      let value: String = whole.text.chars().filter(|c| c.is_ascii_digit()).collect();
      let value = value.as_str().parse::<i128>().map_err(|e| "not an exponent")?;
      return Ok(sign * value);
    }
    // bit_string_literal_type is not a rule in the specification but helps write simpler code
    rule bit_string_literal_type() -> ElementaryTypeName =
      tok(TokenType::Byte) { ElementaryTypeName::BYTE }
      / tok(TokenType::Word) { ElementaryTypeName::WORD }
      / tok(TokenType::Dword) { ElementaryTypeName::DWORD }
      / tok(TokenType::Lword) { ElementaryTypeName::LWORD }
    // The specification says unsigned_integer, but there is no such rule.
    rule bit_string_literal() -> BitStringLiteral = data_type:(t:bit_string_literal_type() tok(TokenType::Hash) {t})? value:(bi:binary_integer() { bi }/ oi:octal_integer() { oi } / hi:hex_integer() { hi } / ui:integer() { ui } ) { BitStringLiteral { value, data_type } }
    rule boolean_literal() -> BooleanLiteral =
      // 1 and 0 can be a Boolean, but only with the prefix is it definitely a Boolean
      tok(TokenType::Bool) tok(TokenType::Hash) id_eq("1") { BooleanLiteral::new(Boolean::True) }
      / tok(TokenType::Bool) tok(TokenType::Hash) id_eq("0") { BooleanLiteral::new(Boolean::False) }
      / tok(TokenType::Bool) tok(TokenType::Hash) tok(TokenType::True)  { BooleanLiteral::new(Boolean::True) }
      / tok(TokenType::True) { BooleanLiteral::new(Boolean::True) }
      / tok(TokenType::Bool) tok(TokenType::Hash) tok(TokenType::False) { BooleanLiteral::new(Boolean::False) }
      / tok(TokenType::False) { BooleanLiteral::new(Boolean::False) }

    // B.1.2.2 Character strings
    rule character_string() -> Vec<char> = single_byte_character_string() / double_byte_character_string()
    rule single_byte_character_string() -> Vec<char>  = (tok(TokenType::String) tok(TokenType::Hash))? t:tok(TokenType::SingleByteString) {
      // The token includes the surrounding single quotes, so remove those when generating the literal
      let mut chars = t.text.chars();
      chars.next();
      chars.next_back();
      chars.collect()
    }
    rule double_byte_character_string() -> Vec<char> = (tok(TokenType::WString) tok(TokenType::Hash))? t:tok(TokenType::DoubleByteString) {
      let mut chars = t.text.chars();
      chars.next();
      chars.next_back();
      chars.collect()
    }

    // B.1.2.3 Time literals
    // Omitted and subsumed into constant.

    // B.1.2.3.1 Duration
    // dt_sep defines case insensitive separators between parts of duration
    rule dt_sep(val: &str) -> &'input Token = [t if t.token_type == TokenType::Identifier && t.text.as_str() == val]

    pub rule duration() -> DurationLiteral = start:position!() (tok(TokenType::Time) / dt_sep("T") / dt_sep("t")) tok(TokenType::Hash) s:(tok(TokenType::Minus))? i:interval() end:position!() {
      let span = SourceSpan::range(start, end);
      let interval = match s {
        Some(sign) => i.interval * -1,
        None => i.interval,
      };
      DurationLiteral {
        span,
        interval,
      }
    }
    // milliseconds must come first because the "m" in "ms" would match the minutes rule
    rule interval() -> DurationLiteral = ms:milliseconds() { ms }
      / d:days() { d }
      / h:hours() { h }
      / m:minutes() { m }
      / s:seconds() { s }
    rule days() -> DurationLiteral = days:fixed_point() dt_sep("d") { DurationLiteral::days(days) } / days:integer() dt_sep("d") dt_sep("_")? hours:hours() { hours.plus(DurationLiteral::days(days.into())) }
    rule fixed_point() -> FixedPoint =
      fp:tok(TokenType::FixedPoint) {?
        FixedPoint::parse(fp.text.as_str())
      }
      / i:integer() {?
        Ok(i.into())
    }
    rule hours() -> DurationLiteral = hours:fixed_point() dt_sep("h") { DurationLiteral::hours(hours) } / hours:integer() dt_sep("h") dt_sep("_")? min:minutes() { min.plus(DurationLiteral::hours(hours.into())) }
    rule minutes() -> DurationLiteral = min:fixed_point() dt_sep("m") { DurationLiteral::minutes(min) } / mins:integer() dt_sep("m") dt_sep("_")? sec:seconds() { sec.plus(DurationLiteral::minutes(mins.into())) }
    rule seconds() -> DurationLiteral = secs:fixed_point() dt_sep("s") { DurationLiteral::seconds(secs) } / sec:integer() dt_sep("s") dt_sep("_")? ms:milliseconds() { ms.plus(DurationLiteral::seconds(sec.into())) }
    rule milliseconds() -> DurationLiteral = ms:fixed_point() dt_sep("ms") { DurationLiteral::milliseconds(ms) }

    // 1.2.3.2 Time of day and date
    rule time_of_day() -> TimeOfDayLiteral = tok(TokenType::TimeOfDay) tok(TokenType::Hash) d:daytime() { TimeOfDayLiteral::new(d) }
    rule daytime() -> Time = h:day_hour() tok(TokenType::Colon) m:day_minute() tok(TokenType::Colon) s:day_second() {?
      Time::from_hms(h.try_into().map_err(|e| "hour")?, m.try_into().map_err(|e| "min")?, s.whole as u8).map_err(|e| "time")
    }
    rule day_hour() -> Integer = integer()
    rule day_minute() -> Integer = integer()
    rule day_second() -> FixedPoint = fixed_point()
    rule date() -> DateLiteral = (tok(TokenType::Date) / dt_sep("D") / dt_sep("d")) tok(TokenType::Hash) d:date_literal() { DateLiteral::new(d) }
    rule date_literal() -> Date = y:year() tok(TokenType::Minus) m:month() tok(TokenType::Minus) d:day() {?
      let y = y.value;
      let m = Month::try_from(<dsl::common::Integer as TryInto<u8>>::try_into(m).map_err(|e| "month")?).map_err(|e| "month")?;
      let d = d.value;
      Date::from_calendar_date(y.try_into().map_err(|e| "year")?, m, d.try_into().map_err(|e| "date")?).map_err(|e| "date")
    }
    rule year() -> Integer = i:integer() { i }
    rule month() -> Integer = i:integer() { i }
    rule day() -> Integer = i:integer() { i }
    rule date_and_time() -> DateAndTimeLiteral = tok(TokenType::DateAndTime) tok(TokenType::Hash) d:date_literal() tok(TokenType::Minus) t:daytime() { DateAndTimeLiteral::new(PrimitiveDateTime::new(d, t)) }

    // B.1.3 Data types
    // This should match generic_type_name, but that's unnecessary because
    // these are all just identifiers
    rule data_type_name() -> Type = non_generic_type_name()
    rule non_generic_type_name() -> Type = et:elementary_type_name() { et.into() } / derived_type_name()

    // B.1.3.1 Elementary data types
    rule elementary_type_name() -> ElementaryTypeName =
      numeric_type_name()
      / date_type_name()
      / bit_string_type_name()
      / tok(TokenType::String) { ElementaryTypeName::STRING }
      / tok(TokenType::WString) { ElementaryTypeName::WSTRING }
      / tok(TokenType::Time) { ElementaryTypeName::TIME }
    rule numeric_type_name() -> ElementaryTypeName = integer_type_name() / real_type_name()
    rule integer_type_name() -> ElementaryTypeName = signed_integer_type_name() / unsigned_integer_type_name()
    rule signed_integer_type_name() -> ElementaryTypeName = tok(TokenType::Sint) { ElementaryTypeName::SINT }  / tok(TokenType::Int) { ElementaryTypeName::INT } / tok(TokenType::Dint) { ElementaryTypeName::DINT } / tok(TokenType::Lint) { ElementaryTypeName::LINT }
    rule unsigned_integer_type_name() -> ElementaryTypeName = tok(TokenType::Usint) { ElementaryTypeName::USINT }  / tok(TokenType::Uint) { ElementaryTypeName::UINT } / tok(TokenType::Udint) { ElementaryTypeName::UDINT } / tok(TokenType::Ulint) { ElementaryTypeName::ULINT }
    rule real_type_name() -> ElementaryTypeName = tok(TokenType::Real) { ElementaryTypeName::REAL } / tok(TokenType::Lreal) { ElementaryTypeName::LREAL }
    rule date_type_name() -> ElementaryTypeName = tok(TokenType::Date) { ElementaryTypeName::DATE } / tok(TokenType::TimeOfDay) { ElementaryTypeName::TimeOfDay } / tok(TokenType::DateAndTime) { ElementaryTypeName::DateAndTime }
    rule bit_string_type_name() -> ElementaryTypeName = tok(TokenType::Bool) { ElementaryTypeName::BOOL } / tok(TokenType::Byte) { ElementaryTypeName::BYTE } / tok(TokenType::Word) { ElementaryTypeName::WORD } / tok(TokenType::Dword) { ElementaryTypeName::DWORD } / tok(TokenType::Lword) { ElementaryTypeName::LWORD }

    // B.1.3.2
    // rule omitted because this matched identifiers. Resolving type names happens later in parsing.
    // rule generic_type_name() -> &'input str = "ANY" / "ANY_DERIVED" / "ANY_ELEMENTARY" / "ANY_MAGNITUDE" / "ANY_NUM" / "ANY_REAL" / "ANY_INT" / "ANY_BIT" / "ANY_STRING" / "ANY_DATE"

    // B.1.3.3
    // TODO review this section for missing rules
    // All of these are aliases for identifiers, which means the single_element_type_name will just match first
    // I've left in just in case the definition changes.
    rule derived_type_name() -> Type = single_element_type_name() / array_type_name() / structure_type_name() / string_type_name()
    rule single_element_type_name() -> Type = simple_type_name() / subrange_type_name() / enumerated_type_name()
    rule simple_type_name() -> Type = type_name()
    rule subrange_type_name() -> Type = type_name()
    rule enumerated_type_name() -> Type = type_name()
    rule array_type_name() -> Type = type_name()
    rule structure_type_name() -> Type = type_name()
    rule data_type_declaration() -> Vec<DataTypeDeclarationKind> = tok(TokenType::Type) _ declarations:semisep(<type_declaration()>) _ tok(TokenType::EndType) { declarations }
    /// the type_declaration also bring in from single_element_type_declaration so that we can match in an order
    /// that identifies the type
    rule type_declaration() -> DataTypeDeclarationKind =
    s:string_type_declaration() { DataTypeDeclarationKind::String(s) }
      / s:string_type_declaration__parenthesis() { DataTypeDeclarationKind::String(s) }
      / a:array_type_declaration() { DataTypeDeclarationKind::Array(a) }
      / subrange:subrange_type_declaration__with_range() { DataTypeDeclarationKind::Subrange(subrange) }
      / structure_type_declaration__with_constant()
      / enumerated:enumerated_type_declaration__with_value() { DataTypeDeclarationKind::Enumeration(enumerated) }
      / simple:simple_type_declaration__with_constant() { DataTypeDeclarationKind::Simple(simple )}
      // The remaining are structure, enumerated and simple without an initializer
      // These all have the general form of
      //    `identifier : identifier`
      // and so are ambiguous.
      / ambiguous:structure_or_enumerated_or_simple_type_declaration__without_value() { DataTypeDeclarationKind::LateBound(ambiguous) }
    // Union of structure_type_declaration, enumerated_type_declaration and
    // simple_type_declaration all without any initializer. These types all
    // look the same
    rule structure_or_enumerated_or_simple_type_declaration__without_value() -> LateBoundDeclaration = data_type_name:type_name() _ tok(TokenType::Colon) _ base_type_name:type_name() {
      LateBoundDeclaration {
        data_type_name,
        base_type_name,
      }
    }

    rule simple_type_declaration__with_constant() -> SimpleDeclaration = type_name:simple_type_name() _ tok(TokenType::Colon) _ spec_and_init:simple_spec_init__with_constant() {
      SimpleDeclaration {
        type_name,
        spec_and_init,
      }
    }
    rule simple_spec_init() -> InitialValueAssignmentKind = type_name:simple_specification() _ constant:(tok(TokenType::Assignment) _ c:constant() { c })? {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: constant,
      })
    }
    // For simple types, they are inherently unambiguous because simple types are keywords (e.g. INT)
    rule simple_spec_init__with_constant() -> InitialValueAssignmentKind = type_name:simple_specification() _ tok(TokenType::Assignment) _ constant:constant() {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: Some(constant),
      })
    }
    rule simple_specification() -> Type = et:elementary_type_name() { et.into() } / simple_type_name()
    rule subrange_type_declaration__with_range() -> SubrangeDeclaration = type_name:subrange_type_name() _ tok(TokenType::Colon) _ spec:subrange_spec_init__with_range() {
      SubrangeDeclaration {
        type_name,
        spec: spec.0,
        default: spec.1,
      }
    }
    rule subrange_spec_init__with_range() -> (SubrangeSpecificationKind, Option<SignedInteger>) = spec:subrange_specification__with_range() _ default:(tok(TokenType::Assignment) _ def:signed_integer() { def })? {
      (spec, default)
    }
    // TODO or add a subrange type name
    rule subrange_specification__with_range() -> SubrangeSpecificationKind
      = type_name:integer_type_name() _ tok(TokenType::LeftParen) _ subrange:subrange() _ tok(TokenType::RightParen) { SubrangeSpecificationKind::Specification(SubrangeSpecification{ type_name, subrange }) }
    rule subrange() -> Subrange = start:signed_integer() tok(TokenType::Range) end:signed_integer() { Subrange{start, end} }

    rule enumerated_type_declaration__with_value() -> EnumerationDeclaration =
      type_name:enumerated_type_name() _ tok(TokenType::Colon) _ spec_init:enumerated_spec_init__with_value() {
        let spec = spec_init.0;
        let init = spec_init.1;

        EnumerationDeclaration {
          type_name,
          spec_init: EnumeratedSpecificationInit {
            spec,
            default: Some(init),
          },
        }
      }
      / type_name:enumerated_type_name() _ tok(TokenType::Colon) _ spec_init:enumerated_spec_init__with_values() {
        let spec = spec_init.0;
        let init = spec_init.1;

        EnumerationDeclaration {
          type_name,
          spec_init: EnumeratedSpecificationInit {
            spec,
            default: init,
          },
        }
      }
    rule enumerated_spec_init__with_value() -> (EnumeratedSpecificationKind, EnumeratedValue) = spec:enumerated_specification() _ tok(TokenType::Assignment) _ def:enumerated_value() {
      (spec, def)
    }
    rule enumerated_spec_init__with_values() -> (EnumeratedSpecificationKind, Option<EnumeratedValue>) = spec:enumerated_specification__only_values() _ default:(tok(TokenType::Assignment) _ d:enumerated_value() { d })? {
      (spec, default)
    }
    rule enumerated_spec_init() -> EnumeratedSpecificationInit = spec:enumerated_specification() _ default:(tok(TokenType::Assignment) _ d:enumerated_value() { d })? {
      EnumeratedSpecificationInit {
        spec,
        default,
      }
    }
    // TODO this doesn't support type name as a value
    rule enumerated_specification__only_values() -> EnumeratedSpecificationKind  =
      tok(TokenType::LeftParen) _ v:enumerated_value() ++ (_ tok(TokenType::Comma) _) _ tok(TokenType::RightParen) { EnumeratedSpecificationKind::values(v) }
    rule enumerated_specification() -> EnumeratedSpecificationKind  =
      tok(TokenType::LeftParen) _ v:enumerated_value() ++ (_ tok(TokenType::Comma) _) _ tok(TokenType::RightParen) { EnumeratedSpecificationKind::values(v) }
      / name:enumerated_type_name() { EnumeratedSpecificationKind::TypeName(name) }
    rule enumerated_value() -> EnumeratedValue = type_name:(name:enumerated_type_name() tok(TokenType::Hash) { name })? value:identifier() { EnumeratedValue {type_name, value} }
    rule array_type_declaration() -> ArrayDeclaration = type_name:array_type_name() _ tok(TokenType::Colon) _ spec_and_init:array_spec_init() {
      ArrayDeclaration {
        type_name,
        spec: spec_and_init.spec,
        init: spec_and_init.initial_values,
      }
    }
    rule array_spec_init() -> ArrayInitialValueAssignment = spec:array_specification() _ init:(tok(TokenType::Assignment) _ a:array_initialization() { a })? {
      ArrayInitialValueAssignment {
        spec,
        initial_values: init.unwrap_or_default()
      }
    }
    rule array_specification() -> ArraySpecificationKind = tok(TokenType::Array) _ tok(TokenType::LeftBracket) _ ranges:subrange() ** (_ tok(TokenType::Comma) _ ) _ tok(TokenType::RightBracket) _ tok(TokenType::Of) _ type_name:non_generic_type_name() {
      ArraySpecificationKind::Subranges(ArraySubranges { ranges, type_name } )
    }
    // TODO
    // type_name:array_type_name() {
    //  ArraySpecification::Type(type_name)
    //} /
    rule array_initialization() -> Vec<ArrayInitialElementKind> = tok(TokenType::LeftBracket) _ init:array_initial_elements() ** (_ tok(TokenType::Comma) _ ) _ tok(TokenType::RightBracket) { init }
    rule array_initial_elements() -> ArrayInitialElementKind = size:integer() _ tok(TokenType::LeftParen) ai:array_initial_element()? tok(TokenType::RightParen) { ArrayInitialElementKind::repeated(size, ai) } / array_initial_element()
    // TODO | structure_initialization | array_initialization
    rule array_initial_element() -> ArrayInitialElementKind = c:constant() { ArrayInitialElementKind::Constant(c) } / e:enumerated_value() { ArrayInitialElementKind::EnumValue(e) }
    rule structure_type_declaration__with_constant() -> DataTypeDeclarationKind =
      type_name:structure_type_name() _ tok(TokenType::Colon) _ decl:structure_declaration() {
        DataTypeDeclarationKind::Structure(StructureDeclaration {
          type_name,
          elements: decl.elements,
        })
      }
      / type_name:structure_type_name() _ tok(TokenType::Colon) _ init:initialized_structure__without_ambiguous() {
        DataTypeDeclarationKind::StructureInitialization(StructureInitializationDeclaration {
          // TODO there is something off with having two type names
          type_name,
          elements_init: init.elements_init,
        })
      }
    // structure_specification - covered in structure_type_declaration because that avoids
    // an intermediate object that doesn't know the type name
    rule initialized_structure() -> StructureInitializationDeclaration = type_name:structure_type_name() _ init:(tok(TokenType::Assignment) _ i:structure_initialization() {i})? {
      StructureInitializationDeclaration {
        type_name,
        elements_init: init.unwrap_or_default(),
      }
    }
    /// Same as initialized_structure but requires an initializer. Without the
    /// initializer, this is ambiguous with simple and enumeration initialization
    /// declarations.
    rule initialized_structure__without_ambiguous() -> StructureInitializationDeclaration = type_name:structure_type_name() _ tok(TokenType::Assignment) _ init:structure_initialization() {
      StructureInitializationDeclaration {
        type_name,
        elements_init: init,
      }
    }
    rule structure_declaration() -> StructureDeclaration = tok(TokenType::Struct) _ elements:semisep_oneplus(<structure_element_declaration()>) _ tok(TokenType::EndStruct) {
      StructureDeclaration {
        // Requires a value but we don't know the name until one level up
        type_name: Type::from(""),
        elements,
      }
    }
    rule structure_element_declaration() -> StructureElementDeclaration = name:structure_element_name() _ tok(TokenType::Colon) _ init:(
      arr:array_spec_init() { InitialValueAssignmentKind::Array(arr) }
      // handle the initial value
      / subrange:subrange_spec_init__with_range() { InitialValueAssignmentKind::Subrange(subrange.0) }
      / i:initialized_structure__without_ambiguous() { InitialValueAssignmentKind::Structure(i) }
      / spec_init:enumerated_spec_init__with_value() {
        match spec_init.0 {
          EnumeratedSpecificationKind::TypeName(id) => {
            InitialValueAssignmentKind::EnumeratedType(
              EnumeratedInitialValueAssignment {
                type_name: id,
                initial_value: Some(spec_init.1),
              }
            )
          },
          EnumeratedSpecificationKind::Values(values) => {
            InitialValueAssignmentKind::EnumeratedValues(
              EnumeratedValuesInitializer {
                values: values.values,
                // TODO initial value
                initial_value: None,
            })
          },
        }
      }
      / simple_spec_init__with_constant()
      / simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()
    ) {
        StructureElementDeclaration {
          name,
          init,
        }
    }
    rule structure_element_name() ->Id = identifier()
    rule structure_initialization() -> Vec<StructureElementInit> = tok(TokenType::LeftParen) _ elems:structure_element_initialization() ++ (_ tok(TokenType::Comma) _) _ tok(TokenType::RightParen) { elems }
    rule structure_element_initialization() -> StructureElementInit = name:structure_element_name() _ tok(TokenType::Assignment) _ init:(c:constant() { StructInitialValueAssignmentKind::Constant(c) } / ev:enumerated_value() { StructInitialValueAssignmentKind::EnumeratedValue(ev) } / ai:array_initialization() { StructInitialValueAssignmentKind::Array(ai) } / si:structure_initialization() {StructInitialValueAssignmentKind::Structure(si)}) {
      StructureElementInit {
        name,
        init,
      }
    }

    // Union of simple_spec_init and enumerated_spec_init rules. In some cases, these both
    // reduce to identifier [':=' identifier] and are inherently ambiguous. To work around
    // this, combine this to check for the unambiguous cases first, later reducing to
    // the ambiguous case that we resolve later.
    //
    // There is still value in trying to disambiguate early because it allows us to use
    // the parser definitions.
    rule simple_or_enumerated_or_subrange_ambiguous_struct_spec_init() -> InitialValueAssignmentKind = s:simple_specification() _ tok(TokenType::Assignment) _ c:constant() {
      // A simple_specification with a constant is unambiguous because the constant is
      // not a valid identifier.
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name: s,
        initial_value: Some(c),
      })
    } / spec:enumerated_specification() _ tok(TokenType::Assignment) _ init:enumerated_value() {
      // An enumerated_specification defined with a value is unambiguous the value
      // is not a valid constant.
      match spec {
        EnumeratedSpecificationKind::TypeName(name) => {
          InitialValueAssignmentKind::EnumeratedType(EnumeratedInitialValueAssignment {
            type_name: name,
            initial_value: Some(init),
          })
        },
        EnumeratedSpecificationKind::Values(values) => {
          InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
            values: values.values,
            initial_value: Some(init),
          })
        }
      }
    } / tok(TokenType::LeftParen) _ values:enumerated_value() ** (_ tok(TokenType::Comma) _ ) _ tok(TokenType::RightParen) _  init:(tok(TokenType::Assignment) _ i:enumerated_value() {i})? {
      // An enumerated_specification defined by enum values is unambiguous because
      // the parenthesis are not valid simple_specification.
      InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
        values,
        initial_value: init,
      })
    } / et:elementary_type_name() {
      // An identifier that is an elementary_type_name s unambiguous because these are
      // reserved keywords
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name: et.into(),
        initial_value: None,
      })
    }/ i:type_name() {
      // What remains is ambiguous and the devolves to a single identifier because the prior
      // cases have captures all cases with a value. This can be simple, enumerated or struct
      InitialValueAssignmentKind::LateResolvedType(i)
    }

    rule string_type_name() -> Type = type_name()
    rule string_type_declaration() -> StringDeclaration = type_name:string_type_name() _ tok(TokenType::Colon) _ width:(tok(TokenType::String) { StringType::String } / tok(TokenType::WString) { StringType::WString }) _ tok(TokenType::LeftBracket) _ length:integer() _ tok(TokenType::RightBracket) _ init:(tok(TokenType::Assignment) _ str:character_string() {str})? {
      StringDeclaration {
        type_name,
        length,
        width,
        init: init.map(|v| v.into_iter().collect()),
      }
    }
    rule string_type_declaration__parenthesis() -> StringDeclaration = type_name:string_type_name() _ tok(TokenType::Colon) _ width:(tok(TokenType::String) { StringType::String } / tok(TokenType::WString) { StringType::WString }) _ tok(TokenType::LeftParen) _ length:integer() _ tok(TokenType::RightParen) _ init:(tok(TokenType::Assignment) _ str:character_string() {str})? {
      StringDeclaration {
        type_name,
        length,
        width,
        init: init.map(|v| v.into_iter().collect()),
      }
    }

    // B.1.4 Variables
    rule variable() -> Variable =
      d:direct_variable() { Variable::Direct(d) }
      / symbolic_variable:symbolic_variable() { symbolic_variable.into() }
    //rule symbolic_variable() -> SymbolicVariableKind =
    //  multi_element_variable()
    //  / name:variable_name() { SymbolicVariableKind::Named(NamedVariable{name}) }
    rule symbolic_variable() -> SymbolicVariableKind = name:identifier() elements:(tok(TokenType::Period) id:identifier() { Element::StructSelector(id) } / sub:subscript_list() {Element::ArraySelector(sub)})* {
      // Start by assuming that the top is just a named variable
      let mut head = SymbolicVariableKind::Named(NamedVariable { name });

      // Then consume additional items to
      for elem in elements {
        match elem {
            Element::StructSelector(st) => {
              let cur = SymbolicVariableKind::Structured(StructuredVariable{
                record: Box::new(head),
                field: st,
              });
              head = cur;
            },
            Element::ArraySelector(arr) => {
              let cur = SymbolicVariableKind::Array(ArrayVariable{
                  subscripted_variable: Box::new(head),
                  subscripts: arr
                });
              head = cur;
            },
        }
      }

      head
    }
    rule variable_name() -> Id = identifier()

    // B.1.4.1 Directly represented variables
    // There is no location_prefix or size_prefix rule because it would be ambiguous when the % prefix normally
    // resolved ambiguity. Therefore, the lexer matches the entire direct variable.
    pub rule direct_variable() -> AddressAssignment = t:tok(TokenType::DirectAddress) {?
      AddressAssignment::try_from(t.text.as_str())
    }
    // B.1.4.2 Multi-element variables
    // TODO support these
    //rule multi_element_variable() -> SymbolicVariableKind =
    //  av:array_variable() {
    //    SymbolicVariableKind::Array(av)
    //  }
    //  / sv:structured_variable() {
    //    // TODO this is clearly wrong
    //    SymbolicVariableKind::Structured(StructuredVariable{ record: Box::new(sv.0), field: sv.1 })
    //  }
    //rule array_variable() -> ArrayVariable = variable:subscripted_variable() subscripts:subscript_list() {
    //    ArrayVariable {
    //      variable: Box::new(variable),
    //      subscripts,
    //    }
    //  }
    rule subscripted_variable() -> SymbolicVariableKind = symbolic_variable()
    rule subscript_list() -> Vec<ExprKind> = tok(TokenType::LeftBracket) _ list:subscript()++ (_ tok(TokenType::Comma) _) _ tok(TokenType::RightBracket) { list }
    rule subscript() -> ExprKind = expression()
    rule structured_variable() -> (SymbolicVariableKind, Id) = r:record_variable() tok(TokenType::Period) f:field_selector() { (r, f) }
    rule record_variable() -> SymbolicVariableKind = symbolic_variable()
    rule field_selector() -> Id = identifier()

    // B.1.4.3 Declarations and initialization
    pub rule input_declarations() -> Vec<VarDecl> = tok(TokenType::VarInput) _ qualifier:(tok(TokenType::Retain) {DeclarationQualifier::Retain} / tok(TokenType::NonRetain) {DeclarationQualifier::NonRetain})? _ declarations:semisep(<input_declaration()>) _ tok(TokenType::EndVar) {
      VarDeclarations::flat_map(declarations, VariableType::Input, qualifier)
    }
    // TODO edge_declaration
    rule input_declaration() -> Vec<UntypedVarDecl> = var_init_decl()
    rule edge_declaration() -> () = var1_list() _ tok(TokenType::Colon) _ tok(TokenType::Bool) _ (tok(TokenType::REdge) / tok(TokenType::FEdge))? {}
    // We have to first handle the special case of enumeration or fb_name without an initializer
    // because these share the same syntax. We only know the type after trying to resolve the
    // type name.
    // TODO add in subrange_spec_init(), enumerated_spec_init()
    rule var_init_decl() -> Vec<UntypedVarDecl> = structured_var_init_decl__without_ambiguous() / string_var_declaration() / array_var_init_decl() /  var1_init_decl__with_ambiguous_struct()
    rule var1_init_decl__with_ambiguous_struct() -> Vec<UntypedVarDecl> = names:var1_list() _ tok(TokenType::Colon) _ init:(a:simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()) {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: init.clone(),
        }
      }).collect()
    }

    rule var1_list() -> Vec<Id> = names:variable_name() ++ (_ tok(TokenType::Comma) _) { names }
    rule structured_var_init_decl() -> Vec<UntypedVarDecl> = names:var1_list() _ tok(TokenType::Colon) _ init_struct:initialized_structure()  {
      names.into_iter().map(|name| {
        // TODO
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Structure(init_struct.clone()),
        }
      }).collect()
    }
    rule structured_var_init_decl__without_ambiguous() -> Vec<UntypedVarDecl> = names:var1_list() _ tok(TokenType::Colon) _ init_struct:initialized_structure__without_ambiguous() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Structure(init_struct.clone()),
        }
      }).collect()
    }
    rule array_var_init_decl() -> Vec<UntypedVarDecl> = names:var1_list() _ tok(TokenType::Colon) _ init:array_spec_init() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Array(init.clone()),
        }
      }).collect()
    }
    // TODO implement this
    // rule fb_name_decl() -> Blah = fb_name_list() _ tok(TokenType::Colon) _ function_block_type_name() _ (tok(TokenType::Assignment) _ structure_initialization())?
    rule fb_name_list() -> Vec<Id> = commasep_oneplus(<fb_name()>)
    rule fb_name() -> Id = i:identifier() { i }
    pub rule output_declarations() -> Vec<VarDecl> = tok(TokenType::VarOutput) _ qualifier:(tok(TokenType::Retain) {DeclarationQualifier::Retain} / tok(TokenType::NonRetain) {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ tok(TokenType::EndVar) {
      VarDeclarations::flat_map(declarations, VariableType::Output, qualifier)
    }
    pub rule input_output_declarations() -> Vec<VarDecl> = tok(TokenType::VarInOut) _ qualifier:(tok(TokenType::Retain) {DeclarationQualifier::Retain} / tok(TokenType::NonRetain) {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ tok(TokenType::EndVar) {
      VarDeclarations::flat_map(declarations, VariableType::InOut,  qualifier)
    }
    // TODO temp_var_decl
    // rule var_declaration() -> Vec<VarDecl> = temp_var_decl() / fb_name_decl()
    // rule temp_var_decl() -> Vec<VarDecl> = var1_declaration() / array_var_declaration() / structured_var_declaration() / string_var_declaration()
    // rule var1_declaration() -> Blah = var1_list() _ tok(TokenType::Colon) _ (simple_specification() / subrange_specification() / enumerated_specification())
    rule var_declarations() -> VarDeclarations = tok(TokenType::Var) _ qualifier:(tok(TokenType::Constant) {DeclarationQualifier::Constant})? _ declarations:semisep(<var_init_decl()>) _ tok(TokenType::EndVar) {
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule retentive_var_declarations() -> VarDeclarations = tok(TokenType::Var) _ tok(TokenType::Retain) _ declarations:semisep(<var_init_decl()>) _ tok(TokenType::EndVar) {
      let qualifier = Option::Some(DeclarationQualifier::Retain);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule located_var_declarations() -> VarDeclarations = tok(TokenType::Var) _ qualifier:(tok(TokenType::Constant) { DeclarationQualifier::Constant } / tok(TokenType::Retain) {DeclarationQualifier::Retain} / tok(TokenType::NonRetain) {DeclarationQualifier::NonRetain})? _ declarations:semisep(<located_var_decl()>) _ tok(TokenType::EndVar) {
      let qualifier = qualifier.or(Some(DeclarationQualifier::Unspecified));
      VarDeclarations::Located(VarDeclarations::map(declarations, qualifier))
    }
    rule located_var_decl() -> VarDecl = name:variable_name()? _ location:location() _ tok(TokenType::Colon) _ initializer:located_var_spec_init() {
      VarDecl {
        identifier: VariableIdentifier::new_direct(name, location),
        // TODO Is the type always var?
        var_type: VariableType::Var,
        qualifier: DeclarationQualifier::Unspecified,
        initializer,
      }
    }
    // We use the same type as in other places for VarInit, but the external always omits the initializer
    rule external_var_declarations() -> VarDeclarations = tok(TokenType::VarExternal) _ qualifier:(tok(TokenType::Constant) {DeclarationQualifier::Constant})? _ declarations:semisep(<external_declaration()>) _ tok(TokenType::EndVar) {
      VarDeclarations::External(VarDeclarations::map(declarations, qualifier))
    }
    // TODO subrange_specification, array_specification(), structure_type_name and others
    rule external_declaration_spec() -> InitialValueAssignmentKind = type_name:simple_specification() {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: None,
      })
    }
    rule external_declaration() -> VarDecl = name:global_var_name() _ tok(TokenType::Colon) _ spec:external_declaration_spec() {
      VarDecl {
        identifier: VariableIdentifier::Symbol(name),
        var_type: VariableType::External,
        qualifier: DeclarationQualifier::Unspecified,
        initializer: spec,
      }
    }
    rule global_var_name() -> Id = i:identifier() { i }

    rule global_var_declarations__qualifier() -> DeclarationQualifier = tok(TokenType::Constant) { DeclarationQualifier::Constant } / tok(TokenType::Retain) { DeclarationQualifier::Retain }
    pub rule global_var_declarations() -> Vec<VarDecl> = tok(TokenType::VarGlobal) _ qualifier:global_var_declarations__qualifier()? _ declarations:semisep(<global_var_decl()>) _ tok(TokenType::EndVar) {
      // TODO set the options - this is pretty similar to VarInit - maybe it should be the same
      let declarations = declarations.into_iter().flatten();
      declarations.into_iter().map(|declaration| {
        let qualifier = qualifier.clone().unwrap_or(DeclarationQualifier::Unspecified);
        let mut declaration = declaration;
        declaration.qualifier = qualifier;
        declaration
      }).collect()
    }
    // TODO this doesn't pass all information. I suspect the rule from the description is not right
    rule global_var_decl() -> (Vec<VarDecl>) = vs:global_var_spec() _ tok:tok(TokenType::Colon) _ initializer:(l:located_var_spec_init() { l } / f:function_block_type_name() { InitialValueAssignmentKind::FunctionBlock(FunctionBlockInitialValueAssignment{type_name: f})})? {
      vs.0.into_iter().map(|name| {
        let init = initializer.clone().unwrap_or(InitialValueAssignmentKind::None(SourceSpan::join(&tok.span, &tok.span)));
        VarDecl {
          identifier: VariableIdentifier::Symbol(name),
          var_type: VariableType::Global,
          qualifier: DeclarationQualifier::Unspecified,
          // TODO this is clearly wrong
          initializer: init,
        }
      }).collect()
     }
    rule global_var_spec() -> (Vec<Id>, Option<AddressAssignment>) = names:global_var_list() {
      (names, None)
    } / global_var_name()? location() {
      // TODO this is clearly wrong, but it feel like the spec is wrong here
      (vec![Id::from("")], None)
    }
    // TODO this is completely fabricated - it isn't correct.
    rule located_var_spec_init() -> InitialValueAssignmentKind = simple:simple_spec_init() { simple }
    rule location() -> AddressAssignment = tok(TokenType::At) _ v:direct_variable() { v }
    rule global_var_list() -> Vec<Id> = names:global_var_name() ++ (_ tok(TokenType::Comma) _) { names }
    rule string_var_declaration() -> Vec<UntypedVarDecl> = single_byte_string_var_declaration() / double_byte_string_var_declaration()
    rule single_byte_string_var_declaration() -> Vec<UntypedVarDecl> = names:var1_list() _ tok(TokenType::Colon) _ spec:single_byte_string_spec() {
      names.into_iter().map(|name| {
        let span = name.span.clone();
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::String(spec.clone()),
        }
      }).collect()
    }
    rule single_byte_string_spec() -> StringInitializer = start:tok(TokenType::String) _ length:(tok(TokenType::LeftBracket) _ i:integer() _ tok(TokenType::RightBracket) {i})? _ initial_value:(tok(TokenType::Assignment) _ v:single_byte_character_string() {v})? {
      StringInitializer {
        length,
        width: StringType::String,
        initial_value,
        keyword_span: start.span.clone(),
      }
    }
    rule double_byte_string_var_declaration() -> Vec<UntypedVarDecl> =  names:var1_list() _ tok(TokenType::Colon) _ spec:double_byte_string_spec() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::String(spec.clone()),
        }
      }).collect()
    }
    rule double_byte_string_spec() -> StringInitializer = start:tok(TokenType::WString) _ length:(tok(TokenType::LeftBracket) _ i:integer() _ tok(TokenType::RightBracket) {i})? _ initial_value:(tok(TokenType::Assignment) _ v:double_byte_character_string() {v})? {
      StringInitializer {
        length,
        width: StringType::WString,
        initial_value,
        keyword_span: start.span.clone(),
      }
    }
    rule incompl_located_var_declarations() -> VarDeclarations = tok(TokenType::Var) _ qualifier:(tok(TokenType::Retain) {DeclarationQualifier::Retain} / tok(TokenType::NonRetain) {DeclarationQualifier::NonRetain})? _ declarations:semisep(<incompl_located_var_decl()>) _ tok(TokenType::EndVar) {
      let declarations = declarations.into_iter().map(|decl| {
        let qualifier = qualifier
          .clone()
          .unwrap_or(DeclarationQualifier::Unspecified);

        IncomplVarDecl {
          name: decl.name,
          qualifier,
          loc: decl.loc,
          spec: decl.spec,
        }
      }).collect();
      VarDeclarations::Incomplete(declarations)
    }
    rule incompl_located_var_decl() -> IncomplVarDecl = name:variable_name() _ loc:incompl_location() _ tok(TokenType::Colon) _ spec:var_spec() {
      IncomplVarDecl {
        name,
        qualifier: DeclarationQualifier::Unspecified,
        loc,
        spec,
      }
    }
    rule incompl_location() -> AddressAssignment = tok(TokenType::At) _ t:tok(TokenType::DirectAddressIncomplete) {?
      AddressAssignment::try_from(t.text.as_str())
    }
    rule var_spec() -> VariableSpecificationKind =
      sr:subrange_specification__with_range() { VariableSpecificationKind::Subrange(sr) }
      / e:enumerated_specification() { VariableSpecificationKind::Enumerated(e) }
      / a:array_specification() { VariableSpecificationKind::Array(a) }
      / tok:tok(TokenType::String) length:(_ tok(TokenType::LeftBracket) _ l:integer() tok(TokenType::RightBracket) { l })? { VariableSpecificationKind::String(StringSpecification{ width: StringType::String, length, keyword_span: tok.span.clone(), }) }
      / tok:tok(TokenType::WString) length:(_ tok(TokenType::LeftBracket) _ l:integer() tok(TokenType::RightBracket) { l })? { VariableSpecificationKind::String(StringSpecification{ width: StringType::WString, length, keyword_span: tok.span.clone(), }) }
      / et:elementary_type_name() { VariableSpecificationKind::Simple(et.into()) }
      / id:type_name() { VariableSpecificationKind::Ambiguous(id) }

    // B.1.5.1 Functions
    rule function_name() -> Id = standard_function_name() / derived_function_name()
    rule standard_function_name() -> Id = identifier()
    rule derived_function_name() -> Id = identifier()
    rule function_declaration() -> FunctionDeclaration = tok(TokenType::Function) _  name:derived_function_name() _ tok(TokenType::Colon) _ rt:(et:elementary_type_name() { et.into() } / dt:derived_type_name() { dt }) _ var_decls:(io:io_var_declarations() / func:function_var_decls()) ** _ _ body:function_body() _ tok(TokenType::EndFunction) {
      let variables = VarDeclarations::unzip(var_decls);
      FunctionDeclaration {
        name,
        return_type: rt,
        variables,
        body,
      }
    }
    rule io_var_declarations() -> VarDeclarations = i:input_declarations() { VarDeclarations::Inputs(i) } / o:output_declarations() { VarDeclarations::Outputs(o) } / io:input_output_declarations() { VarDeclarations::Inouts(io) }
    rule function_var_decls() -> VarDeclarations = tok(TokenType::Var) _ qualifier:(tok(TokenType::Constant) {DeclarationQualifier::Constant})? _ vars:semisep_oneplus(<var2_init_decl()>) _ tok(TokenType::EndVar) {
      VarDeclarations::Var(VarDeclarations::flat_map(vars, VariableType::Var, qualifier))
    }
    // TODO add instruction_list
    rule function_body() -> Vec<StmtKind> = statement_list()
    // TODO add many types here
    rule var2_init_decl() -> Vec<UntypedVarDecl> = var1_init_decl__with_ambiguous_struct()

    // B.1.5.2 Function blocks
    // IEC 61131 defines separate standard and derived function block names,
    // but we don't need that distinction here.
    rule function_block_type_name() -> Type = type_name()
    rule derived_function_block_name() -> Id = !STANDARD_FUNCTION_BLOCK_NAME() i:identifier() { i }
    rule function_block_declaration() -> FunctionBlockDeclaration = start:tok(TokenType::FunctionBlock) _ name:derived_function_block_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other }) ** _ _ body:function_block_body() _ end:tok(TokenType::EndFunctionBlock) {
      let variables = VarDeclarations::unzip(decls);
      FunctionBlockDeclaration {
        name,
        variables,
        body,
        span: SourceSpan::join(&start.span, &end.span),
      }
    }
    // TODO temp_var_decls
    rule other_var_declarations() -> VarDeclarations = external_var_declarations() / var_declarations() / retentive_var_declarations() / non_retentive_var_declarations() / incompl_located_var_declarations()
    //rule temp_var_decls() -> VarDeclarations = tok(TokenType::VarTemp) _ declarations:semisep(<temp_var_decl()>) _ tok(TokenType::EndVar) {
    //  let qualifier = Option::Some(DeclarationQualifier::Retain);
    //  VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::VarTemp, qualifier))
    //}
    rule non_retentive_var_declarations() -> VarDeclarations = tok(TokenType::Var) _ tok(TokenType::NonRetain) _ declarations:semisep(<var_init_decl()>) _ tok(TokenType::EndVar) {
      let qualifier = Option::Some(DeclarationQualifier::NonRetain);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    // TODO add instruction_list
    rule function_block_body() -> FunctionBlockBodyKind = networks:sequential_function_chart() { FunctionBlockBodyKind::sfc(networks) } / statements:statement_list() { FunctionBlockBodyKind::stmts(statements) } / _ { FunctionBlockBodyKind::empty( )}

    // B.1.5.3 Program declaration
    rule program_type_name() -> Id = identifier()
    // TODO program_access_decls
    pub rule program_declaration() -> ProgramDeclaration = tok(TokenType::Program) _ p:program_type_name() _ decls:(access:program_access_decls() { access } / io:io_var_declarations() { io } / other:other_var_declarations() { other } / located:located_var_declarations() { located }) ** _ _ body:function_block_body() _ tok(TokenType::EndProgram) {
      let (variables, access_variables) = VarDeclarations::unzip_with_access(decls);
      ProgramDeclaration {
        name: p,
        variables,
        access_variables,
        body,
      }
    }
    // TODO
    rule program_access_decls() -> VarDeclarations = tok(TokenType::VarAccess) _ decls:semisep_oneplus(<program_access_decl()>) _ tok(TokenType::EndVar) {
      VarDeclarations::ProgramAccess(decls)
    }
    rule program_access_decl() -> ProgramAccessDecl = access_name:access_name() _ tok(TokenType::Colon) _ symbolic_variable:symbolic_variable() _ tok(TokenType::Colon) _ type_name:non_generic_type_name() _ direction:direction()? {
      ProgramAccessDecl {
        access_name,
        symbolic_variable,
        type_name,
        direction,
      }
    }

    // B.1.6 Sequential function chart elements
    pub rule sequential_function_chart() -> Vec<Network> = networks:sfc_network() ++ _ { networks }
    rule sfc_network() ->  Network = init:initial_step() _ elements:((s:step() {s} / a:action() {a} / t:transition() {t}) ** _) {
      Network {
        initial_step: init,
        elements
      }
    }
    rule initial_step() -> Step = tok(TokenType::InitialStep) _ name:step_name() _ tok(TokenType::Colon) _ action_associations:action_association() ** (_ tok(TokenType::Semicolon) _) tok(TokenType::EndStep) {
      Step{
        name,
        action_associations,
       }
    }
    rule step() -> ElementKind = tok(TokenType::Step) _ name:step_name() _ tok(TokenType::Colon) _ action_associations:semisep(<action_association()>) _ tok(TokenType::EndStep) {
      ElementKind::step(
        name,
        action_associations
      )
    }
    rule step_name() -> Id = identifier()
    rule action_association() -> ActionAssociation = name:action_name() _ tok(TokenType::LeftParen) _ qualifier:action_qualifier()? _ indicators:(tok(TokenType::Comma) _ i:indicator_name() ** (_ tok(TokenType::Comma) _) { i })? _ tok(TokenType::RightParen) {
      ActionAssociation {
        name,
        qualifier,
        indicators: indicators.unwrap_or_default(),
      }
    }
    rule action_name() -> Id = identifier()
    rule action_qualifier() -> ActionQualifier =
      id_eq("N") { ActionQualifier::N }
      / id_eq("R") { ActionQualifier::R }
      / id_eq("S") { ActionQualifier::S }
      / id_eq("L") { ActionQualifier::L }
      / id_eq("D") { ActionQualifier::D }
      / id_eq("P") { ActionQualifier::P }
      / id_eq("SD") _ tok(TokenType::Comma) _ at:action_time() { ActionQualifier::SD(at) }
      / id_eq("DS") _ tok(TokenType::Comma) _ at:action_time() { ActionQualifier::DS(at) }
      / id_eq("SL") _ tok(TokenType::Comma) _ at:action_time() { ActionQualifier::SL(at) }
      / id_eq("P1") _ tok(TokenType::Comma) _ at:action_time() { ActionQualifier::PR(at) }
      / id_eq("P0") _ tok(TokenType::Comma) _ at:action_time() { ActionQualifier::PF(at) }
    rule timed_qualifier() -> TimedQualifier =
      id_eq("L") { TimedQualifier::L }
      / id_eq("D") { TimedQualifier::D }
      / id_eq("SD") { TimedQualifier::SD }
      / id_eq("DS") { TimedQualifier::DS }
      / id_eq("SL") { TimedQualifier::SL }
    rule action_time() -> ActionTimeKind = dur:duration() { ActionTimeKind::Duration(dur) } / var:variable_name() { ActionTimeKind::VariableName(var)}
    rule indicator_name() -> Id = variable_name()
    rule transition() -> ElementKind = tok(TokenType::Transition) _ name:transition_name()? _ priority:(tok(TokenType::LeftParen) _ id_eq("PRIORITY") _ tok(TokenType::Assignment) _ p:integer() _ tok(TokenType::RightParen) {p})? _ tok(TokenType::From) _ from:steps() _ tok(TokenType::To) _ to:steps() _ condition:transition_condition() _ tok(TokenType::EndTransition) {?
      let mut prio : Option<u32> = None;
      if let Some(p) = priority {
          let p = p.value.try_into().map_err(|e| "priority")?;
          prio = Some(p);
      }
      Ok(ElementKind::Transition(Transition {
        name,
        priority: prio,
        from,
        to,
        condition,
      }))
    }
    rule transition_name() -> Id = identifier()
    rule steps() -> Vec<Id> = name:step_name() {
      vec![name]
    } / tok(TokenType::LeftParen) _ n1:step_name() _ tok(TokenType::Comma) _ n2:step_name() _ nr:(tok(TokenType::Comma) _ n:step_name()) ** _ _ tok(TokenType::RightParen) {
      // TODO need to extend with nr
      vec![n1, n2]
    }
    // TODO add simple_instruction_list , fbd_network, rung
    rule transition_condition() -> ExprKind =  tok(TokenType::Assignment) _ expr:expression() _ tok(TokenType::Semicolon) { expr }
    rule action() -> ElementKind = tok(TokenType::Action) _ name:action_name() _ tok(TokenType::Colon) _ body:function_block_body() _ tok(TokenType::EndAction) {
      ElementKind::Action(Action {
        name,
        body
      })
    }

    // B.1.7 Configuration elements
    rule configuration_name() -> Id = identifier()
    rule resource_type_name() -> Id = identifier()
    // TODO this is missing some
    pub rule configuration_declaration() -> ConfigurationDeclaration = tok(TokenType::Configuration) _ n:configuration_name() _ g:global_var_declarations()? _ r:resource_declaration() _ i:instance_specific_initializations()? _ tok(TokenType::EndConfiguration) {
      let g = g.unwrap_or_default();
      // TODO this should really be multiple items
      let r = vec![r];

      let mut fb_inits: Vec<FunctionBlockInit> = Vec::new();
      let mut located_var_inits: Vec<LocatedVarInit> = Vec::new();

      if let Some(inits) = i {
        for init in inits {
          match init {
              InstanceInitKind::FunctionBlockInit(fb_init) => fb_inits.push(fb_init),
              InstanceInitKind::LocatedVarInit(located_var_init) => located_var_inits.push(located_var_init),
          }
        }
      }

      ConfigurationDeclaration {
        name: n,
        global_var: g,
        resource_decl: r,
        fb_inits,
        located_var_inits,
      }
    }
    rule resource_declaration() -> ResourceDeclaration = tok(TokenType::Resource) _ n:resource_name() _ tok(TokenType::On) _ t:resource_type_name() _ g:global_var_declarations()? _ resource:single_resource_declaration() _ tok(TokenType::EndResource) {
      let g = g.unwrap_or_default();
      ResourceDeclaration {
        name: n,
        resource: t,
        global_vars: g,
        tasks: resource.0,
        programs: resource.1,
      }
    }
    // TODO need to have more than one
    rule single_resource_declaration() -> (Vec<TaskConfiguration>, Vec<ProgramConfiguration>) = t:semisep(<task_configuration()>)? _ p:semisep_oneplus(<program_configuration()>) { (t.unwrap_or_default(), p) }
    rule resource_name() -> Id = identifier()
    rule access_declarations() -> VarDeclarations = tok(TokenType::VarAccess) _ decls:semisep_oneplus(<access_declaration()>) _ tok(TokenType::EndVar) { VarDeclarations::ConfigAccess(decls) }
    rule access_declaration() -> AccessDeclaration = identifier:access_name() _ tok(TokenType::Colon) _ path:access_path() _ tok(TokenType::Colon) _ type_name:non_generic_type_name() _ direction:direction()? {
      AccessDeclaration {
        identifier,
        path,
        type_name,
        direction,
      }
    }
    rule access_path() -> AccessPathKind =
      resource_name:(r:resource_name() tok(TokenType::Period) { r })? var:direct_variable() {
        AccessPathKind::Direct(DirectAccessPath { resource_name, variable: var })
      }
      / resource_name:(r:resource_name() tok(TokenType::Period) { r })? program_name:(p:program_name() tok(TokenType::Period) { p })? fb_name:periodsep(<fb_name()>) variable:symbolic_variable() {
        AccessPathKind::Symbolic(SymbolicAccessPath { resource_name, program_name, fb_name, variable })
      }
    rule global_var_reference() -> GlobalVarReference =  resource_name:(r:resource_name() tok(TokenType::Period) { r })? name:global_var_name() s:(tok(TokenType::Period) s:structure_element_name() { s } )? {
      GlobalVarReference {
        resource_name,
        global_var_name: name,
        structure_element_name: s,
      }
    }
    rule access_name() -> Id = identifier()
    // TODO
    // rule program_output_reference() -> Blah = program_name() tok(TokenType::Period) symbolic_variable()
    rule program_name() -> Id = identifier()
    rule direction() -> Direction = tok(TokenType::ReadOnly) { Direction::ReadOnly } / tok(TokenType::ReadWrite) { Direction::ReadWrite }
    pub rule task_configuration() -> TaskConfiguration = tok(TokenType::Task) _ name:task_name() _ init:task_initialization() {
      TaskConfiguration {
        name,
        priority: init.0,
        // TODO This needs to set the interval
        interval: init.1,
      }
    }
    rule task_name() -> Id = identifier()
    // TODO add single and interval
    pub rule task_initialization() -> (u32, Option<DurationLiteral>) = tok(TokenType::LeftParen) _ interval:task_initialization_interval()? _ priority:task_initialization_priority() _ tok(TokenType::RightParen) { (priority, interval) }
    rule task_initialization_interval() -> DurationLiteral = id_eq("INTERVAL") _ tok(TokenType::Assignment) _ source:data_source() _ tok(TokenType::Comma) {
      // TODO The interval may not necessarily be a duration, but for now, only support Duration types
      match source {
        ConstantKind::Duration(duration) => duration,
        _ => panic!("Only supporting Duration types for now"),
      }
     }
    rule task_initialization_priority() -> u32 = id_eq("PRIORITY") _ tok(TokenType::Assignment) _ i:integer() {? i.value.try_into().map_err(|e| "priority") }
    // TODO there are more here, but only supporting Constant for now
    pub rule data_source() -> ConstantKind = constant:constant() { constant }
    // TODO more options here
    pub rule program_configuration() -> ProgramConfiguration = tok(TokenType::Program) _ name:program_name() task_name:( _ tok(TokenType::With) _ t:task_name() { t })? _ tok(TokenType::Colon) _ pt:program_type_name() elements:(_ tok(TokenType::LeftParen) _ e:prog_conf_elements() _ tok(TokenType::RightParen) { e })? {
      let mut sources: Vec<ProgramConnectionSource> = Vec::new();
      let mut sinks: Vec<ProgramConnectionSink> = Vec::new();
      let mut fb_tasks: Vec<FunctionBlockTask> = Vec::new();

      if let Some(elements) = elements {
        for elem in elements.into_iter() {
          match elem {
              ProgramConfigurationKind::Source(source) => { sources.push(source) },
              ProgramConfigurationKind::Sink(sink) => { sinks.push(sink) },
              ProgramConfigurationKind::FbTask(fb) => { fb_tasks.push(fb) },
          }
        }
      }

      ProgramConfiguration {
        name,
        task_name,
        type_name: pt,
        fb_tasks,
        sources,
        sinks,
      }
     }
    rule prog_conf_elements() -> Vec<ProgramConfigurationKind> = commasep_oneplus(<prog_conf_element()>)
    rule prog_conf_element() -> ProgramConfigurationKind = fb:fb_task() { ProgramConfigurationKind::FbTask(fb) } / prog_cnxn()
    rule fb_task() -> FunctionBlockTask = fb_name:fb_name() _ tok(TokenType::With) _ task_name:task_name() {
      FunctionBlockTask { fb_name, task_name }
     }
    rule prog_cnxn() -> ProgramConfigurationKind =
      dst:symbolic_variable() _ tok(TokenType::Assignment) _ src:prog_data_source() {
        ProgramConfigurationKind::Source(ProgramConnectionSource {
          dst,
          src,
        })
      }
      / src:symbolic_variable() _ tok(TokenType::RightArrow) _ dst:data_sink() {
        ProgramConfigurationKind::Sink(ProgramConnectionSink {
          src,
          dst
        })
      }
    rule prog_data_source() -> ProgramConnectionSourceKind =
      c:constant() { ProgramConnectionSourceKind::Constant(c) }
      / ev:enumerated_value() { ProgramConnectionSourceKind::EnumeratedValue(ev) }
      / gv:global_var_reference() { ProgramConnectionSourceKind::GlobalVarReference(gv) }
      / dv:direct_variable() { ProgramConnectionSourceKind::DirectVariable(dv) }
    rule data_sink() -> ProgramConnectionSinkKind =
      gv:global_var_reference() { ProgramConnectionSinkKind::GlobalVarReference(gv) }
      / dv:direct_variable() { ProgramConnectionSinkKind::DirectVariable(dv) }
    rule instance_specific_initializations() -> Vec<InstanceInitKind> = tok(TokenType::VarConfig) _ init:semisep_oneplus(<instance_specific_init()>) _ tok(TokenType::EndVar) { init }
    rule instance_specific_init() -> InstanceInitKind = instance_specific_init__fb_init() / instance_specific_init__located()
    rule instance_specific_init__located() -> InstanceInitKind = resource_name:resource_name() tok(TokenType::Period) program_name:program_name() tok(TokenType::Period) fb_path:periodsep_no_trailing(<identifier()>) _ address:location()? _ tok(TokenType::Colon) _ initializer:located_var_spec_init() {
      InstanceInitKind::LocatedVarInit(LocatedVarInit {
        resource_name,
        program_name,
        fb_path,
        address,
        initializer,
      })
    }
    rule instance_specific_init__fb_init() -> InstanceInitKind = resource_name:resource_name() tok(TokenType::Period) program_name:program_name() tok(TokenType::Period) fb_path:periodsep_oneplus_no_trailing(<identifier()>) _ tok(TokenType::Colon) _ type_name:function_block_type_name() _ tok(TokenType::Assignment) _ initializer:structure_initialization() {
      InstanceInitKind::FunctionBlockInit(FunctionBlockInit {
        resource_name,
        program_name,
        fb_path,
        // TODO fix me
        fb_name: Id::from(""),
        type_name,
        initializer,
      })
    }

    // B 2.1 Instruction List
    // TODO this entire section

    // B.3.1 Expressions
    pub rule expression() -> ExprKind = precedence!{
      // or_expression
      x:(@) _ tok(TokenType::Or) _ y:@ { ExprKind::compare(CompareOp::Or, x, y) }
      --
      // xor_expression
      x:(@) _ tok(TokenType::Xor) _ y:@ { ExprKind::compare(CompareOp::Xor, x, y) }
      --
      // and_expression
      x:(@) _ tok(TokenType::And) _ y:@ { ExprKind::compare(CompareOp::And, x, y ) }
      --
      // comparison
      x:(@) _ tok(TokenType::Equal)_ y:@ { ExprKind::compare(CompareOp::Eq, x, y ) }
      x:(@) _ tok(TokenType::NotEqual) _ y:@ { ExprKind::compare(CompareOp::Ne, x, y ) }
      --
      // equ_expression
      x:(@) _ tok(TokenType::Less) _ y:@ { ExprKind::compare(CompareOp::Lt, x, y ) }
      x:(@) _ tok(TokenType::Greater)_ y:@ { ExprKind::compare(CompareOp::Gt, x, y ) }
      x:(@) _ tok(TokenType::LessEqual) _ y:@ { ExprKind::compare(CompareOp::LtEq, x, y) }
      x:(@) _ tok(TokenType::GreaterEqual) _ y:@ { ExprKind::compare(CompareOp::GtEq, x, y) }
      --
      // add_expression
      x:(@) _ tok(TokenType::Plus) _ y:@ { ExprKind::binary(Operator::Add, x, y ) }
      x:(@) _ tok(TokenType::Minus) _ y:@ { ExprKind::binary(Operator::Sub, x, y ) }
      --
      // multiply_operator
      x:(@) _ tok(TokenType::Star) _ y:@ { ExprKind::binary(Operator::Mul, x, y ) }
      x:(@) _ tok(TokenType::Div)_ y:@ { ExprKind::binary(Operator::Div, x, y ) }
      x:(@) _ tok(TokenType::Mod) _ y:@ { ExprKind::binary(Operator::Mod, x, y ) }
      --
      // power_expression
      x:(@) _ tok(TokenType::Power) _ y:@ { ExprKind::binary(Operator::Pow, x, y ) }
      --
      // unary_expression
      p:unary_expression() { p }
      --
      // primary_expression
      // TODO missing items here
      c:constant() { ExprKind::Const(c) }
      //ev:enumerated_value()
      v:variable() { ExprKind::Variable(v) }
      tok(TokenType::LeftParen) _ e:expression() _ tok(TokenType::RightParen) { ExprKind::Expression(Box::new(e)) }
      f:function_expression() { f }
    }
    rule unary_expression() -> ExprKind = unary:unary_operator()? _ expr:primary_expression() {
      if let Some(op) = unary {
        return ExprKind::unary(op, expr);
      }
      expr
    }
    rule unary_operator() -> UnaryOp = tok(TokenType::Minus) {UnaryOp::Neg} / tok(TokenType::Not) {UnaryOp::Not}
    rule primary_expression() -> ExprKind
      = constant:constant() {
          ExprKind::Const(constant)
        }
      // TODO enumerated value
      / function:function_expression() {
          function
        }
      / id:identifier() _ !(tok(TokenType::LeftParen) / tok(TokenType::LeftBracket) / tok(TokenType::Period)) {
        ExprKind::LateBound(LateBound{ name: id })
      }
      / variable:variable() {
        ExprKind::Variable(variable)
      }
      / tok(TokenType::LeftParen) _ expression:expression() _ tok(TokenType::RightParen) {
        expression
      }
    rule function_expression() -> ExprKind = name:function_name() _ tok(TokenType::LeftParen) _ params:param_assignment() ** (_ tok(TokenType::Comma) _) _ tok(TokenType::RightParen) {
      ExprKind::Function(Function {
        name,
        param_assignment: params
      })
    }

    // B.3.2 Statements
    pub rule statement_list() -> Vec<StmtKind> = items:statements_or_empty()+ {
      flatten_statements(items)
    }
    rule statements_or_empty() -> StatementsOrEmpty = _ tok(TokenType::Semicolon) _ { StatementsOrEmpty::Empty() } / s:semisep(<statement()>) { StatementsOrEmpty::Statements(s)}
    rule statement() -> StmtKind = assignment_statement() / selection_statement() / iteration_statement() / subprogram_control_statement()

    // B.3.2.1 Assignment statements
    pub rule assignment_statement() -> StmtKind = var:variable() _ tok(TokenType::Assignment) _ expr:expression() { StmtKind::assignment(var, expr) }

    // B.3.2.2 Subprogram control statements
    rule subprogram_control_statement() -> StmtKind = fb:fb_invocation() { fb } / tok(TokenType::Return) { StmtKind::Return }
    rule fb_invocation() -> StmtKind = name:fb_name() _ tok(TokenType::LeftParen) _ params:param_assignment() ** (_ tok(TokenType::Comma) _) _ end:tok(TokenType::RightParen) {
      let span = SourceSpan::join(&name.span, &end.span);
      StmtKind::FbCall(FbCall {
        var_name: name,
        params,
        position: span,
      })
    }
    // TODO this needs much more
    rule param_assignment() -> ParamAssignmentKind = not:(tok(TokenType::Not) {})? _ src:variable_name() _ tok(TokenType::RightArrow) _ tgt:variable() {
      ParamAssignmentKind::Output (
        Output{
        not: false,
        src,
        tgt,
      })
    } / name:(n:variable_name() _ tok(TokenType::Assignment) { n })? _ expr:expression() {
      match name {
        Some(n) => {
          ParamAssignmentKind::NamedInput(NamedInput {name: n, expr} )
        },
        None => {
          ParamAssignmentKind::positional(expr)
        }
      }
    }

    // B.3.2.3 Selection statements
    rule selection_statement() -> StmtKind = if_statement() / case_statement()
    rule if_statement() -> StmtKind = tok(TokenType::If) _ expr:expression() _ tok(TokenType::Then) _ body:statement_list()? _ else_ifs:(tok(TokenType::Elsif) _ expr:expression() _ tok(TokenType::Then) _ body:statement_list() {ElseIf{expr, body}}) ** _ _ else_body:(tok(TokenType::Else) _ e:statement_list() { e })? _ tok(TokenType::EndIf) {
      StmtKind::If(If {
        expr,
        body: body.unwrap_or_default(),
        else_ifs,
        else_body: else_body.unwrap_or_default()
      })
    }
    rule case_statement() -> StmtKind = tok(TokenType::Case) _ selector:expression() _ tok(TokenType::Of) _ cases:case_element() ** _ _ else_body:(tok(TokenType::Else) _ e:statement_list() { e })? _ tok(TokenType::EndCase) {
      StmtKind::Case(Case {
        selector,
        statement_groups: cases,
        else_body: else_body.unwrap_or_default(),
      })
    }
    rule case_element() -> CaseStatementGroup = selectors:case_list() _ tok(TokenType::Colon) _ statements:statement_list() {
      CaseStatementGroup {
        selectors,
        statements,
      }
    }
    rule case_list() -> Vec<CaseSelectionKind> = cases_list:case_list_element() ++ (_ tok(TokenType::Comma) _) { cases_list }
    rule case_list_element() -> CaseSelectionKind = sr:subrange() {CaseSelectionKind::Subrange(sr)} / si:signed_integer() {CaseSelectionKind::SignedInteger(si)} / ev:enumerated_value() {CaseSelectionKind::EnumeratedValue(ev)}

    // B.3.2.4 Iteration statements
    rule iteration_statement() -> StmtKind = f:for_statement() {StmtKind::For(f)} / w:while_statement() {StmtKind::While(w)} / r:repeat_statement() {StmtKind::Repeat(r)} / exit_statement()
    rule for_statement() -> For = tok(TokenType::For) _ control:control_variable() _ tok(TokenType::Assignment) _ range:for_list() _ tok(TokenType::Do) _ body:statement_list() _ tok(TokenType::EndFor) {
      For {
        control,
        from: range.0,
        to: range.1,
        step: range.2,
        body,
      }
    }
    rule control_variable() -> Id = identifier()
    rule for_list() -> (ExprKind, ExprKind, Option<ExprKind>) = from:expression() _ tok(TokenType::To) _ to:expression() _ step:(tok(TokenType::By) _ s:expression() {s})? { (from, to, step) }
    rule while_statement() -> While = tok(TokenType::While) _ condition:expression() _ tok(TokenType::Do) _ body:statement_list() _ tok(TokenType::EndWhile) {
      While {
        condition,
        body,
      }
    }
    rule repeat_statement() -> Repeat = tok(TokenType::Repeat) _ body:statement_list() _ tok(TokenType::Until) _ until:expression() _ tok(TokenType::EndRepeat) {
      Repeat {
        until,
        body,
      }
    }
    rule exit_statement() -> StmtKind = tok(TokenType::Exit) { StmtKind::Exit }
  }
}
