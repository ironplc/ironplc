//! Primary parser for IEC 61131-3 language elements. The parser transforms
//! text into objects.
//!
//! This parser makes some simplifying assumptions:
//! * there are no comments
//! * there are no pragmas
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

use dsl::core::FileId;
use dsl::core::SourceLoc;
use dsl::diagnostic::Diagnostic;
use dsl::diagnostic::Label;
use dsl::diagnostic::QualifiedPosition;
use peg::parser;

use crate::mapper::*;
use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::sfc::*;
use ironplc_dsl::textual::*;

// Don't use std::time::Duration because it does not allow negative values.
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

/// Parses a IEC 61131-3 library into object form.
pub fn parse_library(source: &str, file_id: &FileId) -> Result<Vec<LibraryElement>, Diagnostic> {
    plc_parser::library(source).map_err(|e| {
        let expected = Vec::from_iter(e.expected.tokens()).join(", ");
        Diagnostic::new(
            "P0002",
            "Syntax error",
            Label::qualified(
                file_id.clone(),
                QualifiedPosition::new(e.location.line, e.location.column, e.location.offset),
                format!("Expected one of: {}", expected),
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
    pub position: SourceLoc,
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
    Located(Vec<LocatedVarDecl>),
    // var_declarations
    Var(Vec<VarDecl>),
    // external_declarations
    External(Vec<VarDecl>),
    // TODO
    // Retentive(Vec<VarDecl>),
    // NonRetentive(Vec<VarDecl>),
    // Temp(Vec<VarDecl>),
}

impl VarDeclarations {
    // Given multiple sets of declarations, unzip them into types of
    // declarations.
    pub fn unzip(mut decls: Vec<VarDeclarations>) -> (Vec<VarDecl>, Vec<LocatedVarDecl>) {
        let mut vars = Vec::new();
        let mut located = Vec::new();

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
                    located.append(&mut l);
                }
                VarDeclarations::Var(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::External(mut v) => {
                    vars.append(&mut v);
                } //VarDeclarations::Retentive(mut v) => {
                  //    other.retentives.append(&mut v);
                  //}
                  //VarDeclarations::NonRetentive(mut v) => {
                  //    other.non_retentives.append(&mut v);
                  //}
                  //VarDeclarations::Temp(mut v) => {
                  //    other.temps.append(&mut v);
                  //}
            }
        }

        (vars, located)
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
                    name: declaration.name,
                    var_type: var_type.clone(),
                    qualifier,
                    initializer: declaration.initializer,
                    position: declaration.position,
                }
            })
            .collect()
    }

    pub fn map_located(
        declarations: Vec<LocatedVarDecl>,
        qualifier: Option<DeclarationQualifier>,
    ) -> Vec<LocatedVarDecl> {
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
}

parser! {
  grammar plc_parser() for str {

    /// Rule to enable optional tracing rule for pegviz markers that makes
    /// working with the parser easier in the terminal.
    rule traced<T>(e: rule<T>) -> T =
    &(input:$([_]*) {
        #[cfg(feature = "trace")]
        println!("[PEG_INPUT_START]\n{}\n[PEG_TRACE_START]", input);
    })
    e:e()? {?
        #[cfg(feature = "trace")]
        println!("[PEG_TRACE_STOP]");
        e.ok_or("")
    }

    // peg rules for making the grammar easier to work with. These produce
    // output on matching with the name of the item
    rule semicolon() -> () = ";" ()
    rule comma() -> () = "," ()
    rule _ = [' ' | '\n' | '\r' | '\t' ]*

    rule i(literal: &'static str)
      = input:$([_]*<{literal.len()}>)
        {? if input.eq_ignore_ascii_case(literal) { Ok(()) } else { Err(literal) } }

    // A semi-colon separated list with required ending separator
    rule semisep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ semicolon() _)) _ semicolon() {v}
    rule semisep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ semicolon() _)) semicolon() {v}
    rule commasep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ comma() _)) comma() {v}

    rule KEYWORD_ITEM() = i("ACTION") / i("END_ACTION") / i("ARRAY") / i("OF") / i("AT") / i("CASE")
                     / i("ELSE") / i("END_CASE") / i("CONFIGURATION") / i("END_CONFIGURATION")
                     / i("CONSTANT") / i("EN") / i("ENO") / i("EXIT") / i("FALSE") / i("F_EDGE")
                     / i("FOR") / i("TO") / i("BY") / i("DO") / i("END_FOR") / i("FUNCTION") / i("END_FUNCTION")
                     / i("FUNCTION_BLOCK") / i("END_FUNCTION_BLOCK") / i("IF") / i("THEN")
                     / i("ELSIF") / i("ELSE") / "END_IF)" / i("INITIAL_STEP") / i("END_STEP")
                     / i("NOT") / i("MOD") / i("AND") / i("XOR") / i("OR") / i("PROGRAM") / i("END_PROGRAM")
                     / i("R_EDGE") / i("READ_ONLY") / i("READ_WRITE") / i("REPEAT") / i("UNTIL")
                     / i("END_REPEAT") / i("RESOURCE") / i("END_RESOURCE") / i("RETAIN") / i("NON_RETAIN")
                     / i("RETURN") / i("STEP") / i("END_STEP") / i("STRUCT") / i("END_STRUCT")
                     / i("TASK") / i("TRANSITION") / i("FROM") / i("END_TRANSITION") / i("TRUE")
                     / i("VAR") / i("END_VAR") / i("VAR_INPUT") / i("VAR_OUTPUT") / i("VAR_IN_OUT")
                     / i("VAR_TEMP") / i("VAR_EXTERNAL") / i("VAR_ACCESS") / i("VAR_CONFIG")
                     / i("VAR_GLOBAL") / i("WHILE") / i("END_WHILE") / i("WITH")
                     / i("PRIORITY") / i("STRING") / i("WSTRING")
    rule ID_CHAR() = ['a'..='z' | '0'..='9' | 'A'..='Z' | '_']
    rule KEYWORD() = KEYWORD_ITEM() !ID_CHAR()
    rule STANDARD_FUNCTION_BLOCK_NAME() = i("END_VAR")


    pub rule library() -> Vec<LibraryElement> = traced(<library__impl()>)
    pub rule library__impl() -> Vec<LibraryElement> = _ decls:library_element_declaration() ** _ _ { decls.into_iter().flatten().collect() }

    // B.0 Programming model
    rule library_element_declaration() -> Vec<LibraryElement> =
      data_types:data_type_declaration() { data_types.into_iter().map(LibraryElement::DataTypeDeclaration).collect() }
      / fbd:function_block_declaration() { vec![LibraryElement::FunctionBlockDeclaration(fbd)] }
      / fd:function_declaration() { vec![LibraryElement::FunctionDeclaration(fd)] }
      / pd:program_declaration() { vec![LibraryElement::ProgramDeclaration(pd)] }
      / cd:configuration_declaration() { vec![LibraryElement::ConfigurationDeclaration(cd)] }

    // B.1.1 Letters, digits and identifier
    //rule digit() -> &'input str = $(['0'..='9'])
    rule identifier() -> Id = start:position!() !KEYWORD() i:$(['a'..='z' | '0'..='9' | 'A'..='Z' | '_']+) end:position!() { Id::from(i).with_position(SourceLoc::range(start, end)) }

    // B.1.2 Constants
    rule constant() -> Constant =
        real:real_literal() { Constant::RealLiteral(real) }
        / integer:integer_literal() { Constant::IntegerLiteral(integer) }
        / c:character_string() { Constant::CharacterString() }
        / duration:duration() { Constant::Duration(duration) }
        / t:time_of_day() { Constant::TimeOfDay() }
        / d:date() { Constant::Date() }
        / date_time:date_and_time() { Constant::DateAndTime() }
        / bit_string:bit_string_literal() { Constant::BitStringLiteral(bit_string) }
        / boolean:boolean_literal() { Constant::Boolean(boolean) }

    // B.1.2.1 Numeric literals
    // numeric_literal omitted because it only appears in constant so we do not need to create a type for it
    rule integer_literal() -> IntegerLiteral = data_type:(t:integer_type_name() "#" {t})? value:(bi:binary_integer() { bi.into() } / oi:octal_integer() { oi.into() } / hi:hex_integer() { hi.into() } / si:signed_integer() { si }) { IntegerLiteral { value, data_type } }
    rule signed_integer__string() -> &'input str = n:$(['+' | '-']?['0'..='9']("_"? ['0'..='9'])*) { n }
    rule signed_integer() -> SignedInteger = start:position!() n:signed_integer__string() end:position!() { SignedInteger::new(n, SourceLoc::range(start, end)) }
    // TODO handle the sign
    rule integer__string() -> &'input str = n:$(['0'..='9']("_"? ['0'..='9'])*) { n }
    rule integer() -> Integer = start:position!() n:integer__string() end:position!() { Integer::new(n, SourceLoc::range(start, end)) }
    rule binary_integer_prefix() -> () = "2#" ()
    rule binary_integer() -> Integer = start:position!() binary_integer_prefix() n:$(['0'..='1']("_"? ['0'..='1'])*) end:position!() { Integer::binary(n, SourceLoc::range(start, end)) }
    rule octal_integer_prefix() -> () = "8#" ()
    rule octal_integer() -> Integer = start:position!() octal_integer_prefix() n:$(['0'..='7']("_"? ['0'..='7'])*) end:position!() { Integer::octal(n, SourceLoc::range(start, end)) }
    rule hex_integer_prefix() -> () = "16#" ()
    rule hex_integer() -> Integer = start:position!() hex_integer_prefix() n:$(['0'..='9' | 'A'..='F']("_"? ['0'..='9' | 'A'..='F'])*) end:position!() { Integer::hex(n, SourceLoc::range(start, end)) }
    rule real_literal() -> Float = tn:(t:real_type_name() "#" {t})? whole:signed_integer__string() "." fraction:integer__string() exp:exponent()? {
      // Create the value from concatenating the parts so that it is trivial
      // to existing parsers.
      let whole: String = whole.chars().filter(|c| c.is_ascii_digit()).collect();
      let fraction: String = fraction.chars().filter(|c| c.is_ascii_digit()).collect();
      let mut value = (whole + "." + &fraction).parse::<f64>().unwrap();

      if let Some(exp) = exp {
        let exp = f64::powf(exp.try_into().unwrap(), 10.0);
        value *= exp;
      }

      Float {
        value,
        data_type: tn,
      }
    }
    rule exponent() -> SignedInteger = ("E" / "e") si:signed_integer() { si }
    // bit_string_literal_type is not a rule in the specification but helps write simpler code
    rule bit_string_literal_type() -> ElementaryTypeName =
      i("BYTE") { ElementaryTypeName::BYTE }
      / i("WORD") { ElementaryTypeName::WORD }
      / i("DWORD") { ElementaryTypeName::DWORD }
      / i("LWORD") { ElementaryTypeName::LWORD }
    // The specification says unsigned_integer, but there is no such rule.
    rule bit_string_literal() -> BitStringLiteral = data_type:(t:bit_string_literal_type() "#" {t})? value:(bi:binary_integer() { bi }/ oi:octal_integer() { oi } / hi:hex_integer() { hi } / ui:integer() { ui } ) { BitStringLiteral { value, data_type } }
    rule boolean_literal() -> Boolean =
      // 1 and 0 can be a Boolean, but only with the prefix is it definitely a Boolean
      i("BOOL#1") { Boolean::True }
      / i("BOOL#0") {Boolean::False }
      / (i("BOOL#"))? i("TRUE") { Boolean::True }
      / (i("BOOL#"))? i("FALSE") { Boolean::False }
    // B.1.2.2 Character strings
    rule character_string() -> Vec<char> = s:single_byte_character_string() / d:double_byte_character_string()
    rule single_byte_character_string() -> Vec<char>  = "'" s:single_byte_character_representation()* "'" { s }
    rule double_byte_character_string() -> Vec<char> = "\"" s:double_byte_character_representation()* "\"" { s }
    // TODO escape characters
    rule single_byte_character_representation() -> char = common_character_representation()
    rule double_byte_character_representation() -> char = common_character_representation()
    rule common_character_representation() -> char = c:[' '..='!' | '#' | '%'..='&' | '('..='~'] { c }

    // B.1.2.3 Time literals
    // Omitted and subsumed into constant.

    // B.1.2.3.1 Duration
    pub rule duration() -> Duration = (i("TIME") / "T" / "t") "#" s:("-")? i:interval() {
      if let Some(sign) = s {
        return i * -1;
      }
      i
    }
    // milliseconds must come first because the "m" in "ms" would match the minutes rule
    rule interval() -> Duration = ms:milliseconds() { ms }
      / d:days() { d }
      / h:hours() { h }
      / m:minutes() { m }
      / s:seconds() { s }
    rule days() -> Duration = f:fixed_point() "d" { to_duration(f, 3600.0 * 24.0) } / i:integer() "d" "_"? h:hours() { h + to_duration(i.try_into().unwrap(), 3600.0 * 24.0) }

    rule fixed_point() -> f32 = i:integer() ("." integer())? {
      // TODO This drops the fraction, but I don't know how to keep it. May need one big regex in the worse case.
      i.try_into().unwrap()
    }
    rule hours() -> Duration = f:fixed_point() "h" { to_duration(f, 3600.0) } / i:integer() "h" "_"? m:minutes() { m + to_duration(i.try_into().unwrap(), 3600.0) }
    rule minutes() -> Duration = f:fixed_point() "m" { to_duration(f, 60.0) } / i:integer() "m" "_"? m:seconds() { m + to_duration(i.try_into().unwrap(), 60.0) }
    rule seconds() -> Duration = f:fixed_point() "s" { to_duration(f, 1.0) } / i:integer() "s" "_"? m:milliseconds() { m + to_duration(i.try_into().unwrap(), 1.0) }
    rule milliseconds() -> Duration = f:fixed_point() "ms" { to_duration(f, 0.001) }

    // 1.2.3.2 Time of day and date
    rule time_of_day() -> Time = (i("TOD") / i("TIME_OF_DAY")) "#" d:daytime() { d }
    rule daytime() -> Time = h:day_hour() ":" m:day_minute() ":" s:day_second() {
      // TODO error handling
      Time::from_hms(h.try_into().unwrap(), m.try_into().unwrap(), s.try_into().unwrap()).unwrap()
    }
    rule day_hour() -> Integer = i:integer() { i }
    rule day_minute() -> Integer = i:integer() { i }
    rule day_second() -> Integer = i:integer() { i }
    rule date() -> Date = (i("DATE") / "D" / "d") "#" d:date_literal() { d }
    rule date_literal() -> Date = y:year() "-" m:month() "-" d:day() {
      let y = y.value;
      // TODO error handling
      let m = Month::try_from(<dsl::common::Integer as TryInto<u8>>::try_into(m).unwrap()).unwrap();
      let d = d.value;
      // TODO error handling
      Date::from_calendar_date(y.try_into().unwrap(), m, d.try_into().unwrap()).unwrap()
    }
    rule year() -> Integer = i:integer() { i }
    rule month() -> Integer = i:integer() { i }
    rule day() -> Integer = i:integer() { i }
    rule date_and_time() -> PrimitiveDateTime = (i("DATE_AND_TIME") / i("DT")) "#" d:date_literal() "-" t:daytime() { PrimitiveDateTime::new(d, t) }

    // B.1.3 Data types
    // This should match generic_type_name, but that's unnecessary because
    // these are all just identifiers
    rule data_type_name() -> Id = non_generic_type_name()
    rule non_generic_type_name() -> Id = et:elementary_type_name() { et.into() } / derived_type_name()

    // B.1.3.1 Elementary data types
    rule elementary_type_name() -> ElementaryTypeName = numeric_type_name() / date_type_name() / bit_string_type_name() / elementary_string_type_name()
    rule elementary_string_type_name() -> ElementaryTypeName = i("STRING") { ElementaryTypeName::STRING } / i("WSTRING") { ElementaryTypeName::WSTRING }
    rule numeric_type_name() -> ElementaryTypeName = integer_type_name() / real_type_name()
    rule integer_type_name() -> ElementaryTypeName = signed_integer_type_name() / unsigned_integer_type_name()
    rule signed_integer_type_name() -> ElementaryTypeName = i("SINT") { ElementaryTypeName::SINT }  / i("INT") { ElementaryTypeName::INT } / i("DINT") { ElementaryTypeName::DINT } / i("LINT") { ElementaryTypeName::LINT }
    rule unsigned_integer_type_name() -> ElementaryTypeName = i("USINT") { ElementaryTypeName::USINT }  / i("UINT") { ElementaryTypeName::UINT } / i("UDINT") { ElementaryTypeName::UDINT } / i("ULINT") { ElementaryTypeName::ULINT }
    rule real_type_name() -> ElementaryTypeName = i("REAL") { ElementaryTypeName::REAL } / i("LREAL") { ElementaryTypeName::LREAL }
    rule date_type_name() -> ElementaryTypeName = i("DATE") { ElementaryTypeName::DATE } / i("TIME_OF_DAY") { ElementaryTypeName::TimeOfDay } / i("TOD") { ElementaryTypeName::TimeOfDay } / i("DATE_AND_TIME") { ElementaryTypeName::DateAndTime } / i("DT") { ElementaryTypeName::DateAndTime }
    rule bit_string_type_name() -> ElementaryTypeName = i("BOOL") { ElementaryTypeName::BOOL } / i("BYTE") { ElementaryTypeName::BYTE } / i("WORD") { ElementaryTypeName::WORD } / i("DWORD") { ElementaryTypeName::DWORD } / i("LWORD") { ElementaryTypeName::LWORD }

    // B.1.3.2
    // Rule not needed for parsing - generics are handled at a later parse stage
    // rule generic_type_name() -> &'input str = "ANY" / "ANY_DERIVED" / "ANY_ELEMENTARY" / "ANY_MAGNITUDE" / "ANY_NUM" / "ANY_REAL" / "ANY_INT" / "ANY_BOOL" / "ANY_STRING" / "ANY_DATE"

    // B.1.3.3
    // All of these are aliases for identifiers, which means the single_element_type_name will just match first
    // I've left in just in case the definition changes.
    rule derived_type_name() -> Id = single_element_type_name() / array_type_name() / structure_type_name() / string_type_name()
    rule single_element_type_name() -> Id = simple_type_name() / subrange_type_name() / enumerated_type_name()
    rule simple_type_name() -> Id = identifier()
    rule subrange_type_name() -> Id = identifier()
    rule enumerated_type_name() -> Id = identifier()
    rule array_type_name() -> Id = identifier()
    rule structure_type_name() -> Id = identifier()
    rule data_type_declaration() -> Vec<DataTypeDeclarationKind> = "TYPE" _ declarations:semisep(<type_declaration()>) _ "END_TYPE" { declarations }
    /// the type_declaration also bring in from single_element_type_declaration so that we can match in an order
    /// that identifies the type
    rule type_declaration() -> DataTypeDeclarationKind =
      a:array_type_declaration() { DataTypeDeclarationKind::Array(a) }
      / s:string_type_declaration() { DataTypeDeclarationKind::String(s) }
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
    rule structure_or_enumerated_or_simple_type_declaration__without_value() -> LateBoundDeclaration = data_type_name:identifier() _ ":" _ base_type_name:identifier() {
      LateBoundDeclaration {
        data_type_name,
        base_type_name,
      }
    }

    rule simple_type_declaration__with_constant() -> SimpleDeclaration = type_name:simple_type_name() _ ":" _ spec_and_init:simple_spec_init__with_constant() {
      SimpleDeclaration {
        type_name,
        spec_and_init,
      }
    }
    rule simple_spec_init() -> InitialValueAssignmentKind = type_name:simple_specification() _ constant:(":=" _ c:constant() { c })? {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: constant,
      })
    }
    // For simple types, they are inherently unambiguous because simple types are keywords (e.g. INT)
    rule simple_spec_init__with_constant() -> InitialValueAssignmentKind = type_name:simple_specification() _ ":=" _ constant:constant() {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: Some(constant),
      })
    }
    rule simple_specification() -> Id = et:elementary_type_name() { et.into() } / simple_type_name()
    rule subrange_type_declaration__with_range() -> SubrangeDeclaration = type_name:subrange_type_name() _ ":" _ spec:subrange_spec_init__with_range() {
      SubrangeDeclaration {
        type_name,
        spec: spec.0,
        default: spec.1,
      }
    }
    rule subrange_spec_init__with_range() -> (SubrangeSpecificationKind, Option<SignedInteger>) = spec:subrange_specification__with_range() _ default:(":=" _ def:signed_integer() { def })? {
      (spec, default)
    }
    // TODO or add a subrange type name
    rule subrange_specification__with_range() -> SubrangeSpecificationKind
      = type_name:integer_type_name() _ "(" _ subrange:subrange() _ ")" { SubrangeSpecificationKind::Specification(SubrangeSpecification{ type_name, subrange }) }
    rule subrange() -> Subrange = start:signed_integer() ".." end:signed_integer() { Subrange{start, end} }

    rule enumerated_type_declaration__with_value() -> EnumerationDeclaration =
      type_name:enumerated_type_name() _ ":" _ spec_init:enumerated_spec_init__with_value() {
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
      / type_name:enumerated_type_name() _ ":" _ spec_init:enumerated_spec_init__with_values() {
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
    rule enumerated_spec_init__with_value() -> (EnumeratedSpecificationKind, EnumeratedValue) = spec:enumerated_specification() _ ":=" _ def:enumerated_value() {
      (spec, def)
    }
    rule enumerated_spec_init__with_values() -> (EnumeratedSpecificationKind, Option<EnumeratedValue>) = spec:enumerated_specification__only_values() _ default:(":=" _ d:enumerated_value() { d })? {
      (spec, default)
    }
    rule enumerated_spec_init() -> EnumeratedSpecificationInit = spec:enumerated_specification() _ default:(":=" _ d:enumerated_value() { d })? {
      EnumeratedSpecificationInit {
        spec,
        default,
      }
    }
    // TODO this doesn't support type name as a value
    rule enumerated_specification__only_values() -> EnumeratedSpecificationKind  =
      start:position!() "(" _ v:enumerated_value() ++ (_ "," _) _ ")" end:position!() { EnumeratedSpecificationKind::values(v, SourceLoc::range(start, end)) }
    rule enumerated_specification() -> EnumeratedSpecificationKind  =
      start:position!() "(" _ v:enumerated_value() ++ (_ "," _) _ ")" end:position!() { EnumeratedSpecificationKind::values(v, SourceLoc::range(start, end)) }
      / name:enumerated_type_name() { EnumeratedSpecificationKind::TypeName(name) }
    rule enumerated_value() -> EnumeratedValue = start:position!() type_name:(name:enumerated_type_name() "#" { name })? value:identifier() end:position!() { EnumeratedValue {type_name, value, position: SourceLoc::range(start, end)} }
    rule array_type_declaration() -> ArrayDeclaration = type_name:array_type_name() _ ":" _ spec_and_init:array_spec_init() {
      ArrayDeclaration {
        type_name,
        spec: spec_and_init.spec,
        init: spec_and_init.initial_values,
      }
    }
    rule array_spec_init() -> ArrayInitialValueAssignment = spec:array_specification() _ init:(":=" _ a:array_initialization() { a })? {
      ArrayInitialValueAssignment {
        spec,
        initial_values: init.unwrap_or_default()
      }
    }
    rule array_specification() -> ArraySpecificationKind = i("ARRAY") _ "[" _ ranges:subrange() ** (_ "," _ ) _ "]" _ i("OF") _ type_name:non_generic_type_name() {
      ArraySpecificationKind::Subranges(ranges, type_name)
    }
    // TODO
    // type_name:array_type_name() {
    //  ArraySpecification::Type(type_name)
    //} /
    rule array_initialization() -> Vec<ArrayInitialElementKind> = "[" _ init:array_initial_elements() ** (_ "," _ ) _ "]" { init }
    rule array_initial_elements() -> ArrayInitialElementKind = size:integer() _ "(" ai:array_initial_element()? ")" { ArrayInitialElementKind::repeated(size, ai) } / array_initial_element()
    // TODO | structure_initialization | array_initialization
    rule array_initial_element() -> ArrayInitialElementKind = c:constant() { ArrayInitialElementKind::Constant(c) } / e:enumerated_value() { ArrayInitialElementKind::EnumValue(e) }
    rule structure_type_declaration__with_constant() -> DataTypeDeclarationKind =
      type_name:structure_type_name() _ ":" _ decl:structure_declaration() {
        DataTypeDeclarationKind::Structure(StructureDeclaration {
          type_name,
          elements: decl.elements,
        })
      }
      / type_name:structure_type_name() _ ":" _ init:initialized_structure__without_ambiguous() {
        DataTypeDeclarationKind::StructureInitialization(StructureInitializationDeclaration {
          // TODO there is something off with having two type names
          type_name,
          elements_init: init.elements_init,
        })
      }
    // structure_specification - covered in structure_type_declaration because that avoids
    // an intermediate object that doesn't know the type name
    rule initialized_structure() -> StructureInitializationDeclaration = type_name:structure_type_name() _ init:(":=" _ i:structure_initialization() {i})? {
      StructureInitializationDeclaration {
        type_name,
        elements_init: init.unwrap_or_default(),
      }
    }
    /// Same as initialized_structure but requires an initializer. Without the
    /// initializer, this is ambiguous with simple and enumeration initialization
    /// declarations.
    rule initialized_structure__without_ambiguous() -> StructureInitializationDeclaration = type_name:structure_type_name() _ ":=" _ init:structure_initialization() {
      StructureInitializationDeclaration {
        type_name,
        elements_init: init,
      }
    }
    rule structure_declaration() -> StructureDeclaration = i("STRUCT") _ elements:semisep_oneplus(<structure_element_declaration()>) _ i("END_STRUCT") {
      StructureDeclaration {
        // Requires a value but we don't know the name until level up
        type_name: Id::from(""),
        elements,
      }
    }
    rule structure_element_declaration() -> StructureElementDeclaration = name:structure_element_name() _ ":" _ init:(
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
                // TODO solve this
                initial_value: None,
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
    rule structure_initialization() -> Vec<StructureElementInit> = "(" _ elems:structure_element_initialization() ++ (_ "," _) _ ")" { elems }
    rule structure_element_initialization() -> StructureElementInit = name:structure_element_name() _ ":=" _ init:(c:constant() { StructInitialValueAssignmentKind::Constant(c) } / ev:enumerated_value() { StructInitialValueAssignmentKind::EnumeratedValue(ev) } / ai:array_initialization() { StructInitialValueAssignmentKind::Array(ai) } / si:structure_initialization() {StructInitialValueAssignmentKind::Structure(si)}) {
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
    rule simple_or_enumerated_or_subrange_ambiguous_struct_spec_init() -> InitialValueAssignmentKind = s:simple_specification() _ ":=" _ c:constant() {
      // A simple_specification with a constant is unambiguous because the constant is
      // not a valid identifier.
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name: s,
        initial_value: Some(c),
      })
    } / spec:enumerated_specification() _ ":=" _ init:enumerated_value() {
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
    } / start:position!() "(" _ values:enumerated_value() ** (_ "," _ ) _ ")" _  init:(":=" _ i:enumerated_value() {i})? {
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
    }/ i:identifier() {
      // What remains is ambiguous and the devolves to a single identifier because the prior
      // cases have captures all cases with a value. This can be simple, enumerated or struct
      InitialValueAssignmentKind::LateResolvedType(i)
    }

    rule string_type_name() -> Id = identifier()
    rule string_type_declaration() -> StringDeclaration = type_name:string_type_name() _ ":" _ width:(i("STRING") { StringKind::String } / i("WSTRING") { StringKind::WString }) _ "[" _ length:integer() _ "]" _ init:(":=" _ str:character_string() {str})? {
      StringDeclaration {
        type_name,
        length,
        width,
        init: init.map(|v| v.into_iter().collect()),
      }
    }

    // B.1.4 Variables
    rule variable() -> Variable =
      d:direct_variable() { Variable::AddressAssignment(d) }
      / symbolic_variable:symbolic_variable() { symbolic_variable.into() }
    // TODO add multi-element variable. This should probably return a different type
    #[cache_left_rec]
    rule symbolic_variable() -> SymbolicVariableKind =
      multi_element_variable()
      / name:variable_name() { SymbolicVariableKind::Named(NamedVariable{name}) }
    rule variable_name() -> Id = identifier()

    // B.1.4.1 Directly represented variables
    pub rule direct_variable() -> AddressAssignment = "%" l:location_prefix() "*" {
      AddressAssignment {
        location: l,
        size: SizePrefix::Unspecified,
        address: vec![],
      }
    } / "%" l:location_prefix() s:size_prefix()? addr:integer() ++ "." {
      let size = s.unwrap_or(SizePrefix::Nil);
      let addr = addr.iter().map(|part|
        part.value.try_into().unwrap()
      ).collect();

      AddressAssignment {
        location: l,
        size,
        address: addr,
      }
    }
    rule location_prefix() -> LocationPrefix =
      ("I" / "i") { LocationPrefix::I }
      / ("Q" / "q" ) { LocationPrefix::Q }
      / ("M" / "m") { LocationPrefix::M }
    rule size_prefix() -> SizePrefix =
      ("X" / "x") { SizePrefix::X }
      / ("B" / "b") { SizePrefix::B }
      / ("W" / "w") { SizePrefix::W }
      / ("D" / "d") { SizePrefix::D }
      / ("L" / "l") { SizePrefix::L }
    // B.1.4.2 Multi-element variables
    #[cache_left_rec]
    rule multi_element_variable() -> SymbolicVariableKind =
      array_variable:array_variable() {
        SymbolicVariableKind::Array(array_variable)
      }
      / sv:structured_variable() {
        // TODO this is clearly wrong
        SymbolicVariableKind::Structured(vec![
          sv.0,
          sv.1,
        ])
      }
    #[cache_left_rec]
    rule array_variable() -> ArrayVariable = variable:subscripted_variable() _ subscripts:subscript_list() {
      ArrayVariable {
        variable: Box::new(variable),
        subscripts,
      }
    }
    //#[cache_left_rec]
    // TODO this is wrong!!
    rule subscripted_variable() -> SymbolicVariableKind = name:variable_name() { SymbolicVariableKind::Named(NamedVariable{ name }) }
    rule subscript_list() -> Vec<ExprKind> = "[" _ list:subscript()++ (_ "," _) _ "]" { list }
    rule subscript() -> ExprKind = expression()
    rule structured_variable() -> (Id, Id) = r:record_variable() "." f:field_selector() { (r, f)}
    // TODO this is definitely wrong but it unblocks for now
    // very likely need to make this a repeated item with ++
    rule record_variable() -> Id = identifier()
    rule field_selector() -> Id = identifier()

    // B.1.4.3 Declarations and initialization
    pub rule input_declarations() -> Vec<VarDecl> = i("VAR_INPUT") _ qualifier:(i("RETAIN") {DeclarationQualifier::Retain} / i("NON_RETAIN") {DeclarationQualifier::NonRetain})? _ declarations:semisep(<input_declaration()>) _ i("END_VAR") {
      VarDeclarations::flat_map(declarations, VariableType::Input, qualifier)
    }
    // TODO add edge declaration (as a separate item - a tuple)
    rule input_declaration() -> Vec<UntypedVarDecl> = var_init_decl()
    rule edge_declaration() -> () = var1_list() _ ":" _ i("BOOL") _ (i("R_EDGE") / i("F_EDGE"))? {}
    // TODO the problem is we match first, then
    // TODO missing multiple here
    // We have to first handle the special case of enumeration or fb_name without an initializer
    // because these share the same syntax. We only know the type after trying to resolve the
    // type name.
    rule var_init_decl() -> Vec<UntypedVarDecl> = structured_var_init_decl__without_ambiguous() / string_var_declaration() / array_var_init_decl() /  var1_init_decl__with_ambiguous_struct()

    // TODO add in subrange_spec_init(), enumerated_spec_init()

    rule var1_init_decl__with_ambiguous_struct() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ init:(a:simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()) end:position!() {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: init.clone(),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }

    rule var1_list() -> Vec<Id> = names:variable_name() ++ (_ "," _) { names }
    rule structured_var_init_decl() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ init_struct:initialized_structure() end:position!() {
      names.into_iter().map(|name| {
        // TODO
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Structure(init_struct.clone()),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule structured_var_init_decl__without_ambiguous() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ init_struct:initialized_structure__without_ambiguous() end:position!() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Structure(init_struct.clone()),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule array_var_init_decl() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ array_spec_init() end:position!() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::None,
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule fb_name() -> Id = i:identifier() { i }
    pub rule output_declarations() -> Vec<VarDecl> = i("VAR_OUTPUT") _ qualifier:(i("RETAIN") {DeclarationQualifier::Retain} / i("NON_RETAIN") {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ i("END_VAR") {
      VarDeclarations::flat_map(declarations, VariableType::Output, qualifier)
    }
    pub rule input_output_declarations() -> Vec<VarDecl> = i("VAR_IN_OUT") _ qualifier:(i("RETAIN") {DeclarationQualifier::Retain} / i("NON_RETAIN") {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ i("END_VAR") {
      VarDeclarations::flat_map(declarations, VariableType::InOut,  qualifier)
    }
    rule var_declarations() -> VarDeclarations = i("VAR") _ qualifier:(i("CONSTANT") {DeclarationQualifier::Constant})? _ declarations:semisep(<var_init_decl()>) _ i("END_VAR") {
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule retentive_var_declarations() -> VarDeclarations = i("VAR") _ i("RETAIN") _ declarations:semisep(<var_init_decl()>) _ i("END_VAR") {
      let qualifier = Option::Some(DeclarationQualifier::Retain);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule located_var_declarations() -> VarDeclarations = i("VAR") _ qualifier:(i("CONSTANT") { DeclarationQualifier::Constant } / i("RETAIN") {DeclarationQualifier::Retain} / i("NON_RETAIN") {DeclarationQualifier::NonRetain})? _ declarations:semisep(<located_var_decl()>) _ i("END_VAR") {
      let qualifier = qualifier.or(Some(DeclarationQualifier::Unspecified));
      VarDeclarations::Located(VarDeclarations::map_located(declarations, qualifier))
    }
    rule located_var_decl() -> LocatedVarDecl = start:position!() name:variable_name()? _ location:location() _ ":" _ initializer:located_var_spec_init() end:position!() {
      LocatedVarDecl {
        name,
        qualifier: DeclarationQualifier::Unspecified,
        location,
        initializer,
        position: SourceLoc::range(start, end),
      }
    }
    // We use the same type as in other places for VarInit, but the external always omits the initializer
    rule external_var_declarations() -> VarDeclarations = i("VAR_EXTERNAL") _ qualifier:(i("CONSTANT") {DeclarationQualifier::Constant})? _ declarations:semisep(<external_declaration()>) _ i("END_VAR") {
      VarDeclarations::External(VarDeclarations::map(declarations, qualifier))
    }
    // TODO subrange_specification, array_specification(), structure_type_name and others
    rule external_declaration_spec() -> InitialValueAssignmentKind = type_name:simple_specification() {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: None,
      })
    }
    rule external_declaration() -> VarDecl = start:position!() name:global_var_name() _ ":" _ spec:external_declaration_spec() end:position!() {
      VarDecl {
        name,
        var_type: VariableType::External,
        qualifier: DeclarationQualifier::Unspecified,
        initializer: spec,
        position: SourceLoc::range(start, end),
      }
    }
    rule global_var_name() -> Id = i:identifier() { i }

    rule qualifier() -> DeclarationQualifier = i("CONSTANT") { DeclarationQualifier::Constant } / i("RETAIN") { DeclarationQualifier::Retain }
    pub rule global_var_declarations() -> Vec<VarDecl> = i("VAR_GLOBAL") _ qualifier:qualifier()? _ declarations:semisep(<global_var_decl()>) _ i("END_VAR") {
      // TODO set the options - this is pretty similar to VarInit - maybe it should be the same
      let declarations = declarations.into_iter().flatten();
      declarations.into_iter().map(|declaration| {
        let qualifier = qualifier.clone().unwrap_or(DeclarationQualifier::Unspecified);
        let mut declaration = declaration;
        declaration.qualifier = qualifier;
        declaration
      }).collect()
    }
    // TODO this doesn't pass all information. I suspect the rule from the dpec is not right
    rule global_var_decl() -> (Vec<VarDecl>) = start:position!() vs:global_var_spec() _ ":" _ initializer:(l:located_var_spec_init() { l } / f:function_block_type_name() { InitialValueAssignmentKind::FunctionBlock(FunctionBlockInitialValueAssignment{type_name: f})})? end:position!() {
      vs.0.into_iter().map(|name| {
        let init = initializer.clone().unwrap_or(InitialValueAssignmentKind::None);
        VarDecl {
          name,
          var_type: VariableType::Global,
          qualifier: DeclarationQualifier::Unspecified,
          // TODO this is clearly wrong
          initializer: init,
          // TODO this is clearly wrong
          position: SourceLoc::range(start, end),
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
    // TODO
    pub rule location() -> AddressAssignment = i("AT") _ v:direct_variable() { v }
    rule global_var_list() -> Vec<Id> = names:global_var_name() ++ (_ "," _) { names }
    rule string_var_declaration() -> Vec<UntypedVarDecl> = single_byte_string_var_declaration() / double_byte_string_var_declaration()
    rule single_byte_string_var_declaration() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ spec:single_byte_string_spec() end:position!() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::String(spec.clone()),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule single_byte_string_spec() -> StringInitializer = i("STRING") _ length:("[" _ i:integer() _ "]" {i})? _ initial_value:(":=" _ v:single_byte_character_string() {v})? {
      StringInitializer {
        length,
        width: StringKind::String,
        initial_value,
      }
    }
    rule double_byte_string_var_declaration() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ spec:double_byte_string_spec() end:position!() {
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::String(spec.clone()),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule double_byte_string_spec() -> StringInitializer = i("WSTRING") _ length:("[" _ i:integer() _ "]" {i})? _ initial_value:(":=" _ v:double_byte_character_string() {v})? {
      StringInitializer {
        length,
        width: StringKind::WString,
        initial_value,
      }
    }

    // B.1.5.1 Functions
    rule function_name() -> Id = standard_function_name() / derived_function_name()
    rule standard_function_name() -> Id = identifier()
    rule derived_function_name() -> Id = identifier()
    rule function_declaration() -> FunctionDeclaration = i("FUNCTION") _  name:derived_function_name() _ ":" _ rt:(et:elementary_type_name() { et.into() } / dt:derived_type_name() { dt }) _ var_decls:(io:io_var_declarations() / func:function_var_decls()) ** _ _ body:function_body() _ i("END_FUNCTION") {
      let (variables, located) = VarDeclarations::unzip(var_decls);
      FunctionDeclaration {
        name,
        return_type: rt,
        variables,
        body,
      }
    }
    rule io_var_declarations() -> VarDeclarations = i:input_declarations() { VarDeclarations::Inputs(i) } / o:output_declarations() { VarDeclarations::Outputs(o) } / io:input_output_declarations() { VarDeclarations::Inouts(io) }
    rule function_var_decls() -> VarDeclarations = i("VAR") _ qualifier:(i("CONSTANT") {DeclarationQualifier::Constant})? _ vars:semisep_oneplus(<var2_init_decl()>) _ i("END_VAR") {
      VarDeclarations::Var(VarDeclarations::flat_map(vars, VariableType::Var, qualifier))
    }
    // TODO a bunch are missing here
    rule function_body() -> Vec<StmtKind> = statement_list()
    // TODO add many types here
    rule var2_init_decl() -> Vec<UntypedVarDecl> = var1_init_decl__with_ambiguous_struct()

    // B.1.5.2 Function blocks
    // IEC 61131 defines separate standard and derived function block names,
    // but we don't need that distinction here.
    rule function_block_type_name() -> Id = i:identifier() { i }
    rule derived_function_block_name() -> Id = !STANDARD_FUNCTION_BLOCK_NAME() i:identifier() { i }
    // TODO add variable declarations
    rule function_block_declaration() -> FunctionBlockDeclaration = start:position!() i("FUNCTION_BLOCK") _ name:derived_function_block_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other }) ** _ _ body:function_block_body() _ i("END_FUNCTION_BLOCK") end:position!() {
      let (variables, located) = VarDeclarations::unzip(decls);
      FunctionBlockDeclaration {
        name,
        variables,
        // TODO located?
        body,
        position: SourceLoc::range(start, end),
      }
    }
    // TODO there are far more here
    rule other_var_declarations() -> VarDeclarations = external_var_declarations() / var_declarations()
    rule function_block_body() -> FunctionBlockBody = networks:sequential_function_chart() { FunctionBlockBody::sfc(networks) } / statements:statement_list() { FunctionBlockBody::stmts(statements) } / _ { FunctionBlockBody::empty( )}

    // B.1.5.3 Program declaration
    rule program_type_name() -> Id = i:identifier() { i }
    pub rule program_declaration() ->  ProgramDeclaration = i("PROGRAM") _ p:program_type_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other } / located:located_var_declarations() { located }) ** _ _ body:function_block_body() _ i("END_PROGRAM") {
      let (variables, located) = VarDeclarations::unzip(decls);
      ProgramDeclaration {
        type_name: p,
        variables,
        // TODO more
        body,
      }
    }

    // B.1.6 Sequential function chart elements
    // TODO return something
    pub rule sequential_function_chart() -> Vec<Network> = networks:sfc_network() ++ _ { networks }
    // TOD add transition and action
    rule sfc_network() ->  Network = init:initial_step() _ elements:((s:step() {s } / a:action() {a} / t:transition() {t}) ** _) {
      Network {
        initial_step: init,
        elements
      }
    }
    rule initial_step() -> Step = i("INITIAL_STEP") _ name:step_name() _ ":" _ action_associations:action_association() ** (_ ";" _) i("END_STEP") {
      Step{
        name,
        action_associations,
       }
    }
    rule step() -> ElementKind = i("STEP") _ name:step_name() _ ":" _ action_associations:semisep(<action_association()>) _ i("END_STEP") {
      ElementKind::step(
        name,
        action_associations
      )
    }
    rule step_name() -> Id = identifier()
    // TODO this is missing stuff
    rule action_association() -> ActionAssociation = name:action_name() _ "(" _ qualifier:action_qualifier()? _ indicators:("," _ i:indicator_name() ** (_ "," _) { i })? _ ")" {
      ActionAssociation {
        name,
        qualifier,
        indicators: indicators.unwrap_or_default(),
      }
    }
    rule action_name() -> Id = identifier()
    rule action_qualifier() -> ActionQualifier =
      i("N") { ActionQualifier::N }
      / i("R") { ActionQualifier::R }
      / i("S") { ActionQualifier::S }
      / i("L") { ActionQualifier::L }
      / i("D") { ActionQualifier::D }
      / i("P") { ActionQualifier::P }
      / i("SD") { ActionQualifier::SD }
      / i("DS") { ActionQualifier::DS }
      / i("SL") { ActionQualifier::SL }
      / i("P1") { ActionQualifier::PR }
      / i("P0") { ActionQualifier::PF }
    rule indicator_name() -> Id = variable_name()
    rule transition() -> ElementKind = i("TRANSITION") _ name:transition_name()? _ priority:("(" _ i("PRIORITY") _ ":=" _ p:integer() _ ")" {p})? _ i("FROM") _ from:steps() _ i("TO") _ to:steps() _ condition:transition_condition() _ i("END_TRANSITION") {
      ElementKind::Transition(Transition {
        name,
        priority: priority.map(|p| p.value.try_into().unwrap()),
        from,
        to,
        condition,
      }
    )}
    rule transition_name() -> Id = identifier()
    rule steps() -> Vec<Id> = name:step_name() {
      vec![name]
    } / "(" _ n1:step_name() _ "," _ n2:step_name() _ nr:("," _ n:step_name()) ** _ _ ")" {
      // TODO need to extend with nr
      vec![n1, n2]
    }
    // TODO add simple_instruction_list , fbd_network, rung
    rule transition_condition() -> ExprKind =  ":=" _ expr:expression() _ ";" { expr }
    rule action() -> ElementKind = i("ACTION") _ name:action_name() _ ":" _ body:function_block_body() _ i("END_ACTION") {
      ElementKind::Action(Action {
        name,
        body
      })
    }

    // B.1.7 Configuration elements
    rule configuration_name() -> Id = i:identifier() { i }
    rule resource_type_name() -> Id = i:identifier() { i }
    pub rule configuration_declaration() -> ConfigurationDeclaration = i("CONFIGURATION") _ n:configuration_name() _ g:global_var_declarations()? _ r:resource_declaration() _ i("END_CONFIGURATION") {
      let g = g.unwrap_or_default();
      // TODO this should really be multiple items
      let r = vec![r];
      ConfigurationDeclaration {
        name: n,
        global_var: g,
        resource_decl: r,
      }
    }
    rule resource_declaration() -> ResourceDeclaration = i("RESOURCE") _ n:resource_name() _ i("ON") _ t:resource_type_name() _ g:global_var_declarations()? _ resource:single_resource_declaration() _ i("END_RESOURCE") {
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
    rule resource_name() -> Id = i:identifier() { i }
    rule program_name() -> Id = i:identifier() { i }
    pub rule task_configuration() -> TaskConfiguration = i("TASK") _ name:task_name() _ init:task_initialization() {
      TaskConfiguration {
        name,
        priority: init.0,
        // TODO This needs to set the interval
        interval: init.1,
      }
    }
    rule task_name() -> Id = i:identifier() { i }
    // TODO add single and interval
    pub rule task_initialization() -> (u32, Option<Duration>) = "(" _ interval:task_initialization_interval()? _ priority:task_initialization_priority() _ ")" { (priority, interval) }
    rule task_initialization_interval() -> Duration = i("INTERVAL") _ ":=" _ source:data_source() _ "," {
      // TODO The interval may not necessarily be a duration, but for now, only support Duration types
      match source {
        Constant::Duration(duration) => duration,
        _ => panic!("Only supporting Duration types for now"),
      }
     }
    rule task_initialization_priority() -> u32 = i("PRIORITY") _ ":=" _ i:integer() { i.value.try_into().unwrap() }
    // TODO there are more here, but only supporting Constant for now
    pub rule data_source() -> Constant = constant:constant() { constant }
    // TODO more options here
    //pub rule data_source() -> &'input str =
    pub rule program_configuration() -> ProgramConfiguration = i("PROGRAM") _ name:program_name() task_name:( _ i("WITH") _ t:task_name() { t })? _ ":" _ pt:program_type_name() (_ "(" _ c:prog_conf_element() ** (_ "," _) _ ")")? {
      ProgramConfiguration {
        name,
        task_name,
        type_name: pt,
      }
     }
    rule prog_conf_element() -> Id = t:fb_task() { t.0 } /*/ p:prog_cnxn() { p }*/
    rule fb_task() -> (Id, Id) = n:fb_name() _ i("WITH") _ tn:task_name() { (n, tn) }

    // B.3.1 Expressions
    pub rule expression() -> ExprKind = precedence!{
      // or_expression
      x:(@) _ i("OR") _ y:@ { ExprKind::compare(CompareOp::Or, x, y) }
      --
      // xor_expression
      x:(@) _ i("XOR") _ y:@ { ExprKind::compare(CompareOp::Xor, x, y) }
      --
      // and_expression
      x:(@) _ "&" _ y:@ { ExprKind::compare(CompareOp::And, x, y ) }
      x:(@) _ i("AND") _ y:@ { ExprKind::compare(CompareOp::And, x, y ) }
      --
      // comparison
      x:(@) _ "=" _ y:@ { ExprKind::compare(CompareOp::Eq, x, y ) }
      x:(@) _ "<>" _ y:@ { ExprKind::compare(CompareOp::Ne, x, y ) }
      --
      // equ_expression
      x:(@) _ "<" _ y:@ { ExprKind::compare(CompareOp::Lt, x, y ) }
      x:(@) _ ">" _ y:@ { ExprKind::compare(CompareOp::Gt, x, y ) }
      x:(@) _ "<=" _ y:@ { ExprKind::compare(CompareOp::LtEq, x, y) }
      x:(@) _ ">=" _ y:@ { ExprKind::compare(CompareOp::GtEq, x, y) }
      --
      // add_expression
      x:(@) _ "+" _ y:@ { ExprKind::binary(Operator::Add, x, y ) }
      x:(@) _ "-" _ y:@ { ExprKind::binary(Operator::Sub, x, y ) }
      --
      // multiply_operator
      x:(@) _ "*" _ y:@ { ExprKind::binary(Operator::Mul, x, y ) }
      x:(@) _ "/" _ y:@ { ExprKind::binary(Operator::Div, x, y ) }
      x:(@) _ i("MOD") _ y:@ { ExprKind::binary(Operator::Mod, x, y ) }
      --
      // power_expression
      x:(@) _ "**" _ y:@ { ExprKind::binary(Operator::Pow, x, y ) }
      --
      //unary_expression
      p:unary_expression() { p }
      --
      // primary_expression
      c:constant() { ExprKind::Const(c) }
      //ev:enumerated_value()
      v:variable() { ExprKind::Variable(v) }
      "(" _ e:expression() _ ")" { ExprKind::Expression(Box::new(e)) }
      f:function_expression() { f }
    }
    rule unary_expression() -> ExprKind = unary:unary_operator()? _ expr:primary_expression() {
      if let Some(op) = unary {
        return ExprKind::unary(op, expr);
      }
      expr
    }
    rule unary_operator() -> UnaryOp = "-" {UnaryOp::Neg} / i("NOT") {UnaryOp::Not}
    rule primary_expression() -> ExprKind
      = constant:constant() {
          ExprKind::Const(constant)
        }
      // TODO enumerated value
      / function:function_expression() {
          function
        }
      / variable:variable() {
        ExprKind::Variable(variable)
      }
      / "(" _ expression:expression() _ ")" {
        expression
      }
    rule function_expression() -> ExprKind = name:function_name() _ "(" _ params:param_assignment() ** (_ "," _) _ ")" {
      ExprKind::Function {
        name,
        param_assignment: params
      }
    }

    // B.3.2 Statements
    pub rule statement_list() -> Vec<StmtKind> = statements:semisep(<statement()>) { statements }
    rule statement() -> StmtKind = assignment_statement() / selection_statement() / iteration_statement() / subprogram_control_statement()

    // B.3.2.1 Assignment statements
    pub rule assignment_statement() -> StmtKind = var:variable() _ ":=" _ expr:expression() { StmtKind::assignment(var, expr) }

    // B.3.2.2 Subprogram control statements
    rule subprogram_control_statement() -> StmtKind = fb:fb_invocation() { fb } / i("RETURN") { StmtKind::Return }
    rule fb_invocation() -> StmtKind = start:position!() name:fb_name() _ "(" _ params:param_assignment() ** (_ "," _) _ ")" end:position!() {
      StmtKind::FbCall(FbCall {
        var_name: name,
        params,
        position: SourceLoc::range(start, end)
      })
    }
    // TODO this needs much more
    rule param_assignment() -> ParamAssignmentKind = not:(i("NOT") {})? _ src:variable_name() _ "=>" _ tgt:variable() {
      ParamAssignmentKind::Output (
        Output{
        not: false,
        src,
        tgt,
      })
    } / name:(n:variable_name() _ ":=" { n })? _ expr:expression() {
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
    rule if_statement() -> StmtKind = i("IF") _ expr:expression() _ i("THEN") _ body:statement_list()? _ else_ifs:(i("ELSIF") expr:expression() _ i("THEN") _ body:statement_list() {(expr, body)}) ** _ _ else_body:("ELSE" _ e:statement_list() { e })? _ "END_IF" {
      StmtKind::If(If {
        expr,
        body: body.unwrap_or_default(),
        else_ifs,
        else_body: else_body.unwrap_or_default()
      })
    }
    rule case_statement() -> StmtKind = i("CASE") _ selector:expression() _ i("OF") _ cases:case_element() ** _ _ else_body:(i("ELSE") _ e:statement_list() { e })? _ i("END_CASE") {
      StmtKind::Case(Case {
        selector,
        statement_groups: cases,
        else_body: else_body.unwrap_or_default(),
      })
    }
    rule case_element() -> CaseStatementGroup = selectors:case_list() _ ":" _ statements:statement_list() {
      CaseStatementGroup {
        selectors,
        statements,
      }
    }
    rule case_list() -> Vec<CaseSelection> = cases_list:case_list_element() ++ (_ "," _) { cases_list }
    rule case_list_element() -> CaseSelection = sr:subrange() {CaseSelection::Subrange(sr)} / si:signed_integer() {CaseSelection::SignedInteger(si)} / ev:enumerated_value() {CaseSelection::EnumeratedValue(ev)}

    // B.3.2.4 Iteration statements
    rule iteration_statement() -> StmtKind = f:for_statement() {StmtKind::For(f)} / w:while_statement() {StmtKind::While(w)} / r:repeat_statement() {StmtKind::Repeat(r)} / exit_statement()
    rule for_statement() -> For = i("FOR") _ control:control_variable() _ ":=" _ range:for_list() _ i("DO") _ body:statement_list() _ i("END_FOR") {
      For {
        control,
        from: range.0,
        to: range.1,
        step: range.2,
        body,
      }
    }
    rule control_variable() -> Id = identifier()
    rule for_list() -> (ExprKind, ExprKind, Option<ExprKind>) = from:expression() _ i("TO") _ to:expression() _ step:(i("BY") _ s:expression() {s})? { (from, to, step) }
    rule while_statement() -> While = i("WHILE") _ condition:expression() _ i("DO") _ body:statement_list() _ i("END_WHILE") {
      While {
        condition,
        body,
      }
    }
    rule repeat_statement() -> Repeat = i("REPEAT") _ body:statement_list() _ i("UNTIL") _ until:expression() _ i("END_REPEAT") {
      Repeat {
        until,
        body,
      }
    }
    // TODO
    rule exit_statement() -> StmtKind = i("EXIT") { StmtKind::Exit }

  }
}

mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn input_declarations_simple() {
        // TODO enumerations - I think we need to be lazy here and make simple less strict
        let decl = "VAR_INPUT
        TRIG : BOOL;
        MSG : STRING;
      END_VAR";
        let vars = vec![
            VarDecl {
                name: Id::from("TRIG"),
                var_type: VariableType::Input,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::simple_uninitialized("BOOL"),
                position: SourceLoc::default(),
            },
            VarDecl {
                name: Id::from("MSG"),
                var_type: VariableType::Input,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::String(StringInitializer {
                    length: None,
                    width: StringKind::String,
                    initial_value: None,
                }),
                position: SourceLoc::default(),
            },
        ];
        assert_eq!(plc_parser::input_declarations(decl), Ok(vars))
    }

    #[test]
    fn input_declarations_custom_type() {
        let decl = "VAR_INPUT
LEVEL : LOGLEVEL := INFO;
END_VAR";
        let expected = Ok(vec![VarDecl {
            name: Id::from("LEVEL"),
            var_type: VariableType::Input,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::EnumeratedType(
                EnumeratedInitialValueAssignment {
                    type_name: Id::from("LOGLEVEL"),
                    initial_value: Some(EnumeratedValue::new("INFO")),
                },
            ),
            position: SourceLoc::default(),
        }]);
        assert_eq!(plc_parser::input_declarations(decl), expected)
    }

    #[test]
    fn output_declarations() {
        let decl = "VAR_OUTPUT
        TRIG : BOOL;
        MSG : STRING;
      END_VAR";
        let vars = vec![
            VarDecl {
                name: Id::from("TRIG"),
                var_type: VariableType::Output,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::simple_uninitialized("BOOL"),
                position: SourceLoc::default(),
            },
            VarDecl {
                name: Id::from("MSG"),
                var_type: VariableType::Output,
                qualifier: DeclarationQualifier::Unspecified,
                initializer: InitialValueAssignmentKind::String(StringInitializer {
                    length: None,
                    width: StringKind::String,
                    initial_value: None,
                }),
                position: SourceLoc::default(),
            },
        ];
        assert_eq!(plc_parser::output_declarations(decl), Ok(vars))
    }

    #[test]
    fn data_source() {
        assert_eq!(
            plc_parser::duration("T#100ms"),
            Ok(Duration::new(0, 100_000_000))
        )
    }

    #[test]
    fn task_configuration() {
        let config = Ok(TaskConfiguration {
            name: Id::from("abc"),
            priority: 11,
            interval: None,
        });
        assert_eq!(
            plc_parser::task_configuration("TASK abc (PRIORITY:=11)"),
            config
        );
        assert_eq!(
            plc_parser::task_configuration("TASK abc (PRIORITY:=1_1)"),
            config
        );
    }

    #[test]
    fn task_initialization() {
        assert_eq!(
            plc_parser::task_initialization("(PRIORITY:=11)"),
            Ok((11, None))
        );
        assert_eq!(
            plc_parser::task_initialization("(PRIORITY:=1_1)"),
            Ok((11, None))
        );
    }

    #[test]
    fn program_configuration() {
        // TODO there is more to extract here
        let cfg = ProgramConfiguration {
            name: Id::from("plc_task_instance"),
            task_name: Option::Some(Id::from("plc_task")),
            type_name: Id::from("plc_prg"),
        };
        assert_eq!(
            plc_parser::program_configuration("PROGRAM plc_task_instance WITH plc_task : plc_prg"),
            Ok(cfg)
        );
    }

    #[test]
    fn direct_variable() {
        let address = vec![1];
        let var = AddressAssignment {
            location: LocationPrefix::I,
            size: SizePrefix::X,
            address,
        };
        assert_eq!(plc_parser::direct_variable("%IX1"), Ok(var))
    }

    #[test]
    fn location() {
        let address = vec![1];
        let var = AddressAssignment {
            location: LocationPrefix::I,
            size: SizePrefix::X,
            address,
        };
        assert_eq!(plc_parser::location("AT %IX1"), Ok(var))
    }

    #[test]
    fn var_global() {
        // TODO assign the right values
        let reset = vec![VarDecl {
            name: Id::from("ResetCounterValue"),
            var_type: VariableType::Global,
            qualifier: DeclarationQualifier::Constant,
            initializer: InitialValueAssignmentKind::simple("INT", Constant::integer_literal("17")),
            position: SourceLoc::default(),
        }];
        assert_eq!(
            plc_parser::global_var_declarations(
                "VAR_GLOBAL CONSTANT ResetCounterValue : INT := 17; END_VAR"
            ),
            Ok(reset)
        );
    }

    #[test]
    fn sequential_function_chart() {
        let sfc = "INITIAL_STEP Start:
      END_STEP
      STEP ResetCounter:
        RESETCOUNTER_INLINE1(N);
        RESETCOUNTER_INLINE2(N);
      END_STEP
      ACTION RESETCOUNTER_INLINE1:
    Cnt := ResetCounterValue;
  END_ACTION
  TRANSITION FROM ResetCounter TO Start
    := NOT Reset;
  END_TRANSITION
  TRANSITION FROM Start TO Count
    := NOT Reset;
  END_TRANSITION
  STEP Count:
    COUNT_INLINE3(N);
    COUNT_INLINE4(N);
  END_STEP";
        let expected = Ok(vec![Network {
            initial_step: Step {
                name: Id::from("Start"),
                action_associations: vec![],
            },
            elements: vec![
                ElementKind::step(
                    Id::from("ResetCounter"),
                    vec![
                        ActionAssociation {
                            name: Id::from("RESETCOUNTER_INLINE1"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                        ActionAssociation {
                            name: Id::from("RESETCOUNTER_INLINE2"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                    ],
                ),
                ElementKind::action(
                    "RESETCOUNTER_INLINE1",
                    vec![StmtKind::assignment(
                        Variable::named("Cnt"),
                        ExprKind::named_variable("ResetCounterValue"),
                    )],
                ),
                ElementKind::transition(
                    "ResetCounter",
                    "Start",
                    ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("Reset")),
                ),
                ElementKind::transition(
                    "Start",
                    "Count",
                    ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("Reset")),
                ),
                ElementKind::step(
                    Id::from("Count"),
                    vec![
                        ActionAssociation {
                            name: Id::from("COUNT_INLINE3"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                        ActionAssociation {
                            name: Id::from("COUNT_INLINE4"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                    ],
                ),
            ],
        }]);
        assert_eq!(plc_parser::sequential_function_chart(sfc), expected);
    }

    #[test]
    fn statement_assign_constant() {
        let expected = Ok(vec![StmtKind::assignment(
            Variable::named("Cnt"),
            ExprKind::integer_literal("1"),
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := 1;"), expected)
    }

    #[test]
    fn statement_assign_add_const_operator() {
        let expected = Ok(vec![StmtKind::assignment(
            Variable::named("Cnt"),
            ExprKind::binary(
                Operator::Add,
                ExprKind::integer_literal("1"),
                ExprKind::integer_literal("2"),
            ),
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := 1 + 2;"), expected)
    }

    #[test]
    fn statement_assign_add_symbol_operator() {
        let expected = Ok(vec![StmtKind::assignment(
            Variable::named("Cnt"),
            ExprKind::binary(
                Operator::Add,
                ExprKind::named_variable("Cnt"),
                ExprKind::integer_literal("1"),
            ),
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := Cnt + 1;"), expected)
    }

    #[test]
    fn statement_if_multi_term() {
        let statement = "IF TRIG AND NOT TRIG THEN
      TRIG0:=TRIG;

    END_IF;";
        let expected = Ok(vec![StmtKind::if_then(
            ExprKind::compare(
                CompareOp::And,
                ExprKind::named_variable("TRIG"),
                ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("TRIG")),
            ),
            vec![StmtKind::assignment(
                Variable::named("TRIG0"),
                ExprKind::named_variable("TRIG"),
            )],
        )]);
        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_if() {
        let statement = "IF Reset THEN
    Cnt := ResetCounterValue;
  ELSE
    Cnt := Cnt + 1;
  END_IF;";
        let expected = Ok(vec![StmtKind::if_then_else(
            ExprKind::named_variable("Reset"),
            vec![StmtKind::assignment(
                Variable::named("Cnt"),
                ExprKind::named_variable("ResetCounterValue"),
            )],
            vec![StmtKind::assignment(
                Variable::named("Cnt"),
                ExprKind::binary(
                    Operator::Add,
                    ExprKind::named_variable("Cnt"),
                    ExprKind::integer_literal("1"),
                ),
            )],
        )]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_without_name() {
        let statement = "CounterLD0(Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            var_name: Id::from("CounterLD0"),
            params: vec![ParamAssignmentKind::positional(ExprKind::named_variable(
                "Reset",
            ))],
            position: SourceLoc::default(),
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_with_name() {
        let statement = "CounterLD0(Cnt := Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            var_name: Id::from("CounterLD0"),
            params: vec![ParamAssignmentKind::named(
                "Cnt",
                ExprKind::named_variable("Reset"),
            )],
            position: SourceLoc::default(),
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn assignment() {
        let assign = "Cnt1 := CounterST0.OUT";
        let expected = Ok(StmtKind::assignment(
            Variable::named("Cnt1"),
            ExprKind::Variable(Variable::Structured(vec![
                Id::from("CounterST0"),
                Id::from("OUT"),
            ])),
        ));
        assert_eq!(plc_parser::assignment_statement(assign), expected)
    }
}
