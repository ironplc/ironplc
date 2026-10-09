//! Tests of the bindings `xform_bind_variables` records.
//!
//! Each test names a declaration by the scope it is declared in and its
//! name (`decl("F", "x")`, `decl("", "x")` for a global) and a reference the
//! same way (`refs("F", "x")`, every reference to `x` written in `F`'s
//! scope, in source order), and asserts the reference carries the
//! declaration's id.
use std::collections::HashMap;
use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::decl_id::DeclId;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options};

/// The declarations and references of a library, keyed by the scope they
/// are written in, as a dotted path of declaration names (`FB.M`), and by
/// the name declared or referred to, lower-cased.
#[derive(Default)]
struct Bindings {
    scope: Vec<String>,
    decls: HashMap<(String, String), Option<DeclId>>,
    refs: HashMap<(String, String), Vec<Option<DeclId>>>,
}

impl Bindings {
    fn of(program: &str, options: &CompilerOptions) -> Self {
        let (library, _) = parse_and_resolve_types_with_options(program, options);
        let mut bindings = Bindings::default();
        let Ok(()) = bindings.walk(&library);
        bindings
    }

    fn key(&self, name: &Id) -> (String, String) {
        (self.scope.join("."), name.lower_case().to_string())
    }

    fn reference(&mut self, name: &Id, decl_id: Option<DeclId>) {
        let key = self.key(name);
        self.refs.entry(key).or_default().push(decl_id);
    }

    /// The id of the declaration of `name` in `scope`.
    fn decl(&self, scope: &str, name: &str) -> Option<DeclId> {
        self.decls[&(scope.to_string(), name.to_lowercase())]
    }

    /// The ids every reference to `name` written in `scope` records.
    fn refs(&self, scope: &str, name: &str) -> Vec<Option<DeclId>> {
        self.refs[&(scope.to_string(), name.to_lowercase())].clone()
    }
}

impl Visitor<Infallible> for Bindings {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        let name = match node {
            ScopeNode::Function(node) => node.name.clone(),
            ScopeNode::FunctionBlock(node) => node.name.name.clone(),
            ScopeNode::Program(node) => node.name.clone(),
            ScopeNode::Method(node) => node.name.clone(),
            ScopeNode::MethodPrototype(node) => node.name.clone(),
            ScopeNode::Interface(node) => node.name.clone(),
        };
        self.scope.push(name.lower_case().to_string());
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.pop();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        if let Some(name) = node.identifier.symbolic_id() {
            let key = self.key(name);
            self.decls.insert(key, node.decl_id);
        }
        node.recurse_visit(self)
    }

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Infallible> {
        let key = (
            node.name.lower_case().to_string(),
            node.name.lower_case().to_string(),
        );
        self.decls.insert(key, node.result_decl_id);
        node.recurse_visit(self)
    }

    fn visit_named_variable(&mut self, node: &NamedVariable) -> Result<(), Infallible> {
        self.reference(&node.name, node.decl_id);
        Ok(())
    }

    fn visit_for(&mut self, node: &For) -> Result<(), Infallible> {
        self.reference(&node.control, node.control_decl_id);
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<(), Infallible> {
        self.reference(&node.var_name, node.instance_decl_id);
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<(), Infallible> {
        if let MethodReceiver::Instance(name) = &node.receiver {
            self.reference(name, node.receiver_decl_id);
        }
        node.recurse_visit(self)
    }
}

fn bindings(program: &str) -> Bindings {
    Bindings::of(program, &CompilerOptions::default())
}

/// Wraps `body` in a configuration that declares the global `x : <global>`
/// and a program that runs `F`, after the type declarations every case uses.
fn with_global_x(global: &str, function: &str) -> String {
    format!(
        "
TYPE
    BIG : DINT(0..100000);
    POINT : STRUCT px : DINT; py : DINT; END_STRUCT;
    LIST : ARRAY[1..3] OF DINT;
END_TYPE
{function}
PROGRAM main VAR r : DINT; END_VAR r := 0; END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL x : {global}; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION"
    )
}

#[spec_test(REQ_VB_analyzer_010)]
#[rstest]
fn apply_when_function_local_hides_global_then_reference_binds_local(
    #[values("INT", "BIG", "STRING", "POINT", "LIST", "TON")] global: &str,
    #[values("INT", "BIG", "STRING", "POINT", "LIST", "TON")] local: &str,
) {
    let function = format!(
        "FUNCTION F : BOOL VAR x : {local}; y : {local}; END_VAR y := x; F := TRUE; END_FUNCTION"
    );
    let bindings = bindings(&with_global_x(global, &function));

    assert_eq!(bindings.refs("f", "x"), vec![bindings.decl("f", "x")]);
    assert_ne!(bindings.decl("f", "x"), bindings.decl("", "x"));
}

// A function cannot declare VAR_EXTERNAL (IEC 61131-3 B.1.5.1); it reads a
// global directly, which `apply_when_function_reads_global_then_reference_binds_global`
// covers.
#[spec_test(REQ_VB_analyzer_020)]
#[rstest]
#[case::program(
    "PROGRAM P VAR_EXTERNAL x : INT; END_VAR VAR y : INT; END_VAR y := x; END_PROGRAM",
    "p"
)]
#[case::function_block(
    "FUNCTION_BLOCK FB VAR_EXTERNAL x : INT; END_VAR VAR y : INT; END_VAR y := x; END_FUNCTION_BLOCK",
    "fb"
)]
fn apply_when_reference_through_var_external_then_binds_global(
    #[case] unit: &str,
    #[case] scope: &str,
) {
    let bindings = bindings(&with_global_x("INT", unit));

    assert_eq!(bindings.refs(scope, "x"), vec![bindings.decl("", "x")]);
    assert_ne!(bindings.decl(scope, "x"), bindings.decl("", "x"));
}

#[spec_test(REQ_VB_analyzer_030)]
#[test]
fn apply_when_var_in_out_then_reference_binds_parameter() {
    let program = "
FUNCTION F : INT VAR_IN_OUT io : INT; END_VAR io := io + 1; F := io; END_FUNCTION";

    let bindings = bindings(program);

    let param = bindings.decl("f", "io");
    assert!(param.is_some());
    assert_eq!(bindings.refs("f", "io"), vec![param; 3]);
}

#[spec_test(REQ_VB_analyzer_013)]
#[test]
fn apply_when_function_assigns_its_name_then_binds_result_variable() {
    let program = "FUNCTION F : INT F := 1; END_FUNCTION";

    let bindings = bindings(program);

    assert!(bindings.decl("f", "f").is_some());
    assert_eq!(bindings.refs("f", "f"), vec![bindings.decl("f", "f")]);
}

#[spec_test(REQ_VB_analyzer_015)]
#[test]
fn apply_when_field_path_then_root_binds_its_declaration() {
    let program = "
TYPE
    INNER : STRUCT v : DINT; END_STRUCT;
    OUTER : STRUCT inner : INNER; items : ARRAY[1..2] OF INNER; END_STRUCT;
END_TYPE
FUNCTION F : DINT
VAR s : OUTER; a : ARRAY[1..2] OF OUTER; i : DINT; END_VAR
F := s.inner.v + s.items[i].v + a[i].inner.v;
END_FUNCTION";

    let bindings = bindings(program);

    assert_eq!(bindings.refs("f", "s"), vec![bindings.decl("f", "s"); 2]);
    assert_eq!(bindings.refs("f", "a"), vec![bindings.decl("f", "a")]);
    assert_eq!(bindings.refs("f", "i"), vec![bindings.decl("f", "i"); 2]);
}

#[spec_test(REQ_VB_analyzer_015)]
#[test]
fn apply_when_dereference_then_reference_binds_its_declaration() {
    let program = "
TYPE POINT : STRUCT px : DINT; END_STRUCT; END_TYPE
FUNCTION F : DINT VAR p : REF_TO POINT; END_VAR F := p^.px; END_FUNCTION";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };

    let bindings = Bindings::of(program, &options);

    assert_eq!(bindings.refs("f", "p"), vec![bindings.decl("f", "p")]);
}

#[spec_test(REQ_VB_analyzer_014)]
#[test]
fn apply_when_for_fb_call_and_method_call_then_each_binds_its_declaration() {
    let program = "
FUNCTION_BLOCK COUNTER
VAR_INPUT step : INT; END_VAR
VAR count : INT; END_VAR
count := count + step;
METHOD Reset count := 0; END_METHOD
END_FUNCTION_BLOCK
PROGRAM P
VAR c : COUNTER; i : INT; END_VAR
FOR i := 1 TO 3 DO c(step := i); END_FOR;
c.Reset();
END_PROGRAM";

    let bindings = Bindings::of(program, &fb_inheritance_options());

    assert_eq!(bindings.refs("p", "c"), vec![bindings.decl("p", "c"); 2]);
    assert!(bindings.decl("p", "c").is_some());
    assert_eq!(bindings.refs("p", "i"), vec![bindings.decl("p", "i"); 2]);
    assert_eq!(
        bindings.refs("counter.reset", "count"),
        vec![bindings.decl("counter", "count")]
    );
}

#[spec_test(REQ_VB_analyzer_012)]
#[test]
fn apply_when_derived_block_reads_inherited_field_then_binds_base_field() {
    let program = "
FUNCTION_BLOCK BASE VAR count : INT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK DERIVED EXTENDS BASE count := count + 1; END_FUNCTION_BLOCK";

    let bindings = Bindings::of(program, &fb_inheritance_options());

    assert_eq!(
        bindings.refs("derived", "count"),
        vec![bindings.decl("base", "count"); 2]
    );
}

#[spec_test(REQ_VB_analyzer_011)]
#[test]
fn apply_when_method_local_hides_field_then_binds_local() {
    let program = "
FUNCTION_BLOCK FB
VAR count : INT; other : INT; END_VAR
METHOD M VAR count : INT; END_VAR count := 1; other := count; END_METHOD
END_FUNCTION_BLOCK";

    let bindings = Bindings::of(program, &fb_inheritance_options());

    assert_eq!(
        bindings.refs("fb.m", "count"),
        vec![bindings.decl("fb.m", "count"); 2]
    );
    assert_eq!(
        bindings.refs("fb.m", "other"),
        vec![bindings.decl("fb", "other")]
    );
    assert_ne!(bindings.decl("fb.m", "count"), bindings.decl("fb", "count"));
}

#[spec_test(REQ_VB_analyzer_016)]
#[test]
fn apply_when_name_declares_no_variable_then_reference_is_unbound() {
    let program = "PROGRAM P VAR y : INT; END_VAR y := undeclared; END_PROGRAM";

    let bindings = bindings(program);

    assert_eq!(bindings.refs("p", "undeclared"), vec![None]);
}

#[spec_test(REQ_VB_analyzer_004)]
#[test]
fn apply_when_symbols_built_then_each_symbol_records_its_declarations_id() {
    let program = "FUNCTION F : INT VAR_INPUT a : INT; END_VAR F := a; END_FUNCTION";
    let (library, context) =
        parse_and_resolve_types_with_options(program, &CompilerOptions::default());
    let mut bindings = Bindings::default();
    let Ok(()) = bindings.walk(&library);
    let scope = crate::symbol_environment::ScopeKind::Named(Id::from("F").into());

    let symbol = |name: &str| {
        context
            .symbols()
            .find(&Id::from(name), &scope)
            .unwrap()
            .decl_id
    };

    assert_eq!(symbol("a"), bindings.decl("f", "a"));
    assert_eq!(symbol("F"), bindings.decl("f", "f"));
}

#[spec_test(REQ_VB_analyzer_005)]
#[test]
fn eq_when_declarations_and_references_differ_only_by_binding_then_equal() {
    let mut decl = VarDecl::simple("x", "INT");
    decl.decl_id = Some(DeclId::from_raw(7));
    let mut reference = NamedVariable::new(Id::from("x"));
    reference.decl_id = Some(DeclId::from_raw(7));

    assert_eq!(decl, VarDecl::simple("x", "INT"));
    assert_eq!(reference, NamedVariable::new(Id::from("x")));
}

/// Every late-bound expression `resolve_types` leaves.
#[derive(Default)]
struct LateBoundNames {
    names: Vec<Id>,
}

impl Visitor<Infallible> for LateBoundNames {
    type Value = ();

    fn visit_expr_kind(&mut self, node: &ExprKind) -> Result<(), Infallible> {
        if let ExprKind::LateBound(late) = node {
            self.names.push(late.value.clone());
        }
        node.recurse_visit(self)
    }
}

#[spec_test(REQ_VB_analyzer_040)]
#[test]
fn resolve_types_when_bare_names_then_none_is_left_late_bound() {
    let program = "
TYPE COLOR : (RED, GREEN); END_TYPE
FUNCTION_BLOCK FB VAR_INPUT i : INT; END_VAR VAR c : COLOR; END_VAR c := GREEN; END_FUNCTION_BLOCK
FUNCTION F : INT VAR_INPUT a : INT; END_VAR F := a; END_FUNCTION
PROGRAM P
VAR x : INT; y : INT; c : COLOR; b : FB; END_VAR
x := y;
x := F(y);
b(i := y);
IF c = RED THEN x := y + 1; END_IF;
END_PROGRAM";
    let (library, _) = parse_and_resolve_types_with_options(program, &CompilerOptions::default());
    let mut late_bound = LateBoundNames::default();

    let Ok(()) = late_bound.walk(&library);

    assert!(late_bound.names.is_empty(), "{:?}", late_bound.names);
}
