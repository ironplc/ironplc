extern crate peg;

use peg::parser;

use crate::mapper::*;
use ironplc_dsl::ast::*;
use ironplc_dsl::dsl::*;
use ironplc_dsl::sfc::*;

// Don't use std::time::Duration because it does not allow negative values.
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

pub fn parse_library(source: &str) -> Result<Vec<LibraryElement>, String> {
    plc_parser::library(source).map_err(|e| String::from(e.to_string()))
}

parser! {
  grammar plc_parser() for str {

    // peg rules for making the grammar easier to work with
    rule semicolon() -> () = ";" ()
    rule _ = [' ' | '\n' | '\r' ]*
    // A semi-colon separated list with required ending separator
    rule semisep<T>(x: rule<T>) -> Vec<T> = v:(x() ** (_ semicolon() _)) semicolon() {v}
    rule semisep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ semicolon() _)) semicolon() {v}

    rule KEYWORD() = "END_VAR" / "VAR" / "VAR_INPUT" / "IF" / "END_IF" / "FUNCTION_BLOCK" / "AND" / "NOT" / "THEN" / "END_IF" / "STEP" / "END_STEP" / "FROM" / "PRIORITY" / "END_VAR"
    rule STANDARD_FUNCTION_BLOCK_NAME() = "END_VAR"

    // B.0
    pub rule library() -> Vec<LibraryElement> = _ libs:library_element_declaration() ** _ _ { libs }
    // TODO This misses some types such as ladder diagrams
    rule library_element_declaration() -> LibraryElement = dt:data_type_declaration() { LibraryElement::DataTypeDeclaration(dt) } / fd:function_declaration() { LibraryElement::FunctionDeclaration(fd) } / fbd:function_block_declaration() { LibraryElement::FunctionBlockDeclaration(fbd) } / pd:program_declaration() { LibraryElement::ProgramDeclaration(pd) } / cd:configuration_declaration() { LibraryElement::ConfigurationDeclaration(cd) }

    // B.1.1 Letters, digits and identifier
    //rule digit() -> &'input str = $(['0'..='9'])
    rule identifier() -> &'input str = !KEYWORD() i:$(['a'..='z' | '0'..='9' | 'A'..='Z' | '_']+) { i }

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
        data_type: tn.map(|v| String::from(v)),
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
    rule elementary_type_name() -> &'input str = "INT" { "INT"} / "BOOL" { "BOOL" } / "STRING" { "STRING" } / "REAL" { "REAL" }
    // TODO regex for REAL
    rule real_type_name() -> &'input str = t:$(("LREAL")) { t }

    // B.1.3.3
    // TODO add all types
    rule derived_type_name() -> &'input str = single_element_type_name()
    // TODO add all options
    rule single_element_type_name() -> &'input str = simple_type_name()
    rule simple_type_name() -> &'input str = identifier()
    rule enumerated_type_name() -> &'input str = identifier()
    rule data_type_declaration() -> Vec<EnumerationDeclaration> = "TYPE" _ declarations:semisep(<type_declaration()>) _ "END_TYPE" { declarations }
    // TODO this is missing multiple types
    rule type_declaration() -> EnumerationDeclaration = s:single_element_type_declaration() { s }
    // TODO this is missing multiple types
    rule single_element_type_declaration() -> EnumerationDeclaration = decl:enumerated_type_declaration() { decl }
    rule enumerated_type_declaration() -> EnumerationDeclaration = name:enumerated_type_name() _ ":" _ def:enumerated_spec_init() {
      EnumerationDeclaration {
        name: String::from(name),
        initializer: def,
      }
    }
    rule enumerated_spec_init__unambiguous() -> TypeInitializer = init:enumerated_specification() _ ":=" _ def:enumerated_value() {
      match init {
        TypeInitializer::EnumeratedValues{values, default} => {
          return TypeInitializer::EnumeratedValues {
            values: values,
            default: Some(String::from(def)),
          };
        },
        TypeInitializer::EnumeratedType{type_name, initial_value} => {
          return TypeInitializer::EnumeratedType {
            type_name: type_name,
            initial_value: Some(String::from(def)),
          };
        }
        _ => panic!("Invalid type")
      }
     }
    rule enumerated_spec_init() -> TypeInitializer = init:enumerated_specification() _ def:(":=" _ d:enumerated_value() { d })? {
      match init {
        TypeInitializer::EnumeratedValues{values, default} => {
          return TypeInitializer::EnumeratedValues {
            values: values,
            default: def.map(|d| String::from(d))
          };
        },
        TypeInitializer::EnumeratedType{type_name, initial_value} => {
          return TypeInitializer::EnumeratedType {
            type_name: type_name,
            initial_value: def.map(|d| String::from(d)),
          };
        }
        _ => panic!("Invalid type")
      }
     }
    // TODO this doesn't support type name as a value
    rule enumerated_specification() -> TypeInitializer  = "(" _ v:enumerated_value() ++ (_ "," _) _ ")" {
      TypeInitializer::EnumeratedValues {
      values: v.iter().map(|v| String::from(*v)).collect(),
      default: None,
    }}  / name:enumerated_type_name() {
      TypeInitializer::EnumeratedType {
        type_name: String::from(name),
        initial_value: None,
      }
    }
    rule enumerated_value() -> &'input str = (enumerated_type_name() "#")? i:identifier() { i }
    // For simple types, they are inherently unambiguous because simple types are keywords (e.g. INT)
    rule simple_spec_init__unambiguous() -> TypeInitializer = simple_spec_init()
    rule simple_spec_init() -> TypeInitializer = type_name:simple_specification() _ constant:(":=" _ c:constant() { c })? {
      TypeInitializer::Simple {
        type_name: String::from(type_name),
        initial_value: constant.map(|v| Initializer::Simple(v)),
      }
    }
    rule simple_specification() -> &'input str = elementary_type_name()

    // B.1.4 Variables
    rule variable() -> Variable = d:direct_variable() { Variable::DirectVariable(d) } / symbolic_variable()
    // TODO add multi-element variable
    rule symbolic_variable() -> Variable = multi_element_variable() / name:variable_name() { Variable::symbolic(name) }
    rule variable_name() -> &'input str = i:identifier() { i }

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
        String::from(sv.0),
        String::from(sv.1),
      ])
    }
    //rule array_variable() -> () = subscripted_variable() _ subscript_list() {}
    //rule subscripted_variable() -> () = symbolic_variable()
    //rule subscript_list() -> () = "[" _ subscript()++ (_ "," _) _ "]" {}
    //rule subscript() -> () = expression() {}
    rule structured_variable() -> (&'input str, &'input str) = r:record_variable() "." f:field_selector() { (r, f)}
    // TODO this is most definitely wrong but it unblocks for now
    // very likely need to make this a repeated item with ++
    rule record_variable() -> &'input str = identifier()
    rule field_selector() -> &'input str = identifier()

    // B.1.4.3 Declarations and initialization
    pub rule input_declarations() -> Vec<VarInitDecl> = "VAR_INPUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<input_declaration()>) _ "END_VAR" {
      var_init_flat_map(declarations, storage)
    }
    // TODO add edge declaration (as a separate item - a tuple)
    rule input_declaration() -> Vec<VarInitDecl> = i:var_init_decl() { i }
    rule edge_declaration() -> () = var1_list() _ ":" _ "BOOL" _ ("R_EDGE" / "F_EDGE")? {}
    // TODO the problem is we match first, then
    // TODO missing multiple here
    // We have to first handle the special case of enumeration or fb_name without an initializer
    // because these share the same syntax. We only know the type after trying to resolve the
    // type name.
    rule var_init_decl() -> Vec<VarInitDecl> = i:var1_init_decl__unambiguous() { i } / i:late_bound_type_init() { i }

    // The initialize for some types looks the same if there is not initialization component. We use this to
    // late bind and later resolve the type.
    rule late_bound_type_init() -> Vec<VarInitDecl> = names:identifier() ** (_ "," _ ) _ ":" _ type_name:identifier() {
      names.iter().map(|name| {
        VarInitDecl {
          name: String::from(*name),
          storage_class: StorageClass::Unspecified,
          initializer: Option::Some(TypeInitializer::LateResolvedType(String::from(type_name))),
        }
      }).collect()
    }
    // TODO add in subrange_spec_init(), enumerated_spec_init()

    rule var1_init_decl__unambiguous() -> Vec<VarInitDecl> = names:var1_list() _ ":" _ init:(s:simple_spec_init__unambiguous() { s } / e:enumerated_spec_init__unambiguous() { e }) {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.iter().map(|name| {
        VarInitDecl {
          name: String::from(*name),
          storage_class: StorageClass::Unspecified,
          initializer: Option::Some(init.clone()),
        }
      }).collect()
    }
    rule var1_init_decl() -> Vec<VarInitDecl> = names:var1_list() _ ":" _ init:(s:simple_spec_init() { s } / e:enumerated_spec_init() { e }) {
      // Each of the names variables has is initialized in the same way. Here we flatten initialization
      names.iter().map(|name| {
        VarInitDecl {
          name: String::from(*name),
          storage_class: StorageClass::Unspecified,
          initializer: Option::Some(init.clone()),
        }
      }).collect()
    }
    rule var1_list() -> Vec<&'input str> = names:variable_name() ++ (_ "," _) { names }
    rule fb_name() -> &'input str = i:identifier() { i }
    pub rule output_declarations() -> Vec<VarInitDecl> = "VAR_OUTPUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      var_init_flat_map(declarations, storage)
    }
    pub rule input_output_declarations() -> Vec<VarInitDecl> = "VAR_IN_OUT" _ storage:("RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      var_init_flat_map(declarations, storage)
    }
    rule var_declarations() -> Vec<VarInitDecl> = "VAR" _ storage:"CONSTANT"? _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      let storage = storage.map(|()| StorageClass::Constant);
      var_init_flat_map(declarations, storage)
    }
    rule retentive_var_declarations() -> Vec<VarInitDecl> = "VAR" _ "RETAIN" _ declarations:semisep(<var_init_decl()>) _ "END_VAR" {
      var_init_flat_map(declarations, Option::Some(StorageClass::Retain))
    }
    rule located_var_declarations() -> Vec<LocatedVarInit> = "VAR" _ storage:("CONSTANT" { StorageClass::Constant }/ "RETAIN" {StorageClass::Retain} / "NON_RETAIN" {StorageClass::NonRetain})? _ declarations:semisep(<located_var_decl()>) _ "END_VAR" {
      declarations
          .into_iter()
          .map(|declaration| {
              let storage = storage
                  .clone()
                  .unwrap_or_else(|| StorageClass::Unspecified);
              let mut declaration = declaration.clone();
              declaration.storage_class = storage;
              declaration
          })
          .collect()
    }
    rule located_var_decl() -> LocatedVarInit = name:variable_name()? _ loc:location() _ ":" _ init:located_var_spec_init() {
      let name = name.map(|n| String::from(n));
      LocatedVarInit {
        name: name,
        storage_class: StorageClass::Unspecified,
        at: loc,
        initializer: init,
      }
    }
    // TODO is this NOT the right type to return?
    // We use the same type as in other places for VarInit, but the external always omits the initializer
    rule external_var_declarations() -> Vec<VarInitDecl> = "VAR_EXTERNAL" _ constant:"CONSTANT"? _ declarations:semisep(<external_declaration()>) _ "END_VAR" {
      let storage = constant.map(|()| StorageClass::Constant);
      var_init_map(declarations, storage)
    }
    // TODO subrange_specification, array_specification(), structure_type_name and others
    rule external_declaration_spec() -> TypeInitializer = type_name:simple_specification() {
      TypeInitializer::Simple {
        type_name: String::from(type_name),
        initial_value: None,
      }
    }
    rule external_declaration() -> VarInitDecl = name:global_var_name() _ ":" _ spec:external_declaration_spec() {
      VarInitDecl {
        name: String::from(name),
        storage_class: StorageClass::Unspecified,
        initializer: Option::Some(spec),
      }
    }
    rule global_var_name() -> &'input str = i:identifier() { i }

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
    rule global_var_decl() -> (Vec<Declaration>) = vs:global_var_spec() _ ":" _ initializer:(l:located_var_spec_init() { l } / f:function_block_type_name() { TypeInitializer::FunctionBlock { type_name: String::from(f) } })? {
      vs.0.iter().map(|name| {
        Declaration {
          name: String::from(*name),
          storage_class: StorageClass::Unspecified,
          at: vs.1.clone(),
          initializer: initializer.clone(),
        }
      }).collect()
     }
    rule global_var_spec() -> (Vec<& 'input str>, Option<At>) = names:global_var_list() {
      (names, None)
    } / global_var_name()? location() {
      // TODO this is clearly wrong, but it feel like the spec is wrong here
      (vec![""], None)
    }
    // TODO this is completely fabricated - it isn't correct.
    rule located_var_spec_init() -> TypeInitializer = simple:simple_spec_init() { simple }
    // TODO
    pub rule location() -> DirectVariable = "AT" _ v:direct_variable() { v }
    rule global_var_list() -> Vec<&'input str> = names:global_var_name() ++ (_ "," _) { names }
    //rule string_var_declaration() -> stri

    // B.1.5.1 Functions
    rule function_name() -> &'input str = standard_function_name() / derived_function_name()
    // TODO this isn't correct
    rule standard_function_name() -> &'input str = identifier()
    rule derived_function_name() -> &'input str = identifier()
    rule function_declaration() -> FunctionDeclaration = "FUNCTION" _  name:derived_function_name() _ ":" _ rt:(elementary_type_name() / derived_type_name()) _ var_decl:(io:io_var_declarations() / func:function_var_decls()) ** _ _ body:function_body() _ "END_FUNCTION" {
      let declarations = var_decl.into_iter().flatten().collect::<Vec<VarInitDecl>>();
      FunctionDeclaration {
        name: String::from(name),
        return_type: String::from(rt),
        // TODO separate into different types
        var_decls: declarations,
        body: body,
      }
    }
    rule io_var_declarations() -> Vec<VarInitDecl> = input_declarations() / output_declarations() / input_output_declarations()
    rule function_var_decls() -> Vec<VarInitDecl> = "VAR" _ storage:"CONSTANT"? _ vars:semisep_oneplus(<var2_init_decl()>) _ "END_VAR" {
      let storage = storage.map(|()| StorageClass::Constant);
      var_init_flat_map(vars, storage)
    }
    // TODO a bunch are missing here
    rule function_body() -> Vec<StmtKind> = statement_list()
    // TODO return types
    rule var2_init_decl() -> Vec<VarInitDecl> = var1_init_decl()

    // B.1.5.2 Function blocks
    // IEC 61131 defines separate standard and derived function block names,
    // but we don't need that distinction here.
    rule function_block_type_name() -> &'input str = i:identifier() { i }
    rule derived_function_block_name() -> &'input str = !STANDARD_FUNCTION_BLOCK_NAME() i:identifier() { i }
    // TODO add variable declarations
    rule function_block_declaration() -> FunctionBlockDeclaration = "FUNCTION_BLOCK" _ name:derived_function_block_name() _ decls:(io:io_var_declarations() { var_init_kind_map(io) } / other:other_var_declarations() { var_init_kind_map(other) }) ** _ _ body:function_block_body() _ "END_FUNCTION_BLOCK" {
      let declarations = decls.into_iter().flatten().collect::<Vec<VarInitKind>>();
      FunctionBlockDeclaration {
        name: String::from(name),
        var_decls: declarations,
        body: body,
      }
    }
    // TODO there are far more here
    rule other_var_declarations() -> Vec<VarInitDecl> = external_var_declarations() / var_declarations()
    rule function_block_body() -> FunctionBlockBody = networks:sequential_function_chart() { FunctionBlockBody::sfc(networks) } / statements:statement_list() { FunctionBlockBody::stmts(statements) }

    // B.1.5.3 Program declaration
    rule program_type_name() -> &'input str = i:identifier() { i }
    pub rule program_declaration() ->  ProgramDeclaration = "PROGRAM" _ p:program_type_name() _ decls:(io:io_var_declarations() { var_init_kind_map(io) } / other:other_var_declarations() { var_init_kind_map(other) } / located:located_var_declarations() { located_var_init_kind_map(located) }) ** _ _ body:function_block_body() _ "END_PROGRAM" {
      let declarations = decls.into_iter().flatten().collect::<Vec<VarInitKind>>();
      ProgramDeclaration {
        type_name: String::from(p),
        var_declarations: declarations,
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
        name: String::from(name),
        action_associations: assoc,
      }
    }
    rule step() -> Element = "STEP" _ name:step_name() _ ":" _ assoc:semisep(<action_association()>) _ "END_STEP" {
      Element::Step {
        name: String::from(name),
        action_associations: assoc
      }
    }
    rule step_name() -> &'input str = identifier()
    // TODO this is missing stuff
    rule action_association() -> ActionAssociation = name:action_name() _ "(" _ qualifier:action_qualifier()? _ indicators:("," _ i:indicator_name() ** (_ "," _) { i })? _ ")" {
      ActionAssociation {
        name: String::from(name),
        qualifier: qualifier,
        indicators: indicators.map(|ind| to_strings(ind)).unwrap_or_else(|| vec![]),
      }
    }
    rule action_name() -> &'input str = identifier()
    // TODO this is missing some
    rule action_qualifier() -> ActionQualifier = q:['N' | 'R' | 'S' | 'P'] { ActionQualifier::from_char(q) }
    rule indicator_name() -> &'input str = variable_name()
    rule transition() -> Element = "TRANSITION" _ name:transition_name()? _ priority:("(" _ "PRIORITY" _ ":=" _ p:integer() _ ")" {p})? _ "FROM" _ from:steps() _ "TO" _ to:steps() _ condition:transition_condition() _ "END_TRANSITION" {
      Element::Transition {
        name: name.map(|n| String::from(n)),
        priority: priority.map(|p| p.try_from::<u32>()),
        from: to_strings(from),
        to: to_strings(to),
        condition: condition,
      }
    }
    rule transition_name() -> &'input str = identifier()
    rule steps() -> Vec<&'input str> = name:step_name() {
      vec![name]
    } / "(" _ n1:step_name() _ "," _ n2:step_name() _ nr:("," _ n:step_name()) ** _ _ ")" {
      // TODO need to extend with nr
      vec![n1, n2]
    }
    // TODO add simple_instruction_list , fbd_network, rung
    rule transition_condition() -> ExprKind =  ":=" _ expr:expression() _ ";" { expr }
    rule action() -> Element = "ACTION" _ name:action_name() _ ":" _ body:function_block_body() _ "END_ACTION" {
      Element::Action {
        name: String::from(name),
        body: body
      }
    }

    // B.1.7 Configuration elements
    rule configuration_name() -> &'input str = i:identifier() { i }
    rule resource_type_name() -> &'input str = i:identifier() { i }
    pub rule configuration_declaration() -> ConfigurationDeclaration = "CONFIGURATION" _ n:configuration_name() _ g:global_var_declarations()? _ r:resource_declaration() _ "END_CONFIGURATION" {
      let g = g.unwrap_or_else(|| vec![]);
      // TODO this should really be multiple items
      let r = vec![r];
      ConfigurationDeclaration {
        name: String::from(n),
        global_var: g,
        resource_decl: r,
      }
    }
    rule resource_declaration() -> ResourceDeclaration = "RESOURCE" _ n:resource_name() _ "ON" _ t:resource_type_name() _ g:global_var_declarations()? _ resource:single_resource_declaration() _ "END_RESOURCE" {
      ResourceDeclaration {
        name: String::from(n),
        tasks: resource.0,
        programs: resource.1,
      }
    }
    // TODO need to have more than one
    rule single_resource_declaration() -> (Vec<TaskConfiguration>, Vec<ProgramConfiguration>) = t:semisep(<task_configuration()>) _ p:semisep_oneplus(<program_configuration()>) { (t, p) }
    rule resource_name() -> &'input str = i:identifier() { i }
    rule program_name() -> &'input str = i:identifier() { i }
    pub rule task_configuration() -> TaskConfiguration = "TASK" _ name:task_name() _ init:task_initialization() {
      TaskConfiguration {
        name: String::from(name),
        priority: init.0,
        // TODO This needs to set the interval
        interval: init.1,
      }
    }
    rule task_name() -> &'input str = i:identifier() { i }
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
        name: String::from(name),
        task_name: task_name.map(|n| String::from(n)),
        type_name: String::from(pt),
      }
     }
    rule prog_conf_element() -> &'input str = t:fb_task() { t.0 } /*/ p:prog_cnxn() { p }*/
    rule fb_task() -> (&'input str, &'input str) = n:fb_name() _ "WITH" _ tn:task_name() { (n, tn) }

    // B.3.1 Expressions
    rule expression() -> ExprKind = exprs:xor_expression() ** (_ "OR" _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::Or, terms: exprs}
      }
      exprs[0].clone()
    }
    rule xor_expression() -> ExprKind = exprs:and_expression() ** (_ "XOR" _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::Xor, terms: exprs}
      }
      exprs[0].clone()
    }
    rule and_expression() -> ExprKind = exprs:comparison() ** (_ ("&" / "AND") _) {
      if exprs.len() > 1 {
        return ExprKind::Compare {op: CompareOp::And, terms: exprs}
      }
      exprs[0].clone()
    }
    rule comparison() -> ExprKind = exprs:equ_expression() ** (_ op:("=" {CompareOp::Eq} / "<>" {CompareOp::Ne}) _) {
      // TODO capture the operator type to distinguish
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::Compare {op: CompareOp::Eq, terms: exprs}
      }
      exprs[0].clone()
    }
    rule equ_expression() -> ExprKind = exprs:add_expression() ** (_ comparison_operator() _) {// TODO capture the operator type to distinguish
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::Compare {op: CompareOp::Lt, terms: exprs}
      }
      exprs[0].clone()
    }
    rule comparison_operator() -> CompareOp = "<"  {CompareOp::Lt } / ">" {CompareOp::Gt} / "<=" {CompareOp::LtEq} / ">=" {CompareOp::GtEq}
    rule add_expression() -> ExprKind = exprs:term() ** (_ add_operator() _ ) {
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::BinaryOp {ops: vec![Operator::Add], terms: exprs}
      }
      exprs[0].clone()
    }
    rule add_operator() -> Operator = "+" {Operator::Add} / "-" {Operator::Sub}
    rule term() -> ExprKind = exprs:power_expression() ** (_ multiply_operator() _) {
      if exprs.len() > 1 {
        // TODO this is wrong op type
        return ExprKind::BinaryOp {ops: vec![Operator::Mul], terms: exprs}
      }
      exprs[0].clone()
    }
    rule multiply_operator() -> Operator = "*" {Operator::Mul} / "/" {Operator::Div}/ "MOD" {Operator::Mod}
    rule power_expression() -> ExprKind = exprs:unary_expression() ** (_ "**" _)  {
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
    rule function_expression() -> ExprKind = name:function_name() _ "(" params:param_assignment() ++ (_ "," _) _ ")" {
      ExprKind::Function {
        name: String::from(name),
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
        name: String::from(name),
        params: params,
      })
    }
    // TODO this needs much more
    rule param_assignment() -> ParamAssignment = name:(n:variable_name() _ ":=" { n })? _ expr:expression() {
      let name = name.map(|n| String::from(n));
      ParamAssignment::Input {
        name: name,
        expr: expr,
      }
    } / not:"NOT"? _ src:variable_name() _ "=>" _ tgt:variable() {
      ParamAssignment::Output {
        // TODO map this optional
        not: false,
        src: String::from(src),
        tgt: tgt,
      }
    }
    // B.3.2.3 Selection statement
    // TODO add case statement
    rule selection_statement() -> StmtKind = ifstmt:if_statement() { ifstmt }
    // TODO handle else if
    rule if_statement() -> StmtKind = "IF" _ expr:expression() _ "THEN" _ body:statement_list()? _ else_body:("ELSE" _ e:statement_list() { e })? _ "END_IF" {
      StmtKind::If{
        expr: expr,
        body: body.unwrap_or_else(|| vec![]),
        else_body: else_body.unwrap_or_else(|| vec![]),
      }
    }
  }
}

mod test {
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
                name: String::from("TRIG"),
                storage_class: StorageClass::Unspecified,
                initializer: Option::Some(TypeInitializer::Simple {
                    type_name: String::from("BOOL"),
                    initial_value: None,
                }),
            },
            VarInitDecl {
                name: String::from("MSG"),
                storage_class: StorageClass::Unspecified,
                initializer: Option::Some(TypeInitializer::Simple {
                    type_name: String::from("STRING"),
                    initial_value: None,
                }),
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
            name: String::from("LEVEL"),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::EnumeratedType {
                type_name: String::from("LOGLEVEL"),
                initial_value: Some(String::from("INFO")),
            }),
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
                name: String::from("TRIG"),
                storage_class: StorageClass::Unspecified,
                initializer: Option::Some(TypeInitializer::Simple {
                    type_name: String::from("BOOL"),
                    initial_value: None,
                }),
            },
            VarInitDecl {
                name: String::from("MSG"),
                storage_class: StorageClass::Unspecified,
                initializer: Option::Some(TypeInitializer::Simple {
                    type_name: String::from("STRING"),
                    initial_value: None,
                }),
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
            name: String::from("abc"),
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
            name: String::from("plc_task_instance"),
            task_name: Option::Some(String::from("plc_task")),
            type_name: String::from("plc_prg"),
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
            name: String::from("ResetCounterValue"),
            storage_class: StorageClass::Constant,
            at: None,
            initializer: Option::Some(TypeInitializer::Simple {
                type_name: String::from("INT"),
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
                name: String::from("Start"),
                action_associations: vec![],
            },
            elements: vec![
                Element::Step {
                    name: String::from("ResetCounter"),
                    action_associations: vec![
                        ActionAssociation {
                            name: String::from("RESETCOUNTER_INLINE1"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                        ActionAssociation {
                            name: String::from("RESETCOUNTER_INLINE2"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                    ],
                },
                Element::Action {
                    name: String::from("RESETCOUNTER_INLINE1"),
                    body: FunctionBlockBody::stmts(vec![StmtKind::assignment(
                        Variable::symbolic("Cnt"),
                        ExprKind::symbolic_variable(
                            "ResetCounterValue"
                        ),
                    )]),
                },
                Element::Transition {
                    name: None,
                    priority: None,
                    from: vec![String::from("ResetCounter")],
                    to: vec![String::from("Start")],
                    condition: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: ExprKind::boxed_symbolic_variable("Reset"),
                    },
                },
                Element::Transition {
                    name: None,
                    priority: None,
                    from: vec![String::from("Start")],
                    to: vec![String::from("Count")],
                    condition: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: Box::new(ExprKind::symbolic_variable(
                            "Reset"
                        )),
                    },
                },
                Element::Step {
                    name: String::from("Count"),
                    action_associations: vec![
                        ActionAssociation {
                            name: String::from("COUNT_INLINE3"),
                            qualifier: Some(ActionQualifier::N),
                            indicators: vec![],
                        },
                        ActionAssociation {
                            name: String::from("COUNT_INLINE4"),
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
        let expected = Ok(vec![StmtKind::If {
            expr: ExprKind::Compare {
                op: CompareOp::And,
                terms: vec![
                    ExprKind::symbolic_variable("TRIG"),
                    ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        term: Box::new(ExprKind::symbolic_variable(
                            "TRIG"
                        )),
                    },
                ],
            },
            body: vec![StmtKind::assignment(
                Variable::symbolic("TRIG0"),
                ExprKind::symbolic_variable("TRIG"),
            )],
            else_body: vec![],
        }]);
        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_if() {
        let statement = "IF Reset THEN
    Cnt := ResetCounterValue;
  ELSE
    Cnt := Cnt + 1;
  END_IF;";
        let expected = Ok(vec![StmtKind::If {
            expr: ExprKind::symbolic_variable("Reset"),
            body: vec![StmtKind::assignment(
                Variable::symbolic("Cnt"),
                ExprKind::symbolic_variable(
                    "ResetCounterValue"
                ),
            )],
            else_body: vec![StmtKind::assignment(
                Variable::symbolic("Cnt"),
                ExprKind::BinaryOp {
                    ops: vec![Operator::Add],
                    terms: vec![
                        ExprKind::symbolic_variable("Cnt"),
                        ExprKind::Const(Constant::IntegerLiteral(1)),
                    ],
                },
            )],
        }]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_without_name() {
        let statement = "CounterLD0(Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            name: String::from("CounterLD0"),
            params: vec![ParamAssignment::Input {
                name: None,
                expr: ExprKind::symbolic_variable("Reset"),
            }],
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn statement_fb_invocation_with_name() {
        let statement = "CounterLD0(Cnt := Reset);";
        let expected = Ok(vec![StmtKind::FbCall(FbCall {
            name: String::from("CounterLD0"),
            params: vec![ParamAssignment::Input {
                name: Option::Some(String::from("Cnt")),
                expr: ExprKind::symbolic_variable("Reset"),
            }],
        })]);

        assert_eq!(plc_parser::statement_list(statement), expected)
    }

    #[test]
    fn assignment() {
        let assign = "Cnt1 := CounterST0.OUT";
        let expected = Ok(StmtKind::assignment(
            Variable::symbolic("Cnt1"),
            ExprKind::Variable(Variable::MultiElementVariable(vec![
                String::from("CounterST0"),
                String::from("OUT"),
            ])),
        ));
        assert_eq!(plc_parser::assignment_statement(assign), expected)
    }
}
