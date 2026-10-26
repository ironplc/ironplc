//! Tests of a whole lowered program: the parts and body of each POU, `init`,
//! the bindings of the execution model, what `finish` refuses, and the
//! program's text.

use super::*;
use crate::execution::{
    Configuration, ExecutionModel, ExecutionModelBuilder, Global, GlobalKind, GlobalScope,
    InstanceOf, ProgramInstance, ProgramType, Resource, Schedule, Task,
};

/// One program `main`, with one field `x`, and one instance `inst` of it,
/// which a global `g` is in scope of.
struct Whole {
    b: ProgramBuilder,
    model: ExecutionModel,
    bindings: ExecutionBindings,
    main: PouId,
    this: VarId,
    inst: VarId,
    g: VarId,
}

fn whole() -> Whole {
    let mut b = ProgramBuilder::new();
    let main = b.push_pou(name("main"));
    let dint = b
        .push_type(
            entry(TypeKind::Integer {
                width: IntWidth::B32,
                signed: true,
            }),
            &span(),
        )
        .unwrap();
    let program = b
        .push_type(
            TypeEntry {
                name: Some(name("main")),
                kind: TypeKind::Program {
                    fields: vec![Field::new("x", span(), Some(Section::Local), dint)],
                    body: main,
                },
            },
            &span(),
        )
        .unwrap();
    let this_type = b
        .push_type(
            entry(TypeKind::Reference {
                target: program,
                nullability: Nullability::NonNull,
            }),
            &span(),
        )
        .unwrap();
    let declared = |section| Declared::in_section(section);
    let g = b
        .push_declared_variable(
            name("g"),
            Owner::Global,
            Declared {
                retention: Retention::Retain,
                ..declared(Section::Global)
            },
            dint,
        )
        .unwrap();
    let inst = b
        .push_declared_variable(
            name("inst"),
            Owner::Global,
            declared(Section::Instance),
            program,
        )
        .unwrap();
    let this = b
        .push_declared_variable(
            name("THIS"),
            Owner::Pou(main),
            declared(Section::Instance),
            this_type,
        )
        .unwrap();

    let mut model = ExecutionModelBuilder::new();
    let program_id = model.add_program(ProgramType { name: name("main") });
    let global_id = model.add_global(Global {
        name: name("g"),
        kind: GlobalKind::Declared(GlobalScope::Configuration),
    });
    let model = model.build(vec![Configuration {
        name: Some(name("config")),
        resources: vec![Resource {
            name: Some(name("res")),
            globals: Vec::new(),
            tasks: vec![Task {
                name: Some(name("t")),
                priority: 1,
                schedule: Schedule::Freewheeling,
                instances: vec![ProgramInstance {
                    name: Some(name("inst")),
                    program: InstanceOf::Program(program_id),
                }],
            }],
        }],
    }]);
    let mut bindings = ExecutionBindings::new();
    bindings.bind_program(program_id, main);
    bindings.bind_global(global_id, g);
    bindings.bind_instance(0, inst);
    Whole {
        b,
        model,
        bindings,
        main,
        this,
        inst,
        g,
    }
}

impl Whole {
    /// Declares `main` a program on `THIS`, whose body stores `g` in `x`, and
    /// whose `init` stores 5 in `g`.
    fn complete(mut self) -> Self {
        let parts = PouParts {
            kind: PouKind::Program,
            instance: Some(self.this),
            locals: vec![],
            declarations: vec![],
            result: None,
        };
        self.b.declare_parts(self.main, parts, &span()).unwrap();
        let this = self.b.var(self.this, &span()).unwrap();
        let instance = self.b.deref(this, &span()).unwrap();
        let x_field = self.b.field_idx(self.instance_type(), 0).unwrap();
        let x = self.b.field(instance, x_field, &span()).unwrap();
        let g = self.b.var(self.g, &span()).unwrap();
        let read = self.b.read(g.clone(), span()).unwrap();
        let body = vec![self.b.assign(x, read, span()).unwrap()];
        self.b.set_body(self.main, vec![], body, &span()).unwrap();
        let init = self.b.assign(g, int(5), span()).unwrap();
        self.b.push_init(init);
        self
    }

    fn instance_type(&self) -> TypeId {
        self.b.variables().get(self.inst).unwrap().ty()
    }

    fn finish(self) -> Result<Program, ironplc_dsl::diagnostic::Diagnostic> {
        self.b.finish(self.model, self.bindings)
    }
}

/// REQ-LOW-ir-029: a program whose POUs all have a body, and whose model is
/// bound, is finished, and prints whole.
#[test]
fn finish_when_complete_then_program_prints_whole() {
    let program = whole().complete().finish().unwrap();

    assert_eq!(
        program.to_string(),
        "types
  t0 i32
  t1 \"main\" program pou0
    f0 x : t0 local
  t2 ref non-null t1
variables
  v0 g : t0 global retain
  v1 inst : t1 instance
  v2 THIS : t2 instance of pou0
pou0 program main instance v2
  assign v2^.f0 (read i32s v0)
end
init
  assign v0 (const i32s 5)
end
program main pou0
global g v0
instance inst v1
"
    );
    let main = program.pous().ids().next().unwrap();
    assert_eq!(program.pous().body(main).map(<[Stmt]>::len), Some(1));
    assert_eq!(program.init().len(), 1);
}

/// REQ-LOW-ir-081: a POU with no body is not a whole program.
#[test]
fn finish_when_pou_has_no_body_then_internal_error() {
    let mut w = whole();
    let parts = PouParts {
        kind: PouKind::Program,
        instance: Some(w.this),
        locals: vec![],
        declarations: vec![],
        result: None,
    };
    w.b.declare_parts(w.main, parts, &span()).unwrap();

    assert!(is_internal_error_naming(
        w.finish(),
        "ProgramBuilder::finish"
    ));
}

/// REQ-LOW-ir-081: a POU with no parts is not a whole program.
#[test]
fn finish_when_pou_has_no_parts_then_internal_error() {
    let mut w = whole();
    w.b.set_body(w.main, vec![], vec![], &span()).unwrap();

    assert!(is_internal_error_naming(
        w.finish(),
        "ProgramBuilder::finish"
    ));
}

/// REQ-LOW-ir-029: a function needs a signature for a call to reach it.
#[test]
fn finish_when_function_has_no_signature_then_internal_error() {
    let mut w = whole().complete();
    let function = w.b.push_pou(name("F"));
    let parts = PouParts {
        kind: PouKind::Function,
        instance: None,
        locals: vec![],
        declarations: vec![],
        result: None,
    };
    w.b.declare_parts(function, parts, &span()).unwrap();
    w.b.set_body(function, vec![], vec![], &span()).unwrap();

    assert!(is_internal_error_naming(
        w.finish(),
        "ProgramBuilder::finish"
    ));
}

/// REQ-LOW-ir-029: every global of the model is bound to a global variable.
#[test]
fn finish_when_global_unbound_or_bound_to_instance_then_internal_error() {
    let mut unbound = whole().complete();
    unbound.bindings = ExecutionBindings::new();
    let (program_id, _) = unbound.model.programs().next().unwrap();
    unbound.bindings.bind_program(program_id, unbound.main);
    unbound.bindings.bind_instance(0, unbound.inst);
    assert!(is_internal_error_naming(
        unbound.finish(),
        "ProgramBuilder::finish"
    ));

    let mut misbound = whole().complete();
    let (global_id, _) = misbound.model.globals().next().unwrap();
    misbound.bindings.bind_global(global_id, misbound.inst);
    assert!(is_internal_error_naming(
        misbound.finish(),
        "ProgramBuilder::finish"
    ));
}

/// REQ-LOW-ir-029: an instance of a program type is bound to a global
/// instance variable of that program's type, and to nothing beyond the
/// model's instances.
#[test]
fn finish_when_instance_unbound_misbound_or_extra_then_internal_error() {
    let mut misbound = whole().complete();
    misbound.bindings.bind_instance(0, misbound.g);
    assert!(is_internal_error_naming(
        misbound.finish(),
        "ProgramBuilder::finish"
    ));

    let mut extra = whole().complete();
    extra.bindings.bind_instance(1, extra.inst);
    assert!(is_internal_error_naming(
        extra.finish(),
        "ProgramBuilder::finish"
    ));

    let mut unbound = whole().complete();
    unbound.bindings = ExecutionBindings::new();
    let (program_id, _) = unbound.model.programs().next().unwrap();
    let (global_id, _) = unbound.model.globals().next().unwrap();
    unbound.bindings.bind_program(program_id, unbound.main);
    unbound.bindings.bind_global(global_id, unbound.g);
    assert!(is_internal_error_naming(
        unbound.finish(),
        "ProgramBuilder::finish"
    ));
}

/// A program's instance is a variable it owns, of section `Instance`, that
/// refers to its own type; a function has none, and a method is the only
/// POU of kind `Method`.
#[test]
fn declare_parts_when_instance_or_kind_wrong_then_internal_error() {
    let parts = |kind, instance| PouParts {
        kind,
        instance,
        locals: vec![],
        declarations: vec![],
        result: None,
    };

    let mut w = whole();
    let result =
        w.b.declare_parts(w.main, parts(PouKind::Program, None), &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));

    let mut w = whole();
    let result =
        w.b.declare_parts(w.main, parts(PouKind::Program, Some(w.inst)), &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));

    let mut w = whole();
    let result =
        w.b.declare_parts(w.main, parts(PouKind::FunctionBlock, Some(w.this)), &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));

    let mut w = whole();
    let result =
        w.b.declare_parts(w.main, parts(PouKind::Method, Some(w.this)), &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));

    let mut w = whole();
    let function = w.b.push_pou(name("F"));
    let result =
        w.b.declare_parts(function, parts(PouKind::Function, Some(w.this)), &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));
}

/// A local is a variable its POU owns; parts and a body are declared once.
#[test]
fn declare_parts_when_local_of_another_pou_or_declared_twice_then_internal_error() {
    let mut w = whole();
    let parts = PouParts {
        kind: PouKind::Program,
        instance: Some(w.this),
        locals: vec![w.g],
        declarations: vec![],
        result: None,
    };
    let result = w.b.declare_parts(w.main, parts, &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));

    let mut w = whole().complete();
    let parts = PouParts {
        kind: PouKind::Program,
        instance: Some(w.this),
        locals: vec![],
        declarations: vec![],
        result: None,
    };
    let twice = w.b.declare_parts(w.main, parts, &span());
    assert!(is_internal_error_naming(
        twice,
        "ProgramBuilder::declare_parts"
    ));
    let body_twice = w.b.set_body(w.main, vec![], vec![], &span());
    assert!(is_internal_error_naming(
        body_twice,
        "ProgramBuilder::set_body"
    ));
}

/// REQ-LOW-ir-203: a POU that stands for a standard function block needs no
/// parts or body, and names the block.
#[test]
fn finish_when_pou_stands_for_standard_block_then_program_names_block() {
    let mut w = whole().complete();
    let ton = w.b.push_pou(name("TON"));
    w.b.declare_standard_block(ton, StandardFunctionBlock::Ton, &span())
        .unwrap();

    let program = w.finish().unwrap();

    assert_eq!(
        program.pous().standard_block(ton),
        Some(StandardFunctionBlock::Ton)
    );
    assert!(program.parts(ton).is_none());
    assert!(program.pous().body(ton).is_none());
}

/// REQ-LOW-ir-203: a POU is a standard function block or has parts and a
/// body, never both.
#[test]
fn declare_standard_block_when_pou_has_parts_or_block_then_internal_error() {
    let mut w = whole().complete();
    let with_parts =
        w.b.declare_standard_block(w.main, StandardFunctionBlock::Ton, &span());
    assert!(is_internal_error_naming(
        with_parts,
        "ProgramBuilder::declare_standard_block"
    ));

    let mut w = whole();
    let ton = w.b.push_pou(name("TON"));
    w.b.declare_standard_block(ton, StandardFunctionBlock::Ton, &span())
        .unwrap();
    let twice =
        w.b.declare_standard_block(ton, StandardFunctionBlock::Tof, &span());
    assert!(is_internal_error_naming(
        twice,
        "ProgramBuilder::declare_standard_block"
    ));
    let body = w.b.set_body(ton, vec![], vec![], &span());
    assert!(is_internal_error_naming(body, "ProgramBuilder::set_body"));
}

/// REQ-LOW-lowering-195, 201: a POU's body runs its prologue and then its own
/// statements, and the program tells the two apart.
#[test]
fn set_body_when_prologue_and_statements_then_body_runs_both_in_order() {
    let mut w = whole();
    let parts = PouParts {
        kind: PouKind::Program,
        instance: Some(w.this),
        locals: vec![],
        declarations: vec![],
        result: None,
    };
    w.b.declare_parts(w.main, parts, &span()).unwrap();
    let g = w.b.var(w.g, &span()).unwrap();
    let first = w.b.assign(g.clone(), int(1), span()).unwrap();
    let second = w.b.assign(g, int(2), span()).unwrap();
    w.b.set_body(w.main, vec![first], vec![second], &span())
        .unwrap();
    let main = w.main;

    let program = w.finish().unwrap();

    assert_eq!(program.pous().body(main).map(<[Stmt]>::len), Some(2));
    assert_eq!(program.prologue(main).map(<[Stmt]>::len), Some(1));
    assert_eq!(program.statements(main).map(<[Stmt]>::len), Some(1));
}

/// REQ-LOW-lowering-197: the variable that holds a POU's result is one the
/// POU owns.
#[test]
fn declare_parts_when_result_of_another_pou_then_internal_error() {
    let mut w = whole();
    let parts = PouParts {
        kind: PouKind::Program,
        instance: Some(w.this),
        locals: vec![],
        declarations: vec![],
        result: Some(w.g),
    };
    let result = w.b.declare_parts(w.main, parts, &span());
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::declare_parts"
    ));
}
