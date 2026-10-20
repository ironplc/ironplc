//! Tests of methods: their declaration, and the `Call` statement's
//! constructor and text form.

use super::*;

/// A program with a function block `Counter`, a variable `c` of it, a
/// `DINT` `d`, and a POU `Add` to declare a method of it with an instance
/// parameter `this`, an input `a` and a result `r`.
struct Method {
    b: ProgramBuilder,
    body: PouId,
    counter: TypeId,
    instance: VarId,
    dint: VarId,
    pou: PouId,
    this: VarId,
    a: VarId,
    r: VarId,
}

fn method() -> Method {
    let mut b = ProgramBuilder::new();
    let dint_type = b
        .push_type(
            entry(TypeKind::Integer {
                width: IntWidth::B32,
                signed: true,
            }),
            &span(),
        )
        .unwrap();
    let main = b.push_pou(name("main"));
    let body = b.push_pou(name("Counter"));
    let counter = b
        .push_type(
            entry(TypeKind::FunctionBlock {
                fields: vec![Field::new("n", span(), Some(Section::Local), dint_type)],
                body,
                base: None,
            }),
            &span(),
        )
        .unwrap();
    let instance = b
        .push_variable(name("c"), Owner::Pou(main), counter)
        .unwrap();
    let dint = b
        .push_variable(name("d"), Owner::Pou(main), dint_type)
        .unwrap();
    let pou = b.push_method_pou(name("Add"), body);
    let this_type = b
        .push_type(
            entry(TypeKind::Reference {
                target: counter,
                nullability: Nullability::NonNull,
            }),
            &span(),
        )
        .unwrap();
    let mut var = |text: &str, ty| b.push_variable(name(text), Owner::Pou(pou), ty).unwrap();
    let this = var("THIS", this_type);
    let a = var("a", dint_type);
    let r = var("Add", dint_type);
    Method {
        b,
        body,
        counter,
        instance,
        dint,
        pou,
        this,
        a,
        r,
    }
}

impl Method {
    fn args(&self) -> Vec<Arg> {
        let instance = self.b.var(self.instance, &span()).unwrap();
        let d = self.b.var(self.dint, &span()).unwrap();
        vec![
            Arg::Ref(instance),
            Arg::Value(self.b.read(d, span()).unwrap()),
        ]
    }
}

/// REQ-LOW-lowering-076: a method's first parameter is its instance
/// parameter, which takes a `Ref` to an instance of its function block, and
/// the method belongs to that block's body. A call of it with a result is a
/// value of the result's type.
#[test]
fn declare_method_when_first_parameter_is_instance_then_method_of_its_block() {
    let mut m = method();
    m.b.declare_method(m.pou, vec![m.this, m.a], Some(m.r), &span())
        .unwrap();
    assert_eq!(m.b.pous().method_of(m.pou), Some(m.body));
    assert_eq!(m.b.pous().method_of(m.body), None);
    assert_eq!(
        m.b.params(m.pou),
        Some(vec![
            Param::Ref(m.counter),
            Param::Value(ScalarType::I32(Signedness::Signed))
        ])
    );
    let call = m.b.call(m.pou, m.args(), span()).unwrap();
    assert_eq!(call.to_string(), "(call pou2 i32s (ref v0) (read i32s v1))");
}

/// REQ-LOW-ir-173: a method without a result is called only by a
/// statement, which prints its arguments as a call expression does.
#[test]
fn call_stmt_when_method_without_result_then_statement_only() {
    let mut m = method();
    m.b.declare_method(m.pou, vec![m.this, m.a], None, &span())
        .unwrap();
    let stmt = m.b.call_stmt(m.pou, m.args(), span()).unwrap();
    assert!(matches!(
        stmt.kind(),
        StmtKind::Call { callee: Callee::User(pou), args } if *pou == m.pou && args.len() == 2
    ));
    assert_eq!(stmt.to_string(), "call pou2 (ref v0) (read i32s v1)");
    assert!(is_internal_error_naming(
        m.b.call(m.pou, m.args(), span()),
        "ProgramBuilder::call"
    ));
}

/// REQ-LOW-ir-173: a call statement discards the result of a method that
/// has one, and is printed indented inside an enclosing statement.
#[test]
fn call_stmt_when_method_with_result_inside_if_then_printed_in_its_block() {
    let mut m = method();
    m.b.declare_method(m.pou, vec![m.this, m.a], Some(m.r), &span())
        .unwrap();
    let call = m.b.call_stmt(m.pou, m.args(), span()).unwrap();
    let stmt = Stmt::if_(Expr::boolean(true, span()), vec![call], vec![], span()).unwrap();
    assert_eq!(
        stmt.to_string(),
        "if (const bool true)\n  call pou2 (ref v0) (read i32s v1)\nend"
    );
}

/// REQ-LOW-ir-173: a call statement passes one argument for each parameter,
/// at its type.
#[test]
fn call_stmt_when_arguments_do_not_match_then_internal_error() {
    let mut m = method();
    m.b.declare_method(m.pou, vec![m.this, m.a], None, &span())
        .unwrap();
    let mut missing = m.args();
    missing.pop();
    let mut swapped = m.args();
    swapped.reverse();
    for args in [missing, swapped] {
        assert!(is_internal_error_naming(
            m.b.call_stmt(m.pou, args, span()),
            "ProgramBuilder::call_stmt"
        ));
    }
    assert!(is_internal_error_naming(
        m.b.call_stmt(m.body, vec![], span()),
        "ProgramBuilder::call_stmt"
    ));
}

/// REQ-LOW-lowering-076, REQ-LOW-ir-173: a method's first parameter is a
/// non-null reference to an instance of its function block, and only a
/// method is declared one.
#[test]
fn declare_method_when_first_parameter_not_instance_then_internal_error() {
    let mut m = method();
    let function = m.b.push_pou(name("f"));
    let calls = [
        (m.pou, vec![]),
        (m.pou, vec![m.a, m.this]),
        (function, vec![m.this, m.a]),
    ];
    for (pou, parameters) in calls {
        assert!(is_internal_error_naming(
            m.b.declare_method(pou, parameters, None, &span()),
            "ProgramBuilder::declare_method"
        ));
    }
    assert!(is_internal_error_naming(
        m.b.declare_function(m.pou, vec![m.a], m.r, &span()),
        "ProgramBuilder::declare_function"
    ));
}

impl Method {
    /// A function block type that extends `base` with `fields`, and its body.
    fn block_extending(
        &mut self,
        block: &str,
        fields: Vec<Field>,
        base: Option<TypeId>,
    ) -> Result<TypeId, ironplc_dsl::diagnostic::Diagnostic> {
        let body = self.b.push_pou(name(block));
        self.b.push_type(
            entry(TypeKind::FunctionBlock { fields, body, base }),
            &span(),
        )
    }

    /// A field of a function block of the type of `d`, named `text`.
    fn dint_field(&self, text: &str) -> Field {
        let dint = self.b.variables().get(self.dint).unwrap().ty();
        Field::new(text, span(), Some(Section::Local), dint)
    }
}

/// REQ-LOW-ir-176: a function block type that extends another begins with
/// its fields, and extends it and every block it extends in turn; a block
/// extends itself and no block that extends it.
#[test]
fn push_type_when_block_begins_with_base_fields_then_extends_it() {
    let mut m = method();
    let fields = vec![m.dint_field("n"), m.dint_field("k")];
    let derived = m
        .block_extending("Derived", fields.clone(), Some(m.counter))
        .unwrap();
    let leaf = m.block_extending("Leaf", fields, Some(derived)).unwrap();
    let types = m.b.types();
    assert!(types.extends(derived, m.counter));
    assert!(types.extends(leaf, m.counter));
    assert!(types.extends(m.counter, m.counter));
    assert!(!types.extends(m.counter, derived));
}

/// REQ-LOW-ir-176: a function block type does not extend one whose fields
/// it does not begin with, nor a type that is not a function block.
#[test]
fn push_type_when_block_does_not_begin_with_base_fields_then_internal_error() {
    let mut m = method();
    let renamed = vec![m.dint_field("k")];
    let dint = m.b.variables().get(m.dint).unwrap().ty();
    assert!(is_internal_error_naming(
        m.block_extending("Missing", vec![], Some(m.counter)),
        "TypeTable::push"
    ));
    assert!(is_internal_error_naming(
        m.block_extending("Renamed", renamed, Some(m.counter)),
        "TypeTable::push"
    ));
    assert!(is_internal_error_naming(
        m.block_extending("OfInteger", vec![], Some(dint)),
        "TypeTable::push"
    ));
}

/// REQ-LOW-ir-173, REQ-LOW-ir-176: a method's instance parameter takes a
/// `Ref` to an instance of a block that extends the method's block, and not
/// one to an instance of an unrelated block.
#[test]
fn call_when_receiver_extends_method_block_then_passes() {
    let mut m = method();
    m.b.declare_method(m.pou, vec![m.this, m.a], Some(m.r), &span())
        .unwrap();
    let fields = vec![m.dint_field("n")];
    let derived = m
        .block_extending("Derived", fields.clone(), Some(m.counter))
        .unwrap();
    let other = m.block_extending("Other", fields, None).unwrap();
    let main = m.b.variables().get(m.dint).unwrap().owner();
    let mut args = |ty| {
        let instance = m.b.push_variable(name("x"), main, ty).unwrap();
        let mut args = m.args();
        args[0] = Arg::Ref(m.b.var(instance, &span()).unwrap());
        args
    };
    let (of_derived, of_other) = (args(derived), args(other));
    assert!(m.b.call(m.pou, of_derived, span()).is_ok());
    assert!(is_internal_error_naming(
        m.b.call(m.pou, of_other, span()),
        "ProgramBuilder::call"
    ));
}

/// REQ-LOW-lowering-072, REQ-LOW-lowering-075: a method's input of a
/// function block type takes a copy of an instance of that block, a `Copy`
/// argument. A `Ref` to the instance, and a copy of a place of another type,
/// are refused.
#[test]
fn call_when_instance_input_then_takes_copy_of_instance() {
    let mut m = method();
    let i =
        m.b.push_variable(name("i"), Owner::Pou(m.pou), m.counter)
            .unwrap();
    m.b.declare_method(m.pou, vec![m.this, i], Some(m.r), &span())
        .unwrap();
    assert_eq!(
        m.b.params(m.pou),
        Some(vec![Param::Ref(m.counter), Param::Copy(m.counter)])
    );
    let c = m.b.var(m.instance, &span()).unwrap();
    let d = m.b.var(m.dint, &span()).unwrap();
    let call =
        m.b.call(
            m.pou,
            vec![Arg::Ref(c.clone()), Arg::Copy(c.clone())],
            span(),
        )
        .unwrap();
    assert_eq!(call.to_string(), "(call pou2 i32s (ref v0) (copy v0))");
    for refused in [Arg::Ref(c.clone()), Arg::Copy(d)] {
        assert!(is_internal_error_naming(
            m.b.call(m.pou, vec![Arg::Ref(c.clone()), refused], span()),
            "ProgramBuilder::call"
        ));
    }
}
