extern crate peg;

use dsl::core::SourceLoc;
use peg::parser;

use crate::error::{Location, ParserDiagnostic};
use crate::mapper::*;
use ironplc_dsl::ast::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::dsl::*;
use ironplc_dsl::sfc::*;

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

/// Defines VarInitDecl type without the type information (e.g. input, output).
/// Useful only as an intermediate step in the parser where we do not know
/// the specific type.
struct UntypedVarInitDecl {
    pub name: Id,
    pub initializer: TypeInitializer,
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
    Inputs(Vec<VarInitDecl>),
    // output_declarations
    Outputs(Vec<VarInitDecl>),
    // input_output_declarations
    Inouts(Vec<VarInitDecl>),
    // located_var_declarations
    Located(Vec<LocatedVarInit>),
    // var_declarations
    Var(Vec<VarInitDecl>),
    // external_declarations
    External(Vec<VarInitDecl>),
    // TODO
    // Retentive(Vec<VarInitDecl>),
    // NonRetentive(Vec<VarInitDecl>),
    // Temp(Vec<VarInitDecl>),
}

struct InputOutputDeclarations {
    inputs: Vec<VarInitDecl>,
    outputs: Vec<VarInitDecl>,
    inouts: Vec<VarInitDecl>,
}

impl InputOutputDeclarations {
    fn new() -> Self {
        InputOutputDeclarations {
            inputs: vec![],
            outputs: vec![],
            inouts: vec![],
        }
    }
}
struct OtherDeclarations {
    externals: Vec<VarInitDecl>,
    vars: Vec<VarInitDecl>,
    //retentives: Vec<VarInitDecl>,
    //non_retentives: Vec<VarInitDecl>,
    //temps: Vec<VarInitDecl>,
    // TODO incompl_located_var_declarations
}

impl OtherDeclarations {
    fn new() -> Self {
        OtherDeclarations {
            externals: vec![],
            vars: vec![],
            //retentives: vec![],
            //non_retentives: vec![],
            //temps: vec![],
        }
    }
}
struct LocatedDeclarations {
    decl: Vec<LocatedVarInit>,
}

impl LocatedDeclarations {
    fn new() -> Self {
        LocatedDeclarations { decl: vec![] }
    }
}

impl VarDeclarations {
    // Given multiple sets of declarations, unzip them into types of
    // declarations.
    fn unzip(
        mut decls: Vec<VarDeclarations>,
    ) -> (
        InputOutputDeclarations,
        OtherDeclarations,
        LocatedDeclarations,
    ) {
        let mut io = InputOutputDeclarations::new();
        let mut other = OtherDeclarations::new();
        let mut located = LocatedDeclarations::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(mut i) => {
                    io.inputs.append(&mut i);
                }
                VarDeclarations::Outputs(mut o) => {
                    io.outputs.append(&mut o);
                }
                VarDeclarations::Inouts(mut inouts) => {
                    io.inouts.append(&mut inouts);
                }
                VarDeclarations::Located(mut l) => {
                    located.decl.append(&mut l);
                }
                VarDeclarations::Var(mut v) => {
                    other.vars.append(&mut v);
                }
                VarDeclarations::External(mut v) => {
                    other.externals.append(&mut v);
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

        return (io, other, located);
    }

    pub fn map(
        declarations: Vec<VarInitDecl>,
        storage_class: Option<StorageClass>,
    ) -> Vec<VarInitDecl> {
        declarations
            .into_iter()
            .map(|declaration| {
                let storage = storage_class
                    .clone()
                    .unwrap_or_else(|| StorageClass::Unspecified);
                let mut declaration = declaration.clone();
                declaration.storage_class = storage;
                declaration
            })
            .collect()
    }

    pub fn flat_map(
        declarations: Vec<Vec<UntypedVarInitDecl>>,
        var_type: VariableType,
        storage_class: Option<StorageClass>,
    ) -> Vec<VarInitDecl> {
        let declarations = declarations
            .into_iter()
            .flatten()
            .collect::<Vec<UntypedVarInitDecl>>();

        declarations
            .into_iter()
            .map(|declaration| {
                let storage = storage_class
                    .clone()
                    .unwrap_or_else(|| StorageClass::Unspecified);

                VarInitDecl {
                    name: declaration.name,
                    var_type: var_type.clone(),
                    storage_class: storage,
                    initializer: declaration.initializer,
                    position: declaration.position,
                }
            })
            .collect()
    }

    pub fn map_located(
        declarations: Vec<LocatedVarInit>,
        storage_class: Option<StorageClass>,
    ) -> Vec<LocatedVarInit> {
        declarations
            .into_iter()
            .map(|declaration| {
                let storage = storage_class
                    .clone()
                    .unwrap_or_else(|| StorageClass::Unspecified);
                let mut declaration = declaration.clone();
                declaration.storage_class = storage;
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

    // peg rules for making the grammar easier to work with
    rule semicolon() -> () = ";" ()
    rule _ = [' ' | '\n' | '\r' ]*
    // A semi-colon separated list with required ending separator
    rule semisep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ semicolon() _)) semicolon() {v}
    rule semisep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ semicolon() _)) semicolon() {v}

    rule KEYWORD() = "END_VAR" / "VAR" / "VAR_INPUT" / "IF" / "END_IF" / "FUNCTION_BLOCK" / "END_FUNCTION_BLOCK" / "AND" / "NOT" / "THEN" / "END_IF" / "STEP" / "END_STEP" / "FROM" / "PRIORITY" / "END_VAR"
    rule STANDARD_FUNCTION_BLOCK_NAME() = "END_VAR"

    // B.0
    pub rule library() -> Vec<LibraryElement> = traced(<library__impl()>)
    pub rule library__impl() -> Vec<LibraryElement> = _ libs:library_element_declaration() ** _ _ { libs }
    // TODO This misses some types such as ladder diagrams
    rule library_element_declaration() -> LibraryElement = dt:data_type_declaration() { LibraryElement::DataTypeDeclaration(dt) } / fbd:function_block_declaration() { LibraryElement::FunctionBlockDeclaration(fbd) } / fd:function_declaration() { LibraryElement::FunctionDeclaration(fd) } / pd:program_declaration() { LibraryElement::ProgramDeclaration(pd) } / cd:configuration_declaration() { LibraryElement::ConfigurationDeclaration(cd) }

    // B.1.1 Letters, digits and identifier
    //rule digit() -> &'input str = $(['0'..='9'])
    rule identifier() -> Id = !KEYWORD() i:$(['a'..='z' | '0'..='9' | 'A'..='Z' | '_']+) { Id::from(i) }

    // B.1.2 Constants
    rule constant() -> Constant = r:real_literal() { Constant::RealLiteral(r) } / i:integer_literal() { Constant::IntegerLiteral(i.try_from::<i128>()) } / c:character_string() { Constant::CharacterString() }  / d:duration() { Constant::Duration(d) } / t:time_of_day() { Constant::TimeOfDay() } / d:date() { Constant::Date() } / dt:date_and_time() { Constant::DateAndTime() }

    // B.1.2.1 Numeric literals
    // numeric_literal omitted and only in constant.
    // TODO fill out the rest here
    rule integer_literal() -> Integer = i:integer() { i }
    rule signed_integer() -> SignedInteger = n:$(['+' | '-']?['0'..='9']("_"? ['0'..='9'])*) { SignedInteger::from(n) }
    rule real_literal() -> Float = tn:(t:real_type_name() "#" {t})? whole:signed_integer() "." f:integer() e:exponent()? {
      let whole = whole.as_type::<f64>();
      let frac = f.as_type::<f64>();

      // To get the right size of the fraction part, determine how many digits
      // we have
      let num_chars = f.num_chars();
      // TODO this is not right
      let factor = 10;
      let factor: f64 = factor.into();
      let frac = frac / factor;

      // TODO need to add the exponent part
      println!("{} {}", whole, frac);

      Float {
        value: whole + frac,
        data_type: tn,
      }
    }
    rule exponent() -> (bool, Integer) = ("E" / "e") s:("+" / "-")? i:integer() { (true, i) }
    // TODO handle the sign
    rule integer() -> Integer = n:$(['0'..='9']("_"? ['0'..='9'])*) { Integer::from(n) }

    // B.1.2.2 Character strings
    rule character_string() -> Vec<char> = s:single_byte_character_string() / d:double_byte_character_string()
    rule single_byte_character_string() -> Vec<char>  = "'" s:single_byte_character_representation()+ "'" { s }
    rule double_byte_character_string() -> Vec<char> = "\"" s:double_byte_character_representation()+ "\"" { s }
    // TODO escape characters
    rule single_byte_character_representation() -> char = c:common_character_representation() { c }
    rule double_byte_character_representation() -> char = c:common_character_representation() { c }
    // TODO other printable characters
    rule common_character_representation() -> char = c:['a'..='z' | 'A'..='Z'] { c }

    // B.1.2.3 Time literals
    // Omitted and subsumed into constant.

    // B.1.2.3.1 Duration
    pub rule duration() -> Duration = ("TIME" / "T") "#" s:("-")? i:interval() {
      if let Some(sign) = s {
        return i * -1;
      }
      return i;
    }
    // milliseconds must come first because the "m" in "ms" would match the minutes rule
    rule interval() -> Duration = ms:milliseconds() { ms } / d:days() { d } / h:hours() { h } / m:minutes() { m } / s:seconds() { s }
    rule days() -> Duration = f:fixed_point() "d" { to_duration(f, 3600.0 * 24.0) } / i:integer() "d" "_"? h:hours() { h + to_duration(i.try_from::<f32>(), 3600.0 * 24.0) }

    rule fixed_point() -> f32 = i:integer() ("." integer())? {
      // TODO This drops the fraction, but I don't know how to keep it. May need one big regex in the worse case.
      i.try_from::<f32>()
    }
    rule hours() -> Duration = f:fixed_point() "h" { to_duration(f, 3600.0) } / i:integer() "h" "_"? m:minutes() { m + to_duration(i.try_from::<f32>(), 3600.0) }
    rule minutes() -> Duration = f:fixed_point() "m" { to_duration(f, 60.0) } / i:integer() "m" "_"? m:seconds() { m + to_duration(i.try_from::<f32>(), 60.0) }
    rule seconds() -> Duration = f:fixed_point() "s" { to_duration(f, 1.0) } / i:integer() "s" "_"? m:milliseconds() { m + to_duration(i.try_from::<f32>(), 1.0) }
    rule milliseconds() -> Duration = f:fixed_point() "ms" { to_duration(f, 0.001) }

    // 1.2.3.2 Time of day and date
    rule time_of_day() -> Time = ("TOD" / "TIME_OF_DAY") "#" d:daytime() { d }
    rule daytime() -> Time = h:day_hour() ":" m:day_minute() ":" s:day_second() {
      let h = h.try_from::<u8>();
      let m = m.try_from::<u8>();
      let s = s.try_from::<u8>();
      // TODO error handling
      Time::from_hms(h, m, s).unwrap()
    }
    rule day_hour() -> Integer = i:integer() { i }
    rule day_minute() -> Integer = i:integer() { i }
    rule day_second() -> Integer = i:integer() { i }
    rule date() -> Date = ("DATE" / "D") "#" d:date_literal() { d }
    rule date_literal() -> Date = y:year() "-" m:month() "-" d:day() {
      let y = y.try_from::<i32>();
      // TODO error handling
      let m = Month::try_from(m.try_from::<u8>()).unwrap();
      let d = d.try_from::<u8>();
      // TODO error handling
      Date::from_calendar_date(y, m, d).unwrap()
    }
    rule year() -> Integer = i:integer() { i }
    rule month() -> Integer = i:integer() { i }
    rule day() -> Integer = i:integer() { i }
    rule date_and_time() -> PrimitiveDateTime = ("DATE_AND_TIME" / "DT") "#" d:date_literal() "-" t:daytime() { PrimitiveDateTime::new(d, t) }

    // B.1.3.1 Elementary data types
    rule elementary_type_name() -> Id = "INT" { Id::from("INT")} / "BOOL" { Id::from("BOOL") } / "STRING" { Id::from("STRING") } / "REAL" { Id::from("REAL") }
    // TODO regex for REAL
    rule real_type_name() -> Id = t:$(("LREAL")) { Id::from(t) }

    // B.1.3.3
    // TODO add all types
    rule derived_type_name() -> Id = single_element_type_name()
    // TODO add all options
    rule single_element_type_name() -> Id = simple_type_name()
    rule simple_type_name() -> Id = identifier()
    rule enumerated_type_name() -> Id = identifier()
    rule data_type_declaration() -> Vec<EnumerationDeclaration> = "TYPE" _ declarations:semisep(<type_declaration()>) _ "END_TYPE" { declarations }
    // TODO this is missing multiple types
    rule type_declaration() -> EnumerationDeclaration = s:single_element_type_declaration() { s }
    // TODO this is missing multiple types
    rule single_element_type_declaration() -> EnumerationDeclaration = decl:enumerated_type_declaration() { decl }
    rule enumerated_type_declaration() -> EnumerationDeclaration = name:enumerated_type_name() _ ":" _ spec:enumerated_spec_init() {
      EnumerationDeclaration {
        name: name,
        spec: spec.0,
        default: spec.1,
      }
    }
    rule enumerated_spec_init__with_constant() -> TypeInitializer = spec:enumerated_specification() _ ":=" _ def:enumerated_value() {
      // TODO gut feeling says there is a defect here but I haven't looked into it
      match spec {
        EnumeratedSpecificationKind::TypeName(name) => {
          return TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
            type_name: name,
            initial_value: Some(def),
          });
        },
        EnumeratedSpecificationKind::Values(values) => {
          return TypeInitializer::EnumeratedValues {
            values: values,
            default: Some(def),
          };
        }
      }
     }
    rule enumerated_spec_init() -> (EnumeratedSpecificationKind, Option<Id>) = init:enumerated_specification() _ def:(":=" _ d:enumerated_value() { d })? {
      (init, def)
     }
    // TODO this doesn't support type name as a value
    rule enumerated_specification() -> EnumeratedSpecificationKind  = "(" _ v:enumerated_value() ++ (_ "," _) _ ")" {
      EnumeratedSpecificationKind::Values(v)
    }  / name:enumerated_type_name() {
      EnumeratedSpecificationKind::TypeName(name)
    }
    rule enumerated_value() -> Id = (enumerated_type_name() "#")? i:identifier() { i }
    // For simple types, they are inherently unambiguous because simple types are keywords (e.g. INT)
    rule simple_spec_init__with_constant() -> TypeInitializer = type_name:simple_specification() _ ":=" _ c:constant() {
      TypeInitializer::Simple {
        type_name: type_name,
        initial_value: Some(Initializer::Simple(c)),
      }
    }
    rule simple_spec_init() -> TypeInitializer = type_name:simple_specification() _ constant:(":=" _ c:constant() { c })? {
      TypeInitializer::Simple {
        type_name: type_name,
        initial_value: constant.map(|v| Initializer::Simple(v)),
      }
    }
    rule simple_specification() -> Id = elementary_type_name() / simple_type_name()

    // Union of simple_spec_init and enumerated_spec_init rules. In some cases, these both
    // reduce to identifier [':=' identifier] and are inherently ambiguous. To work around
    // this, combine this to check for the unambiguous cases first, later reducing to
    // the ambiguous case that we resolve later.
    //
    // There is still value in trying to disambiguate early because it allows us to use
    // the parser definitions.
    rule simple_or_enumerated_spec_init() -> TypeInitializer = s:simple_specification() _ ":=" _ c:constant() {
      // A simple_specification with a constant is unambiguous because the constant is
      // not a valid identifier.
      TypeInitializer::Simple {
        type_name: s,
        initial_value: Some(Initializer::Simple(c)),
      }
    } / spec:enumerated_specification() _ ":=" _ init:enumerated_value() {
      // An enumerated_specification defined with a value is unambiguous the value
      // is not a valid constant.
      match spec {
        EnumeratedSpecificationKind::TypeName(name) => {
          return TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
            type_name: name,
            initial_value: Some(init),
          });
        },
        EnumeratedSpecificationKind::Values(values) => {
          return TypeInitializer::EnumeratedValues {
            values: values,
            default: Some(init),
          };
        }
      }
    } / "(" _ values:enumerated_value() ** (_ "," _ ) _ ")" _  init:(":=" _ i:enumerated_value() {i})? {
      // An enumerated_specification defined by enum values is unambiguous because
      // the parenthesis are not valid simple_specification.
      TypeInitializer::EnumeratedValues {
        values: values,
        default: init
      }
    } / et:elementary_type_name() {
      // An identifier that is an elementary_type_name s unambiguous because these are
      // reserved keywords
      TypeInitializer::Simple {
        type_name: et,
        initial_value: None,
      }
    }/ i:identifier() {
      // What remains is ambiguous and the devolves to a single identifier because the prior
      // cases have captures all cases with a value.
      TypeInitializer::LateResolvedType(i)
    }

    // B.1.4 Variables
    rule variable() -> Variable = d:direct_variable() { Variable::DirectVariable(d) } / symbolic_variable()
    // TODO add multi-element variable
    rule symbolic_variable() -> Variable = multi_element_variable() / name:variable_name() { Variable::SymbolicVariable(SymbolicVariable{name: name}) }
    rule variable_name() -> Id = i:identifier() { i }

    // B.1.4.1 Directly represented variables
    pub rule direct_variable() -> DirectVariable = "%" l:location_prefix() s:size_prefix()? addr:integer() ++ "." {
      let size = s.unwrap_or_else(|| SizePrefix::Nil);
      let addr = addr.iter().map(|part|
        part.try_from::<u32>()
      ).collect();

      DirectVariable {
        location: l,
        size: size,
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
    pub rule input_declarations() -> Vec<VarInitDecl> = "VAR_INPUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<input_declaration()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::Input, storage)
    }
    // TODO add edge declaration (as a separate item - a tuple)
    rule input_declaration() -> Vec<UntypedVarInitDecl> = i:var_init_decl() { i }
    rule edge_declaration() -> () = var1_list() _ ":" _ "BOOL" _ ("R_EDGE" / "F_EDGE")? {}
    // TODO the problem is we match first, then
    // TODO missing multiple here
    // We have to first handle the special case of enumeration or fb_name without an initializer
    // because these share the same syntax. We only know the type after trying to resolve the
    // type name.
    rule var_init_decl() -> Vec<UntypedVarInitDecl> = var1_init_decl()

    // TODO add in subrange_spec_init(), enumerated_spec_init()

    rule var1_init_decl() -> Vec<UntypedVarInitDecl> = start:position!() names:var1_list() _ ":" _ init:(a:simple_or_enumerated_spec_init()) {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.into_iter().map(|name| {
        UntypedVarInitDecl {
          name: name,
          initializer: init.clone(),
          position: SourceLoc::new(start)
        }
      }).collect()
    }

    rule var1_list() -> Vec<Id> = names:variable_name() ++ (_ "," _) { names }
    rule fb_name() -> Id = i:identifier() { i }
    pub rule output_declarations() -> Vec<VarInitDecl> = "VAR_OUTPUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::Output, storage)
    }
    pub rule input_output_declarations() -> Vec<VarInitDecl> = "VAR_IN_OUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      VarDeclarations::flat_map(declarations, VariableType::InOut,  storage)
    }
    rule var_declarations() -> VarDeclarations = "VAR" _ storage:"CONSTANT"? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      let storage = storage.map(|()| StorageClass::Constant);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, storage))
    }
    rule retentive_var_declarations() -> VarDeclarations = "VAR" _ "RETAIN" _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      let storage = Option::Some(StorageClass::Retain);
      VarDeclarations::Var(VarDeclarations::flat_map(declarations, VariableType::Var, storage))
    }
    rule located_var_declarations() -> VarDeclarations = "VAR" _ storage:("CONSTANT" { StorageClass::Constant } / "RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<located_var_decl()>) _ "END_VAR" {
      let storage = storage.or(Some(StorageClass::Unspecified));
      VarDeclarations::Located(VarDeclarations::map_located(declarations, storage))
    }
    rule located_var_decl() -> LocatedVarInit = name:variable_name()? _ loc:location() _ ":" _ init:located_var_spec_init() {
      LocatedVarInit {
        name: name,
        storage_class: StorageClass::Unspecified,
        at: loc,
        initializer: init,
      }
    }
    // TODO is this NOT the right type to return?
    // We use the same type as in other places for VarInit, but the external always omits the initializer
    rule external_var_declarations() -> VarDeclarations = "VAR_EXTERNAL" _ constant:"CONSTANT"? _ declarations:semisep(<external_declaration()>) _ "END_VAR" {
      let storage = constant.map(|()| StorageClass::Constant);
      VarDeclarations::External(VarDeclarations::map(declarations, storage))
    }
    // TODO subrange_specification, array_specification(), structure_type_name and others
    rule external_declaration_spec() -> TypeInitializer = type_name:simple_specification() {
      TypeInitializer::Simple {
        type_name: type_name,
        initial_value: None,
      }
    }
    rule external_declaration() -> VarInitDecl = start:position!() name:global_var_name() _ ":" _ spec:external_declaration_spec() {
      VarInitDecl {
        name: name,
        var_type: VariableType::External,
        storage_class: StorageClass::Unspecified,
        initializer: spec,
        position: SourceLoc::new(start),
      }
    }
    rule global_var_name() -> Id = i:identifier() { i }

    rule storage_class() -> StorageClass = "CONSTANT" { StorageClass::Constant } / "RETAIN" { StorageClass::Retain }
    pub rule global_var_declarations() -> Vec<Declaration> = "VAR_GLOBAL" _ storage:storage_class()? _ declarations:semisep(<global_var_decl()>) _ "END_VAR" {
      // TODO set the options - this is pretty similar to VarInit - maybe it should be the same
      let declarations = declarations.into_iter().flatten().collect::<Vec<Declaration>>();
      declarations.into_iter().map(|declaration| {
        let storage = storage.clone().unwrap_or_else(|| StorageClass::Unspecified);
        let mut declaration = declaration.clone();
        declaration.storage_class = storage;
        declaration
      }).collect()
    }
    // TODO this doesn't pass all information. I suspect the rule from the dpec is not right
    rule global_var_decl() -> (Vec<Declaration>) = vs:global_var_spec() _ ":" _ initializer:(l:located_var_spec_init() { l } / f:function_block_type_name() { TypeInitializer::FunctionBlock(FunctionBlockTypeInitializer{type_name: f})})? {
      vs.0.into_iter().map(|name| {
        Declaration {
          name: name,
          storage_class: StorageClass::Unspecified,
          at: vs.1.clone(),
          initializer: initializer.clone(),
        }
      }).collect()
     }
    rule global_var_spec() -> (Vec<Id>, Option<At>) = names:global_var_list() {
      (names, None)
    } / global_var_name()? location() {
      // TODO this is clearly wrong, but it feel like the spec is wrong here
      (vec![Id::from("")], None)
    }
    // TODO this is completely fabricated - it isn't correct.
    rule located_var_spec_init() -> TypeInitializer = simple:simple_spec_init() { simple }
    // TODO
    pub rule location() -> DirectVariable = "AT" _ v:direct_variable() { v }
    rule global_var_list() -> Vec<Id> = names:global_var_name() ++ (_ "," _) { names }
    //rule string_var_declaration() -> stri

    // B.1.5.1 Functions
    rule function_name() -> Id = standard_function_name() / derived_function_name()
    // TODO this isn't correct
    rule standard_function_name() -> Id = identifier()
    rule derived_function_name() -> Id = identifier()
    rule function_declaration() -> FunctionDeclaration = "FUNCTION" _  name:derived_function_name() _ ":" _ rt:(elementary_type_name() / derived_type_name()) _ var_decls:(io:io_var_declarations() / func:function_var_decls()) ** _ _ body:function_body() _ "END_FUNCTION" {
      let (io, other, located) = VarDeclarations::unzip(var_decls);
      FunctionDeclaration {
        name: name,
        return_type: rt,
        inputs: io.inputs,
        outputs: io.outputs,
        inouts: io.inouts,
        // TODO
        vars: other.vars,
        externals: other.externals,
        body: body,
      }
    }
    rule io_var_declarations() -> VarDeclarations = i:input_declarations() { VarDeclarations::Inputs(i) } / o:output_declarations() { VarDeclarations::Outputs(o) } / io:input_output_declarations() { VarDeclarations::Inouts(io) }
    rule function_var_decls() -> VarDeclarations = "VAR" _ storage:"CONSTANT"? _ vars:semisep_oneplus(<var2_init_decl()>) _ "END_VAR" {
      let storage = storage.map(|()| StorageClass::Constant);
      VarDeclarations::Var(VarDeclarations::flat_map(vars, VariableType::Var, storage))
    }
    // TODO a bunch are missing here
    rule function_body() -> Vec<StmtKind> = statement_list()
    // TODO return types
    rule var2_init_decl() -> Vec<UntypedVarInitDecl> = var1_init_decl()

    // B.1.5.2 Function blocks
    // IEC 61131 defines separate standard and derived function block names,
    // but we don't need that distinction here.
    rule function_block_type_name() -> Id = i:identifier() { i }
    rule derived_function_block_name() -> Id = !STANDARD_FUNCTION_BLOCK_NAME() i:identifier() { i }
    // TODO add variable declarations
    rule function_block_declaration() -> FunctionBlockDeclaration = "FUNCTION_BLOCK" _ name:derived_function_block_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other }) ** _ _ body:function_block_body() _ "END_FUNCTION_BLOCK" {
      let (io, other, located) = VarDeclarations::unzip(decls);
      FunctionBlockDeclaration {
        name: name,
        inputs: io.inputs,
        outputs: io.outputs,
        inouts: io.inouts,
        // TODO
        vars: other.vars,
        externals: other.externals,
        body: body,
      }
    }
    // TODO there are far more here
    rule other_var_declarations() -> VarDeclarations = external_var_declarations() / var_declarations()
    rule function_block_body() -> FunctionBlockBody = networks:sequential_function_chart() { FunctionBlockBody::sfc(networks) } / statements:statement_list() { FunctionBlockBody::stmts(statements) } / _ { FunctionBlockBody::empty( )}

    // B.1.5.3 Program declaration
    rule program_type_name() -> Id = i:identifier() { i }
    pub rule program_declaration() ->  ProgramDeclaration = "PROGRAM" _ p:program_type_name() _ decls:(io:io_var_declarations() { io } / other:other_var_declarations() { other } / located:located_var_declarations() { located }) ** _ _ body:function_block_body() _ "END_PROGRAM" {
      let (io, other, located) = VarDeclarations::unzip(decls);
      ProgramDeclaration {
        type_name: p,
        inputs: io.inputs,
        outputs: io.outputs,
        inouts: io.inouts,
        vars: other.vars,
        // TODO more
        body: body,
      }
    }

    // B.1.6 Sequential function chart elements
    // TODO return something
    pub rule sequential_function_chart() -> Vec<Network> = networks:sfc_network() ++ _ { networks }
    // TOD add transition and action
    rule sfc_network() ->  Network = init:initial_step() _ elements:((s:step() {s } / a:action() {a} / t:transition() {t}) ** _) {
      Network {
        initial_step: init,
        elements: elements
      }
    }
    rule initial_step() -> Element = "INITIAL_STEP" _ name:step_name() _ ":" _ assoc:action_association() ** (_ ";" _) "END_STEP" {
      Element::InitialStep {
        name: name,
        action_associations: assoc,
      }
    }
    rule step() -> Element = "STEP" _ name:step_name() _ ":" _ assoc:semisep(<action_association()>) _ "END_STEP" {
      Element::Step {
        name: name,
        action_associations: assoc
      }
    }
    rule step_name() -> Id = identifier()
    // TODO this is missing stuff
    rule action_association() -> ActionAssociation = name:action_name() _ "(" _ qualifier:action_qualifier()? _ indicators:("," _ i:indicator_name() ** (_ "," _) { i })? _ ")" {
      ActionAssociation {
        name: name,
        qualifier: qualifier,
        indicators: indicators.unwrap_or_else(|| vec![]),
      }
    }
    rule action_name() -> Id = identifier()
    // TODO this is missing some
    rule action_qualifier() -> ActionQualifier = q:['N' | 'R' | 'S' | 'P'] { ActionQualifier::from_char(q) }
    rule indicator_name() -> Id = variable_name()
    rule transition() -> Element = "TRANSITION" _ name:transition_name()? _ priority:("(" _ "PRIORITY" _ ":=" _ p:integer() _ ")" {p})? _ "FROM" _ from:steps() _ "TO" _ to:steps() _ condition:transition_condition() _ "END_TRANSITION" {
      Element::Transition {
        name: name,
        priority: priority.map(|p| p.try_from::<u32>()),
        from: from,
        to: to,
        condition: condition,
      }
    }
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
      Element::Action {
        name: name,
        body: body
      }
    }

    // B.1.7 Configuration elements
    rule configuration_name() -> Id = i:identifier() { i }
    rule resource_type_name() -> Id = i:identifier() { i }
    pub rule configuration_declaration() -> ConfigurationDeclaration = "CONFIGURATION" _ n:configuration_name() _ g:global_var_declarations()? _ r:resource_declaration() _ "END_CONFIGURATION" {
      let g = g.unwrap_or_else(|| vec![]);
      // TODO this should really be multiple items
      let r = vec![r];
      ConfigurationDeclaration {
        name: n,
        global_var: g,
        resource_decl: r,
      }
    }
    rule resource_declaration() -> ResourceDeclaration = "RESOURCE" _ n:resource_name() _ "ON" _ t:resource_type_name() _ g:global_var_declarations()? _ resource:single_resource_declaration() _ "END_RESOURCE" {
      let g = g.unwrap_or_else(|| vec![]);
      ResourceDeclaration {
        name: n,
        resource: t,
        global_vars: g,
        tasks: resource.0,
        programs: resource.1,
      }
    }
    // TODO need to have more than one
    rule single_resource_declaration() -> (Vec<TaskConfiguration>, Vec<ProgramConfiguration>) = t:semisep(<task_configuration()>)? _ p:semisep_oneplus(<program_configuration()>) { (t.unwrap_or(vec![]), p) }
    rule resource_name() -> Id = i:identifier() { i }
    rule program_name() -> Id = i:identifier() { i }
    pub rule task_configuration() -> TaskConfiguration = "TASK" _ name:task_name() _ init:task_initialization() {
      TaskConfiguration {
        name: name,
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
    rule task_initialization_priority() -> u32 = "PRIORITY" _ ":=" _ i:integer() { i.try_from::<u32>() }
    // TODO there are more here, but only supporting Constant for now
    pub rule data_source() -> Constant = constant:constant() { constant }
    // TODO more options here
    //pub rule data_source() -> &'input str =
    pub rule program_configuration() -> ProgramConfiguration = "PROGRAM" _ name:program_name() task_name:( _ "WITH" _ t:task_name() { t })? _ ":" _ pt:program_type_name() (_ "(" _ c:prog_conf_element() ** (_ "," _) _ ")")? {
      ProgramConfiguration {
        name: name,
        task_name: task_name,
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
        return ExprKind::UnaryOp { op: op, term: Box::new(expr) };
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
        name: name,
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
        params: params,
      })
    }
    // TODO this needs much more
    rule param_assignment() -> ParamAssignment = name:(n:variable_name() _ ":=" { n })? _ expr:expression() {
      match name {
        Some(n) => {
          ParamAssignment::NamedInput(NamedInput {name: n, expr: expr} )
        },
        None => {
          ParamAssignment::positional(expr)
        }
      }
    } / not:"NOT"? _ src:variable_name() _ "=>" _ tgt:variable() {
      ParamAssignment::Output {
        // TODO map this optional
        not: false,
        src: src,
        tgt: tgt,
      }
    }
    // B.3.2.3 Selection statement
    // TODO add case statement
    rule selection_statement() -> StmtKind = ifstmt:if_statement() { ifstmt }
    // TODO handle else if
    rule if_statement() -> StmtKind = "IF" _ expr:expression() _ "THEN" _ body:statement_list()? _ else_body:("ELSE" _ e:statement_list() { e })? _ "END_IF" {
      StmtKind::If(If {
        expr: expr,
        body: body.unwrap_or_else(|| vec![]),
        else_body: else_body.unwrap_or_else(|| vec![])
      })
    }
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
            VarInitDecl {
                name: Id::from("TRIG"),
                var_type: VariableType::Input,
                storage_class: StorageClass::Unspecified,
                initializer: TypeInitializer::simple_uninitialized("BOOL"),
                position: SourceLoc::new(18),
            },
            VarInitDecl {
                name: Id::from("MSG"),
                var_type: VariableType::Input,
                storage_class: StorageClass::Unspecified,
                initializer: TypeInitializer::simple_uninitialized("STRING"),
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
        let expected = Ok(vec![VarInitDecl {
            name: Id::from("LEVEL"),
            var_type: VariableType::Input,
            storage_class: StorageClass::Unspecified,
            initializer: TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
                type_name: Id::from("LOGLEVEL"),
                initial_value: Some(Id::from("INFO")),
            }),
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
            VarInitDecl {
                name: Id::from("TRIG"),
                var_type: VariableType::Output,
                storage_class: StorageClass::Unspecified,
                initializer: TypeInitializer::simple_uninitialized("BOOL"),
                position: SourceLoc::new(19),
            },
            VarInitDecl {
                name: Id::from("MSG"),
                var_type: VariableType::Output,
                storage_class: StorageClass::Unspecified,
                initializer: TypeInitializer::simple_uninitialized("STRING"),
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
        let var = DirectVariable {
            location: LocationPrefix::I,
            size: SizePrefix::X,
            address: address,
        };
        assert_eq!(plc_parser::direct_variable("%IX1"), Ok(var))
    }

    #[test]
    fn location() {
        let address = vec![1];
        let var = DirectVariable {
            location: LocationPrefix::I,
            size: SizePrefix::X,
            address: address,
        };
        assert_eq!(plc_parser::location("AT %IX1"), Ok(var))
    }

    #[test]
    fn var_global() {
        // TODO assign the right values
        let reset = vec![Declaration {
            name: Id::from("ResetCounterValue"),
            storage_class: StorageClass::Constant,
            at: None,
            initializer: Option::Some(TypeInitializer::Simple {
                type_name: Id::from("INT"),
                initial_value: Option::Some(Initializer::Simple(Constant::IntegerLiteral(17))),
            }),
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
            initial_step: Element::InitialStep {
                name: Id::from("Start"),
                action_associations: vec![],
            },
            elements: vec![
                Element::Step {
                    name: Id::from("ResetCounter"),
                    action_associations: vec![
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
                },
                Element::Action {
                    name: Id::from("RESETCOUNTER_INLINE1"),
                    body: FunctionBlockBody::stmts(vec![StmtKind::assignment(
                        Variable::symbolic("Cnt"),
                        ExprKind::symbolic_variable("ResetCounterValue"),
                    )]),
                },
                Element::Transition {
                    name: None,
                    priority: None,
                    from: vec![Id::from("ResetCounter")],
                    to: vec![Id::from("Start")],
                    condition: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: ExprKind::boxed_symbolic_variable("Reset"),
                    },
                },
                Element::Transition {
                    name: None,
                    priority: None,
                    from: vec![Id::from("Start")],
                    to: vec![Id::from("Count")],
                    condition: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: Box::new(ExprKind::symbolic_variable("Reset")),
                    },
                },
                Element::Step {
                    name: Id::from("Count"),
                    action_associations: vec![
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
                },
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
