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

use dsl::core::SourceLoc;
use peg::parser;

use crate::error::{Location, ParserDiagnostic};
use crate::mapper::*;
use ironplc_dsl::common::*;
use ironplc_dsl::common_sfc::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::*;

// Don't use std::time::Duration because it does not allow negative values.
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

use std::collections::HashSet;

/// Parses a IEC 61131-3 library into object form.
pub fn parse_library(source: &str) -> Result<Vec<LibraryElement>, ParserDiagnostic> {
    plc_parser::library(source).map_err(|e| ParserDiagnostic {
        location: Location {
            line: e.location.line,
            column: e.location.column,
            offset: e.location.offset,
        },
        expected: HashSet::from_iter(e.expected.tokens()),
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
    rule _ = [' ' | '\n' | '\r' ]*

    // A semi-colon separated list with required ending separator
    rule semisep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ semicolon() _)) _ semicolon() {v}
    rule semisep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ semicolon() _)) semicolon() {v}
    rule commasep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ comma() _)) comma() {v}

    rule KEYWORD() = "ACTION" / "END_ACTION" / "ARRAY" / "OF" / "AT" / "CASE"
                     / "ELSE" / "END_CASE" / "CONFIGURATION" / "END_CONFIGURATION"
                     / "CONSTANT" / "EN" / "ENO" / "EXIT" / "FALSE" / "F_EDGE"
                     / "FOR" / "TO" / "BY" / "DO" / "END_FOR" / "FUNCTION" / "END_FUNCTION"
                     / "FUNCTION_BLOCK" / "END_FUNCTION_BLOCK" / "IF" / "THEN"
                     / "ELSIF" / "ELSE" / "END_IF" / "INITIAL_STEP" / "END_STEP"
                     / "NOT" / "MOD" / "AND" / "XOR" / "OR" / "PROGRAM" / "END_PROGRAM"
                     / "R_EDGE" / "READ_ONLY" / "READ_WRITE" / "REPEAT" / "UNTIL"
                     / "END_REPEAT" / "RESOURCE" / "END_RESOURCE" / "RETAIN" / "NON_RETAIN"
                     / "RETURN" / "STEP" / "END_STEP" / "STRUCT" / "END_STRUCT"
                     / "TASK" / "TRANSITION" / "FROM" / "END_TRANSITION" / "TRUE"
                     / "VAR" / "END_VAR" / "VAR_INPUT" / "VAR_OUTPUT" / "VAR_IN_OUT"
                     / "VAR_TEMP" / "VAR_EXTERNAL" / "VAR_ACCESS" / "VAR_CONFIG"
                     / "VAR_GLOBAL" / "WHILE" / "END_WHILE" / "WITH"
                     / "PRIORITY" / "STRING" / "WSTRING"
    rule STANDARD_FUNCTION_BLOCK_NAME() = "END_VAR"

    // B.0
    pub rule library() -> Vec<LibraryElement> = traced(<library__impl()>)
    pub rule library__impl() -> Vec<LibraryElement> = _ decls:library_element_declaration() ** _ _ { decls.into_iter().flatten().collect() }
    // TODO This misses some types such as ladder diagrams
    rule library_element_declaration() -> Vec<LibraryElement> = data_types:data_type_declaration() {
      data_types.into_iter().map(LibraryElement::DataTypeDeclaration).collect()
    } / fbd:function_block_declaration() { vec![LibraryElement::FunctionBlockDeclaration(fbd)] } / fd:function_declaration() { vec![LibraryElement::FunctionDeclaration(fd)] } / pd:program_declaration() { vec![LibraryElement::ProgramDeclaration(pd)] } / cd:configuration_declaration() { vec![LibraryElement::ConfigurationDeclaration(cd)] }

    // B.1.1 Letters, digits and identifier
    //rule digit() -> &'input str = $(['0'..='9'])
    rule identifier() -> Id = start:position!() !KEYWORD() i:$(['a'..='z' | '0'..='9' | 'A'..='Z' | '_']+) end:position!() { Id::from(i).with_position(SourceLoc::range(start, end)) }

    // B.1.2 Constants
    rule constant() -> Constant = r:real_literal() { Constant::RealLiteral(r) }
        / i:integer_literal() { Constant::IntegerLiteral(i.try_into().unwrap()) }
        / c:character_string() { Constant::CharacterString() }
        / d:duration() { Constant::Duration(d) }
        / t:time_of_day() { Constant::TimeOfDay() }
        / d:date() { Constant::Date() }
        / dt:date_and_time() { Constant::DateAndTime() }
        / b:boolean_literal() { Constant::Boolean(b) }

    // B.1.2.1 Numeric literals
    // numeric_literal omitted and only in constant.
    // TODO fill out the rest here
    rule integer_literal() -> SignedInteger = bi:binary_integer() { bi.into() } / oi:octal_integer() { oi.into() } / hi:hex_integer() { hi.into() } / si:signed_integer() { si }
    rule signed_integer__string() -> &'input str = n:$(['+' | '-']?['0'..='9']("_"? ['0'..='9'])*) { n }
    rule signed_integer() -> SignedInteger = start:position!() n:signed_integer__string() { SignedInteger::new(n, SourceLoc::new(start)) }
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
    // TODO handle the sign
    rule integer__string() -> &'input str = n:$(['0'..='9']("_"? ['0'..='9'])*) { n }
    rule integer() -> Integer = start:position!() n:integer__string() { Integer::new(n, SourceLoc::new(start)) }
    rule binary_integer_prefix() -> () = "2#" ()
    rule binary_integer() -> Integer = start:position!() binary_integer_prefix() n:$(['0'..='1']("_"? ['0'..='1'])*) { Integer::binary(n, SourceLoc::new(start)) }
    rule octal_integer_prefix() -> () = "8#" ()
    rule octal_integer() -> Integer = start:position!() octal_integer_prefix() n:$(['0'..='7']("_"? ['0'..='7'])*) { Integer::octal(n, SourceLoc::new(start)) }
    rule hex_integer_prefix() -> () = "16#" ()
    rule hex_integer() -> Integer = start:position!() hex_integer_prefix() n:$(['0'..='9' | 'A'..='F']("_"? ['0'..='9' | 'A'..='F'])*) { Integer::hex(n, SourceLoc::new(start)) }
    rule boolean_literal() -> Boolean =
      // 1 and 0 can be a Boolean, but only with the prefix is it definitely a Boolean
      "BOOL#1" { Boolean::True }
      / "BOOL#0" {Boolean::False }
      / ("BOOL#")? "TRUE" { Boolean::True }
      / ("BOOL#")? "FALSE" { Boolean::False }
    // B.1.2.2 Character strings
    rule character_string() -> Vec<char> = s:single_byte_character_string() / d:double_byte_character_string()
    rule single_byte_character_string() -> Vec<char>  = "'" s:single_byte_character_representation()+ "'" { s }
    rule double_byte_character_string() -> Vec<char> = "\"" s:double_byte_character_representation()+ "\"" { s }
    // TODO escape characters
    rule single_byte_character_representation() -> char = common_character_representation()
    rule double_byte_character_representation() -> char = common_character_representation()
    // TODO other printable characters
    rule common_character_representation() -> char = c:['a'..='z' | 'A'..='Z'] { c }

    // B.1.2.3 Time literals
    // Omitted and subsumed into constant.

    // B.1.2.3.1 Duration
    pub rule duration() -> Duration = ("TIME" / "T") "#" s:("-")? i:interval() {
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
    rule time_of_day() -> Time = ("TOD" / "TIME_OF_DAY") "#" d:daytime() { d }
    rule daytime() -> Time = h:day_hour() ":" m:day_minute() ":" s:day_second() {
      // TODO error handling
      Time::from_hms(h.try_into().unwrap(), m.try_into().unwrap(), s.try_into().unwrap()).unwrap()
    }
    rule day_hour() -> Integer = i:integer() { i }
    rule day_minute() -> Integer = i:integer() { i }
    rule day_second() -> Integer = i:integer() { i }
    rule date() -> Date = ("DATE" / "D") "#" d:date_literal() { d }
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
    rule date_and_time() -> PrimitiveDateTime = ("DATE_AND_TIME" / "DT") "#" d:date_literal() "-" t:daytime() { PrimitiveDateTime::new(d, t) }

    // B.1.3 Data types
    rule non_generic_type_name() -> Id = et:elementary_type_name() { et.into() } / derived_type_name()

    // B.1.3.1 Elementary data types
    rule elementary_type_name() -> ElementaryTypeName = numeric_type_name() / date_type_name() / bit_string_type_name() / elementary_string_type_name()
    rule elementary_string_type_name() -> ElementaryTypeName = "STRING" { ElementaryTypeName::STRING } / "WSTRING" { ElementaryTypeName::WSTRING }
    rule numeric_type_name() -> ElementaryTypeName = integer_type_name() / real_type_name()
    rule integer_type_name() -> ElementaryTypeName = signed_integer_type_name() / unsigned_integer_type_name()
    rule signed_integer_type_name() -> ElementaryTypeName = "SINT" { ElementaryTypeName::SINT }  / "INT" { ElementaryTypeName::INT } / "DINT" { ElementaryTypeName::DINT } / "LINT" { ElementaryTypeName::LINT }
    rule unsigned_integer_type_name() -> ElementaryTypeName = "USINT" { ElementaryTypeName::USINT }  / "UINT" { ElementaryTypeName::UINT } / "UDINT" { ElementaryTypeName::UDINT } / "ULINT" { ElementaryTypeName::ULINT }
    rule real_type_name() -> ElementaryTypeName = "REAL" { ElementaryTypeName::REAL } / "LREAL" { ElementaryTypeName::LREAL }
    rule date_type_name() -> ElementaryTypeName = "DATE" { ElementaryTypeName::DATE } / "TIME_OF_DAY" { ElementaryTypeName::TimeOfDay } / "TOD" { ElementaryTypeName::TimeOfDay } / "DATE_AND_TIME" { ElementaryTypeName::DateAndTime } / "DT" { ElementaryTypeName::DateAndTime }
    rule bit_string_type_name() -> ElementaryTypeName = "BOOL" { ElementaryTypeName::BOOL } / "BYTE" { ElementaryTypeName::BYTE } / "WORD" { ElementaryTypeName::WORD } / "DWORD" { ElementaryTypeName::DWORD } / "LWORD" { ElementaryTypeName::LWORD }

    // B.1.3.3
    // TODO add all types
    rule derived_type_name() -> Id = single_element_type_name()
    // TODO add all options
    rule single_element_type_name() -> Id = simple_type_name()
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
      / subrange:subrange_type_declaration() { DataTypeDeclarationKind::Subrange(subrange) }
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
    rule subrange_type_declaration() -> SubrangeDeclaration = type_name:subrange_type_name() _ ":" _ spec:subrange_spec_init() {
      SubrangeDeclaration {
        type_name,
        spec,
      }
    }
    rule subrange_spec_init() -> SubrangeSpecification = spec:subrange_specification() _ default:(":=" _ def:signed_integer() { def })? {
      SubrangeSpecification {
        type_name: spec.0,
        subrange: spec.1,
        default,
      }
    }
    // TODO or add a subrange type name
    rule subrange_specification() -> (ElementaryTypeName, Subrange) = itn:integer_type_name() _ "(" _ sr:subrange() _ ")" { (itn, sr) }
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
      start:position!() "(" _ v:enumerated_value() ++ (_ "," _) _ ")" { EnumeratedSpecificationKind::values(v, SourceLoc::new(start)) }
    rule enumerated_specification() -> EnumeratedSpecificationKind  =
      start:position!() "(" _ v:enumerated_value() ++ (_ "," _) _ ")" { EnumeratedSpecificationKind::values(v, SourceLoc::new(start)) }
      / name:enumerated_type_name() { EnumeratedSpecificationKind::TypeName(name) }
    rule enumerated_value() -> EnumeratedValue = start:position!() type_name:(name:enumerated_type_name() "#" { name })? value:identifier() end:position!() { EnumeratedValue {type_name, value, position: Some(SourceLoc::range(start, end))} }
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
    rule array_specification() -> ArraySpecificationKind = "ARRAY" _ "[" _ ranges:subrange() ** (_ "," _ ) _ "]" _ "OF" _ type_name:non_generic_type_name() {
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
    rule structure_declaration() -> StructureDeclaration = "STRUCT" _ elements:semisep_oneplus(<structure_element_declaration()>) _ "END_STRUCT" {
      StructureDeclaration {
        // Requires a value but we don't know the name until level up
        type_name: Id::from(""),
        elements,
      }
    }
    rule structure_element_declaration() -> StructureElementDeclaration = name:structure_element_name() _ ":" _ init:(
      arr:array_spec_init() { InitialValueAssignmentKind::Array(arr) }
      / i:subrange_spec_init() { InitialValueAssignmentKind::Subrange(i) }
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
      / simple_or_enumerated_or_ambiguous_struct_spec_init()
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
    rule simple_or_enumerated_or_ambiguous_struct_spec_init() -> InitialValueAssignmentKind = s:simple_specification() _ ":=" _ c:constant() {
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
        values: values,
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
    rule string_type_declaration() -> StringDeclaration = type_name:string_type_name() _ ":" _ width:("STRING" { StringKind::String } / "WSTRING" { StringKind::WString }) _ "[" _ length:integer() _ "]" _ init:(":=" _ str:character_string() {str})? {
      StringDeclaration {
        type_name,
        length,
        width,
        init: init.map(|v| v.into_iter().collect()),
      }
    }

    // B.1.4 Variables
    rule variable() -> Variable = d:direct_variable() { Variable::AddressAssignment(d) } / symbolic_variable()
    // TODO add multi-element variable
    rule symbolic_variable() -> Variable = multi_element_variable() / name:variable_name() { Variable::SymbolicVariable(SymbolicVariable{name}) }
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
    rule location_prefix() -> LocationPrefix = l:['I' | 'Q' | 'M'] { LocationPrefix::from_char(l) }
    rule size_prefix() -> SizePrefix = s:['X' | 'B' | 'W' | 'D' | 'L'] { SizePrefix::from_char(s) }

    // B.1.4.2 Multi-element variables
    rule multi_element_variable() -> Variable = sv:structured_variable() {
      // TODO this is clearly wrong
      Variable::MultiElementVariable(vec![
        sv.0,
        sv.1,
      ])
    }
    //rule array_variable() -> () = subscripted_variable() _ subscript_list() {}
    //rule subscripted_variable() -> () = symbolic_variable()
    //rule subscript_list() -> () = "[" _ subscript()++ (_ "," _) _ "]" {}
    //rule subscript() -> () = expression() {}
    rule structured_variable() -> (Id, Id) = r:record_variable() "." f:field_selector() { (r, f)}
    // TODO this is most definitely wrong but it unblocks for now
    // very likely need to make this a repeated item with ++
    rule record_variable() -> Id = identifier()
    rule field_selector() -> Id = identifier()

    // B.1.4.3 Declarations and initialization
    pub rule input_declarations() -> Vec<VarDecl> = "VAR_INPUT" _ qualifier:("RETAIN" {DeclarationQualifier::Retain} / "NON_RETAIN" {DeclarationQualifier::NonRetain})? _ declarations:semisep(<input_declaration()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::Input, qualifier)
    }
    // TODO add edge declaration (as a separate item - a tuple)
    rule input_declaration() -> Vec<UntypedVarDecl> = var_init_decl()
    rule edge_declaration() -> () = var1_list() _ ":" _ "BOOL" _ ("R_EDGE" / "F_EDGE")? {}
    // TODO the problem is we match first, then
    // TODO missing multiple here
    // We have to first handle the special case of enumeration or fb_name without an initializer
    // because these share the same syntax. We only know the type after trying to resolve the
    // type name.
    rule var_init_decl() -> Vec<UntypedVarDecl> = structured_var_init_decl__without_ambiguous() / string_var_declaration() / array_var_init_decl() /  var1_init_decl__with_ambiguous_struct()

    // TODO add in subrange_spec_init(), enumerated_spec_init()

    rule var1_init_decl__with_ambiguous_struct() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ init:(a:simple_or_enumerated_or_ambiguous_struct_spec_init()) {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.into_iter().map(|name| {
        UntypedVarDecl {
          name,
          initializer: init.clone(),
          position: SourceLoc::new(start)
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
        // TODO
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::Structure(init_struct.clone()),
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule array_var_init_decl() -> Vec<UntypedVarDecl> = start:position!() names:var1_list() _ ":" _ array_spec_init() end:position!() {
      names.into_iter().map(|name| {
        // TODO
        UntypedVarDecl {
          name,
          initializer: InitialValueAssignmentKind::None,
          position: SourceLoc::range(start, end)
        }
      }).collect()
    }
    rule fb_name() -> Id = i:identifier() { i }
    pub rule output_declarations() -> Vec<VarDecl> = "VAR_OUTPUT" _ qualifier:("RETAIN" {DeclarationQualifier::Retain} / "NON_RETAIN" {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::Output, qualifier)
    }
    pub rule input_output_declarations() -> Vec<VarDecl> = "VAR_IN_OUT" _ qualifier:("RETAIN" {DeclarationQualifier::Retain} / "NON_RETAIN" {DeclarationQualifier::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::InOut,  qualifier)
    }
    rule var_declarations() -> VarDeclarations = "VAR" _ qualifier:"CONSTANT"? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      let qualifier = qualifier.map(|()| DeclarationQualifier::Constant);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule retentive_var_declarations() -> VarDeclarations = "VAR" _ "RETAIN" _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      let qualifier = Option::Some(DeclarationQualifier::Retain);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, qualifier))
    }
    rule located_var_declarations() -> VarDeclarations = "VAR" _ qualifier:("CONSTANT" { DeclarationQualifier::Constant } / "RETAIN" {DeclarationQualifier::Retain} / "NON_RETAIN" {DeclarationQualifier::NonRetain})? _ declarations:semisep(<located_var_decl()>) _ "END_VAR" {
      let qualifier = qualifier.or(Some(DeclarationQualifier::Unspecified));
      VarDeclarations::Located(VarDeclarations::map_located(declarations, qualifier))
    }
    rule located_var_decl() -> LocatedVarDecl = start:position!() name:variable_name()? _ location:location() _ ":" _ initializer:located_var_spec_init() {
      LocatedVarDecl {
        name,
        qualifier: DeclarationQualifier::Unspecified,
        location,
        initializer,
        position: SourceLoc::new(start),
      }
    }
    // TODO is this NOT the right type to return?
    // We use the same type as in other places for VarInit, but the external always omits the initializer
    rule external_var_declarations() -> VarDeclarations = "VAR_EXTERNAL" _ constant:"CONSTANT"? _ declarations:semisep(<external_declaration()>) _ "END_VAR" {
      let qualifier = constant.map(|()| DeclarationQualifier::Constant);
      VarDeclarations::External(VarDeclarations::map(declarations, qualifier))
    }
    // TODO subrange_specification, array_specification(), structure_type_name and others
    rule external_declaration_spec() -> InitialValueAssignmentKind = type_name:simple_specification() {
      InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: None,
      })
    }
    rule external_declaration() -> VarDecl = start:position!() name:global_var_name() _ ":" _ spec:external_declaration_spec() {
      VarDecl {
        name,
        var_type: VariableType::External,
        qualifier: DeclarationQualifier::Unspecified,
        initializer: spec,
        position: SourceLoc::new(start),
      }
    }
    rule global_var_name() -> Id = i:identifier() { i }

    rule qualifier() -> DeclarationQualifier = "CONSTANT" { DeclarationQualifier::Constant } / "RETAIN" { DeclarationQualifier::Retain }
    pub rule global_var_declarations() -> Vec<VarDecl> = "VAR_GLOBAL" _ qualifier:qualifier()? _ declarations:semisep(<global_var_decl()>) _ "END_VAR" {
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
    rule global_var_decl() -> (Vec<VarDecl>) = start:position!() vs:global_var_spec() _ ":" _ initializer:(l:located_var_spec_init() { l } / f:function_block_type_name() { InitialValueAssignmentKind::FunctionBlock(FunctionBlockInitialValueAssignment{type_name: f})})? {
      vs.0.into_iter().map(|name| {
        let init = initializer.clone().unwrap_or(InitialValueAssignmentKind::None);
        VarDecl {
          name,
          var_type: VariableType::Global,
          qualifier: DeclarationQualifier::Unspecified,
          // TODO this is clearly wrong
          initializer: init,
          // TODO this is clearly wrong
          position: SourceLoc::new(start),
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
    pub rule location() -> AddressAssignment = "AT" _ v:direct_variable() { v }
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
    rule single_byte_string_spec() -> StringInitializer = "STRING" _ length:("[" _ i:integer() _ "]" {i})? _ initial_value:(":=" _ v:single_byte_character_string() {v})? {
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
    rule double_byte_string_spec() -> StringInitializer = "WSTRING" _ length:("[" _ i:integer() _ "]" {i})? _ initial_value:(":=" _ v:double_byte_character_string() {v})? {
      StringInitializer {
        length,
        width: StringKind::WString,
        initial_value,
      }
    }

    // B.1.5.1 Functions
    rule function_name() -> Id = standard_function_name() / derived_function_name()
    // TODO this isn't correct
    rule standard_function_name() -> Id = identifier()
    rule derived_function_name() -> Id = identifier()
    rule function_declaration() -> FunctionDeclaration = "FUNCTION" _  name:derived_function_name() _ ":" _ rt:(et:elementary_type_name() { et.into() } / dt:derived_type_name() { dt }) _ var_decls:(io:io_var_declarations() / func:function_var_decls()) ** _ _ body:function_body() _ "END_FUNCTION" {
      let (variables, located) = VarDeclarations::unzip(var_decls);
      FunctionDeclaration {
        name,
        return_type: rt,
        variables,
        body,
      }
    }
    rule io_var_declarations() -> VarDeclarations = i:input_declarations() { VarDeclarations::Inputs(i) } / o:output_declarations() { VarDeclarations::Outputs(o) } / io:input_output_declarations() { VarDeclarations::Inouts(io) }
    rule function_var_decls() -> VarDeclarations = "VAR" _ qualifier:"CONSTANT"? _ vars:semisep_oneplus(<var2_init_decl()>) _ "END_VAR" {
      let qualifier = qualifier.map(|()| DeclarationQualifier::Constant);
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
    rule function_block_declaration() -> FunctionBlockDeclaration = "FUNCTION_BLOCK" _ name:derived_function_block_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other }) ** _ _ body:function_block_body() _ "END_FUNCTION_BLOCK" {
      let (variables, located) = VarDeclarations::unzip(decls);
      FunctionBlockDeclaration {
        name,
        variables,
        // TODO located?
        body,
      }
    }
    // TODO there are far more here
    rule other_var_declarations() -> VarDeclarations = external_var_declarations() / var_declarations()
    rule function_block_body() -> FunctionBlockBody = networks:sequential_function_chart() { FunctionBlockBody::sfc(networks) } / statements:statement_list() { FunctionBlockBody::stmts(statements) } / _ { FunctionBlockBody::empty( )}

    // B.1.5.3 Program declaration
    rule program_type_name() -> Id = i:identifier() { i }
    pub rule program_declaration() ->  ProgramDeclaration = "PROGRAM" _ p:program_type_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other } / located:located_var_declarations() { located }) ** _ _ body:function_block_body() _ "END_PROGRAM" {
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
    rule initial_step() -> Element = "INITIAL_STEP" _ name:step_name() _ ":" _ action_associations:action_association() ** (_ ";" _) "END_STEP" {
      Element::InitialStep(Step{
        name,
        action_associations,
       })
    }
    rule step() -> Element = "STEP" _ name:step_name() _ ":" _ action_associations:semisep(<action_association()>) _ "END_STEP" {
      Element::step(
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
    // TODO this is missing some
    rule action_qualifier() -> ActionQualifier = q:['N' | 'R' | 'S' | 'P'] { ActionQualifier::from_char(q) }
    rule indicator_name() -> Id = variable_name()
    rule transition() -> Element = "TRANSITION" _ name:transition_name()? _ priority:("(" _ "PRIORITY" _ ":=" _ p:integer() _ ")" {p})? _ "FROM" _ from:steps() _ "TO" _ to:steps() _ condition:transition_condition() _ "END_TRANSITION" {
      Element::Transition(Transition {
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
    rule action() -> Element = "ACTION" _ name:action_name() _ ":" _ body:function_block_body() _ "END_ACTION" {
      Element::Action(Action {
        name,
        body
      })
    }

    // B.1.7 Configuration elements
    rule configuration_name() -> Id = i:identifier() { i }
    rule resource_type_name() -> Id = i:identifier() { i }
    pub rule configuration_declaration() -> ConfigurationDeclaration = "CONFIGURATION" _ n:configuration_name() _ g:global_var_declarations()? _ r:resource_declaration() _ "END_CONFIGURATION" {
      let g = g.unwrap_or_default();
      // TODO this should really be multiple items
      let r = vec![r];
      ConfigurationDeclaration {
        name: n,
        global_var: g,
        resource_decl: r,
      }
    }
    rule resource_declaration() -> ResourceDeclaration = "RESOURCE" _ n:resource_name() _ "ON" _ t:resource_type_name() _ g:global_var_declarations()? _ resource:single_resource_declaration() _ "END_RESOURCE" {
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
    pub rule task_configuration() -> TaskConfiguration = "TASK" _ name:task_name() _ init:task_initialization() {
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
    rule task_initialization_interval() -> Duration = "INTERVAL" _ ":=" _ source:data_source() _ "," {
      // TODO The interval may not necessarily be a duration, but for now, only support Duration types
      match source {
        Constant::Duration(duration) => duration,
        _ => panic!("Only supporting Duration types for now"),
      }
     }
    rule task_initialization_priority() -> u32 = "PRIORITY" _ ":=" _ i:integer() { i.value.try_into().unwrap() }
    // TODO there are more here, but only supporting Constant for now
    pub rule data_source() -> Constant = constant:constant() { constant }
    // TODO more options here
    //pub rule data_source() -> &'input str =
    pub rule program_configuration() -> ProgramConfiguration = "PROGRAM" _ name:program_name() task_name:( _ "WITH" _ t:task_name() { t })? _ ":" _ pt:program_type_name() (_ "(" _ c:prog_conf_element() ** (_ "," _) _ ")")? {
      ProgramConfiguration {
        name,
        task_name,
        type_name: pt,
      }
     }
    rule prog_conf_element() -> Id = t:fb_task() { t.0 } /*/ p:prog_cnxn() { p }*/
    rule fb_task() -> (Id, Id) = n:fb_name() _ "WITH" _ tn:task_name() { (n, tn) }

    // B.3.1 Expressions
    rule expression() -> ExprKind = exprs:xor_expression() ++ (_ "OR" _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::Or, terms: exprs}
      }
      exprs[0].clone()
    }
    rule xor_expression() -> ExprKind = exprs:and_expression() ++ (_ "XOR" _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::Xor, terms: exprs}
      }
      exprs[0].clone()
    }
    rule and_expression() -> ExprKind = exprs:comparison() ++ (_ ("&" / "AND") _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::And, terms: exprs}
      }
      exprs[0].clone()
    }
    rule comparison() -> ExprKind = exprs:equ_expression() ++ (_ op:("=" {CompareOp::Eq} / "<>" {CompareOp::Ne}) _) {
      // TODO capture the operator type to distinguish
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::Compare {op: CompareOp::Eq, terms: exprs}
      }
      exprs[0].clone()
    }
    rule equ_expression() -> ExprKind = exprs:add_expression() ++ (_ comparison_operator() _) {// TODO capture the operator type to distinguish
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::Compare {op: CompareOp::Lt, terms: exprs}
      }
      exprs[0].clone()
    }
    rule comparison_operator() -> CompareOp = "<"  {CompareOp::Lt } / ">" {CompareOp::Gt} / "<=" {CompareOp::LtEq} / ">=" {CompareOp::GtEq}
    rule add_expression() -> ExprKind = exprs:term() ++ (_ add_operator() _ ) {
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::BinaryOp {ops: vec![Operator::Add], terms: exprs}
      }
      exprs[0].clone()
    }
    rule add_operator() -> Operator = "+" {Operator::Add} / "-" {Operator::Sub}
    rule term() -> ExprKind = exprs:power_expression() ++ (_ multiply_operator() _) {
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::BinaryOp {ops: vec![Operator::Mul], terms: exprs}
      }
      exprs[0].clone()
    }
    rule multiply_operator() -> Operator = "*" {Operator::Mul} / "/" {Operator::Div}/ "MOD" {Operator::Mod}
    rule power_expression() -> ExprKind = exprs:unary_expression() ++ (_ "**" _) {
      if exprs.len() > 1 {
        return ExprKind::BinaryOp {ops: vec![Operator::Pow], terms: exprs}
      }
      exprs[0].clone()
    }
    rule unary_expression() -> ExprKind = unary:unary_operator()? _ expr:primary_expression() {
      if let Some(op) = unary {
        return ExprKind::UnaryOp { op, term: Box::new(expr) };
      }
      expr
    }
    rule unary_operator() -> UnaryOp = "-" {UnaryOp::Neg} / "NOT" {UnaryOp::Not}
    rule primary_expression() -> ExprKind = constant:constant() {
      ExprKind::Const(constant)
    } / function:function_expression() {
      function
    } / variable:variable() {
      ExprKind::Variable(variable)
    }
    rule function_expression() -> ExprKind = name:function_name() _ "(" _ params:param_assignment() ** (_ "," _) _ ")" {
      ExprKind::Function {
        name,
        param_assignment: params
      }
    }

    // B.3.2 Statements
    pub rule statement_list() -> Vec<StmtKind> = statements:semisep(<statement()>) { statements }
    // TODO add other statement types
    rule statement() -> StmtKind = assignment:assignment_statement() { assignment }
      / selection:selection_statement() { selection } / fb:fb_invocation() { fb }

    // B.3.2.1 Assignment statements
    pub rule assignment_statement() -> StmtKind = var:variable() _ ":=" _ expr:expression() { StmtKind::assignment(var, expr) }

    // B.3.2.2 Subprogram control statements
    // TODO add RETURN
    rule subprogram_control_statement() -> StmtKind = fb:fb_invocation() { fb }
    rule fb_invocation() -> StmtKind = name:fb_name() _ "(" _ params:param_assignment() ** (_ "," _) _ ")" {
      StmtKind::FbCall(FbCall {
        var_name: name,
        params,
      })
    }
    // TODO this needs much more
    rule param_assignment() -> ParamAssignment = not:"NOT"? _ src:variable_name() _ "=>" _ tgt:variable() {
      ParamAssignment::Output (
        Output{
        not: false,
        src,
        tgt,
      })
    } / name:(n:variable_name() _ ":=" { n })? _ expr:expression() {
      match name {
        Some(n) => {
          ParamAssignment::NamedInput(NamedInput {name: n, expr} )
        },
        None => {
          ParamAssignment::positional(expr)
        }
      }
    }
    // B.3.2.3 Selection statement
    rule selection_statement() -> StmtKind = if_statement() / case_statement()
    rule if_statement() -> StmtKind = "IF" _ expr:expression() _ "THEN" _ body:statement_list()? _ else_ifs:("ELSIF" expr:expression() _ "THEN" _ body:statement_list() {(expr, body)}) ** _ _ else_body:("ELSE" _ e:statement_list() { e })? _ "END_IF" {
      StmtKind::If(If {
        expr,
        body: body.unwrap_or_default(),
        else_ifs,
        else_body: else_body.unwrap_or_default()
      })
    }
    rule case_statement() -> StmtKind = "CASE" _ selector:expression() _ "OF" _ cases:case_element() ** _ _ else_body:("ELSE" _ e:statement_list() { e })? _ "END_CASE" {
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
                position: SourceLoc::new(18),
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
                position: SourceLoc::new(39),
            },
        ];
        assert_eq!(plc_parser::input_declarations(decl), Ok(vars))
    }

    #[test]
    fn input_declarations_custom_type() {
        // TODO add a test
        // TODO enumerations - I think we need to be lazy here and make simple less strict
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
            position: SourceLoc::new(10),
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
                position: SourceLoc::new(19),
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
                position: SourceLoc::new(40),
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
            initializer: InitialValueAssignmentKind::simple("INT", Constant::IntegerLiteral(17)),
            position: SourceLoc::new(0),
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
            initial_step: Element::initial_step("Start", vec![]),
            elements: vec![
                Element::step(
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
                Element::action(
                    "RESETCOUNTER_INLINE1",
                    vec![StmtKind::assignment(
                        Variable::symbolic("Cnt"),
                        ExprKind::symbolic_variable("ResetCounterValue"),
                    )],
                ),
                Element::transition(
                    "ResetCounter",
                    "Start",
                    ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: ExprKind::boxed_symbolic_variable("Reset"),
                    },
                ),
                Element::transition(
                    "Start",
                    "Count",
                    ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: Box::new(ExprKind::symbolic_variable("Reset")),
                    },
                ),
                Element::step(
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
            Variable::symbolic("Cnt"),
            ExprKind::Const(Constant::IntegerLiteral(1)),
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := 1;"), expected)
    }

    #[test]
    fn statement_assign_add_const_operator() {
        let expected = Ok(vec![StmtKind::assignment(
            Variable::symbolic("Cnt"),
            ExprKind::BinaryOp {
                ops: vec![Operator::Add],
                terms: vec![
                    ExprKind::Const(Constant::IntegerLiteral(1)),
                    ExprKind::Const(Constant::IntegerLiteral(2)),
                ],
            },
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := 1 + 2;"), expected)
    }

    #[test]
    fn statement_assign_add_symbol_operator() {
        let expected = Ok(vec![StmtKind::assignment(
            Variable::symbolic("Cnt"),
            ExprKind::BinaryOp {
                ops: vec![Operator::Add],
                terms: vec![
                    ExprKind::symbolic_variable("Cnt"),
                    ExprKind::Const(Constant::IntegerLiteral(1)),
                ],
            },
        )]);
        assert_eq!(plc_parser::statement_list("Cnt := Cnt + 1;"), expected)
    }

    #[test]
    fn statement_if_multi_term() {
        let statement = "IF TRIG AND NOT TRIG THEN
      TRIG0:=TRIG;

    END_IF;";
        let expected = Ok(vec![StmtKind::if_then(
            ExprKind::Compare {
                op: CompareOp::And,
                terms: vec![
                    ExprKind::symbolic_variable("TRIG"),
                    ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: Box::new(ExprKind::symbolic_variable("TRIG")),
                    },
                ],
            },
            vec![StmtKind::assignment(
                Variable::symbolic("TRIG0"),
                ExprKind::symbolic_variable("TRIG"),
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
            ExprKind::symbolic_variable("Reset"),
            vec![StmtKind::assignment(
                Variable::symbolic("Cnt"),
                ExprKind::symbolic_variable("ResetCounterValue"),
            )],
            vec![StmtKind::assignment(
                Variable::symbolic("Cnt"),
                ExprKind::BinaryOp {
                    ops: vec![Operator::Add],
                    terms: vec![
                        ExprKind::symbolic_variable("Cnt"),
                        ExprKind::Const(Constant::IntegerLiteral(1)),
                    ],
                },
            )],
        )]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_without_name() {
        let statement = "CounterLD0(Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            var_name: Id::from("CounterLD0"),
            params: vec![ParamAssignment::positional(ExprKind::symbolic_variable(
                "Reset",
            ))],
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_with_name() {
        let statement = "CounterLD0(Cnt := Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            var_name: Id::from("CounterLD0"),
            params: vec![ParamAssignment::named(
                "Cnt",
                ExprKind::symbolic_variable("Reset"),
            )],
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn assignment() {
        let assign = "Cnt1 := CounterST0.OUT";
        let expected = Ok(StmtKind::assignment(
            Variable::symbolic("Cnt1"),
            ExprKind::Variable(Variable::MultiElementVariable(vec![
                Id::from("CounterST0"),
                Id::from("OUT"),
            ])),
        ));
        assert_eq!(plc_parser::assignment_statement(assign), expected)
    }
}
