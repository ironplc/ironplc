//! A whole program: every reachable POU with its declarations and its body,
//! lowered in one walk (`specs/design/lowered-program.md`, "Program, POUs
//! and variables").
//!
//! [`Lowering::lower_program`] declares everything before it lowers any
//! statement:
//!
//! 1. the POU of each reachable program, function and function block, in
//!    library order, each method after its block (REQ-LOW-lowering-025);
//! 2. the variable of each global, in the execution model's order, the
//!    compiler-provided ones first;
//! 3. each POU's frame, every declaration that is no field of its instance,
//!    in declaration order, and a function's or method's signature
//!    (REQ-LOW-lowering-193);
//! 4. one variable for each program instance, of section `Instance`.
//!
//! Then it lowers `init` (`init.rs`), the stores of each global's and each
//! instance field's starting value.
//!
//! A statement that reaches a variable then finds the one already declared.
//! Then it lowers each top-level statement of each body, as
//! [`Lowering::lower_statement`] does.
//!
//! What it makes is a [`PartialProgram`]: lowering does not express every
//! declaration and statement yet, and codegen compiles one it does not from
//! the AST. [`PartialProgram::into_program`] gives the whole program once
//! every one is lowered.

use ironplc_dsl::common::{FunctionBlockBodyKind, LibraryElementKind, VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::StmtKind;
use ironplc_ir::execution::{DebugName, ExecutionModel, InstanceOf};
use ironplc_ir::lowered::{
    Declared, ExecutionBindings, Owner, PouId, PouKind, PouParts, PouTable, Program,
    ProgramBuilder, Section, Stmt, TypeTable, VarId, VariableTable,
};

use crate::init::Init;
use crate::variables::BodyFrame;
use crate::{Body, Lowering, StatementLowering};

/// What one walk of [`Lowering::lower_program`] made: the program's tables,
/// what each top-level statement of each body lowered to, and the execution
/// model bound to the program's POUs and variables.
#[derive(Clone, Debug)]
pub struct PartialProgram {
    builder: ProgramBuilder,
    execution: ExecutionModel,
    bindings: ExecutionBindings,
    bodies: Vec<LoweredBody>,
    /// Where a declaration is that lowering cannot express yet.
    incomplete: Vec<SourceSpan>,
    init: Init,
}

/// One body of the program and what its statements lowered to.
#[derive(Clone, Debug)]
struct LoweredBody {
    pou: PouId,
    kind: PouKind,
    /// The span of the POU's name.
    span: SourceSpan,
    /// The body's instance parameter; `None` for a function's.
    instance: Option<VarId>,
    statements: Statements,
}

/// What the statements of one body lowered to.
#[derive(Clone, Debug)]
enum Statements {
    /// Each top-level statement, with its span, by index.
    Lowered(Vec<(SourceSpan, StatementLowering)>),
    /// An SFC body, which lowering does not express yet.
    Sfc,
    /// The body of a POU whose declarations do not lower, which
    /// [`PartialProgram::incomplete`] records.
    Undeclared,
}

impl PartialProgram {
    /// What the top-level statement at `index` of the body of `pou` lowered
    /// to; `None` when the body has no such statement, or none lowered.
    pub fn statement(&self, pou: PouId, index: usize) -> Option<&StatementLowering> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        match &body.statements {
            Statements::Lowered(statements) => statements.get(index).map(|(_, lowered)| lowered),
            Statements::Sfc | Statements::Undeclared => None,
        }
    }

    /// The statements of `init`, which give every global and every field of
    /// every program instance the value it starts with; `None` when one of
    /// those values does not lower yet.
    pub fn init(&self) -> Option<&[Stmt]> {
        self.init
            .unsupported
            .is_empty()
            .then_some(self.init.statements.as_slice())
    }

    /// Whether every declaration of every reachable POU, and every global,
    /// lowered.
    pub fn declarations_complete(&self) -> bool {
        self.incomplete.is_empty()
    }

    /// The types the program refers to.
    pub fn types(&self) -> &TypeTable {
        self.builder.types()
    }

    /// The variables the program refers to.
    pub fn variables(&self) -> &VariableTable {
        self.builder.variables()
    }

    /// The POUs the program refers to.
    pub fn pous(&self) -> &PouTable {
        self.builder.pous()
    }

    /// The whole program; otherwise one P9999 at each declaration, body,
    /// statement and starting value lowering does not express yet, and at
    /// each POU that has no body in the library, a standard function
    /// block's, so that every one is reported (REQ-LOW-lowering-110, 111,
    /// 112).
    pub fn into_program(self) -> Result<Program, Vec<Diagnostic>> {
        let mut unsupported: Vec<Diagnostic> = self
            .incomplete
            .iter()
            .chain(&self.init.unsupported)
            .map(|span| Diagnostic::todo_with_span(span.clone()))
            .collect();
        for body in &self.bodies {
            match &body.statements {
                Statements::Lowered(statements) => {
                    for (span, lowered) in statements {
                        match lowered {
                            StatementLowering::Lowered(_) => {}
                            StatementLowering::Unsupported => {
                                unsupported.push(Diagnostic::todo_with_span(span.clone()))
                            }
                        }
                    }
                }
                Statements::Sfc => unsupported.push(Diagnostic::todo_with_span(body.span.clone())),
                Statements::Undeclared => {}
            }
        }
        let pous = self.builder.pous();
        for pou in pous.ids() {
            if !self.bodies.iter().any(|body| body.pou == pou) {
                let span = pous.name(pou).map(|name| name.span()).unwrap_or_default();
                unsupported.push(Diagnostic::todo_with_span(span));
            }
        }
        if !unsupported.is_empty() {
            return Err(unsupported);
        }
        let mut builder = self.builder;
        for stmt in self.init.statements {
            builder.push_init(stmt);
        }
        for body in self.bodies {
            let parts = PouParts {
                kind: body.kind,
                instance: body.instance,
                locals: locals(&builder, body.pou, body.instance),
            };
            builder
                .declare_parts(body.pou, parts, &body.span)
                .map_err(|diagnostic| vec![diagnostic])?;
            let statements = match body.statements {
                Statements::Lowered(statements) => statements,
                Statements::Sfc | Statements::Undeclared => Vec::new(),
            };
            let mut lowered = Vec::new();
            for (_, statement) in statements {
                match statement {
                    StatementLowering::Lowered(statements) => lowered.extend(statements),
                    StatementLowering::Unsupported => {}
                }
            }
            builder
                .set_body(body.pou, lowered, &body.span)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        builder
            .finish(self.execution, self.bindings)
            .map_err(|diagnostic| vec![diagnostic])
    }
}

/// The locals of the POU `pou`: every variable it owns, in the order they
/// were declared, other than its instance parameter `instance` and its
/// signature's parameters and result.
fn locals(builder: &ProgramBuilder, pou: PouId, instance: Option<VarId>) -> Vec<VarId> {
    let signature = builder.pous().function(pou);
    let in_signature = |var: VarId| {
        signature.is_some_and(|signature| {
            signature.parameters().contains(&var)
                || signature.result() == Some(var)
                || signature.result_parameter() == Some(var)
        })
    };
    builder
        .variables()
        .iter()
        .filter(|(var, variable)| {
            variable.owner() == Owner::Pou(pou) && Some(*var) != instance && !in_signature(*var)
        })
        .map(|(var, _)| var)
        .collect()
}

impl<'a> Lowering<'a> {
    /// Lowers every reachable POU of the library, with its declarations and
    /// its body, the globals and the program instances, in one walk.
    ///
    /// Returns a P9998 when the analyzed library breaks an invariant
    /// lowering relies on. A declaration or statement lowering does not
    /// express yet is no error: [`PartialProgram`] records it.
    pub fn lower_program(mut self) -> Result<PartialProgram, Vec<Diagnostic>> {
        self.walk().map_err(|diagnostic| vec![diagnostic])
    }

    fn walk(&mut self) -> Result<PartialProgram, Diagnostic> {
        let bodies = self.reachable_bodies();
        for body in &bodies {
            self.body_pou(*body);
        }
        let execution = self.analysis.execution_model().clone();
        let mut bindings = ExecutionBindings::new();
        let mut incomplete = Vec::new();
        for (id, global) in execution.globals() {
            let name = Id::from(&global.name.to_string());
            match self.global_place(&name, &global.name.span())? {
                Some(place) => bindings.bind_global(id, place.root()),
                None => incomplete.push(global.name.span()),
            }
        }
        let mut frames = Vec::with_capacity(bodies.len());
        for body in &bodies {
            let frame = self.declare_body(*body)?;
            if frame.is_none() {
                incomplete.push(body_name(*body).span());
            }
            frames.push(frame);
        }
        for (id, _) in execution.programs() {
            if let Some(program) = self.analysis.program_declaration(id) {
                bindings.bind_program(id, self.body_pou(Body::Program(program)));
            }
        }
        for (position, instance) in execution.instances().enumerate() {
            let InstanceOf::Program(id) = &instance.program else {
                continue;
            };
            let Some(program) = self.analysis.program_declaration(*id) else {
                continue;
            };
            let pou = self.body_pou(Body::Program(program));
            let Some(Some(frame)) = self.bodies.get(&pou) else {
                continue;
            };
            let Some(instance_type) = frame.instance.as_ref().map(|this| this.instance_type) else {
                continue;
            };
            let name = instance
                .name
                .clone()
                .unwrap_or_else(|| DebugName::new(program.name.to_string(), program.name.span()));
            let var = self.program.push_declared_variable(
                name,
                Owner::Global,
                Declared::in_section(Section::Instance),
                instance_type,
            )?;
            bindings.bind_instance(position, var);
        }
        let init = self.lower_init(&execution, &bindings)?;
        let mut lowered = Vec::with_capacity(bodies.len());
        for (body, frame) in bodies.into_iter().zip(frames) {
            lowered.push(self.lower_body(body, frame)?);
        }
        Ok(PartialProgram {
            builder: std::mem::replace(&mut self.program, ProgramBuilder::new()),
            execution,
            bindings,
            bodies: lowered,
            incomplete,
            init,
        })
    }

    /// The bodies of the reachable programs, functions and function blocks,
    /// in library order, each method of a block after it.
    fn reachable_bodies(&self) -> Vec<Body<'a>> {
        let reachable = self.analysis.context().reachable();
        let mut bodies = Vec::new();
        for element in &self.analysis.library().elements {
            match element {
                LibraryElementKind::ProgramDeclaration(program)
                    if reachable.contains(&program.name) =>
                {
                    bodies.push(Body::Program(program));
                }
                LibraryElementKind::FunctionDeclaration(function)
                    if reachable.contains(&function.name) =>
                {
                    bodies.push(Body::Function(function));
                }
                LibraryElementKind::FunctionBlockDeclaration(block)
                    if reachable.contains(&block.name.name) =>
                {
                    bodies.push(Body::FunctionBlock(block));
                    for method in &block.methods {
                        bodies.push(Body::Method { block, method });
                    }
                }
                LibraryElementKind::DataTypeDeclaration(_)
                | LibraryElementKind::FunctionDeclaration(_)
                | LibraryElementKind::FunctionBlockDeclaration(_)
                | LibraryElementKind::ProgramDeclaration(_)
                | LibraryElementKind::ConfigurationDeclaration(_)
                | LibraryElementKind::GlobalVarDeclarations(_)
                | LibraryElementKind::InterfaceDeclaration(_) => {}
            }
        }
        bodies
    }

    /// The POU of `body`.
    pub(crate) fn body_pou(&mut self, body: Body<'a>) -> PouId {
        match body {
            Body::Program(program) => self.pou(&program.name.to_string(), &program.name.span()),
            Body::FunctionBlock(block) => {
                self.pou(&block.name.name.to_string(), &block.name.name.span())
            }
            Body::Function(function) => self.pou(&function.name.to_string(), &function.name.span()),
            Body::Method { block, method } => self.method_pou(&block.name.name, &method.name),
        }
    }

    /// Declares the frame of `body`, each of its declarations that is no
    /// field of its instance, in declaration order, and, for a function or
    /// method, its signature. `None` when one of them is not one lowering
    /// expresses yet.
    fn declare_body(&mut self, body: Body<'a>) -> Result<Option<BodyFrame>, Diagnostic> {
        let Some(frame) = self.frame(body)? else {
            return Ok(None);
        };
        for decl in declarations(body) {
            if !is_own_variable(body, decl) {
                continue;
            }
            // A located variable with no name, which no statement names.
            let Some(name) = decl.identifier.symbolic_id() else {
                continue;
            };
            if self.named_place(&frame, name, &name.span())?.is_none() {
                return Ok(None);
            }
        }
        match body {
            Body::Function(_) | Body::Method { .. } => {
                if self.signature_pou(body, &body_name(body).span())?.is_none() {
                    return Ok(None);
                }
            }
            Body::Program(_) | Body::FunctionBlock(_) => {}
        }
        Ok(Some(frame))
    }

    /// Lowers each top-level statement of `body`, whose frame is `frame`.
    fn lower_body(
        &mut self,
        body: Body<'a>,
        frame: Option<BodyFrame>,
    ) -> Result<LoweredBody, Diagnostic> {
        let span = body_name(body).span();
        let kind = match body {
            Body::Program(_) => PouKind::Program,
            Body::FunctionBlock(_) => PouKind::FunctionBlock,
            Body::Function(_) => PouKind::Function,
            Body::Method { .. } => PouKind::Method,
        };
        let pou = self.body_pou(body);
        let unsupported = |statements| LoweredBody {
            pou,
            kind,
            span: span.clone(),
            instance: None,
            statements,
        };
        let Some(frame) = frame else {
            return Ok(unsupported(Statements::Undeclared));
        };
        let Some(source) = statements_of(body) else {
            return Ok(unsupported(Statements::Sfc));
        };
        let mut statements = Vec::with_capacity(source.len());
        for stmt in source {
            let lowered = match self.statement(&frame, stmt)? {
                Some(lowered) => StatementLowering::Lowered(lowered),
                None => StatementLowering::Unsupported,
            };
            statements.push((statement_span(stmt, &span), lowered));
        }
        Ok(LoweredBody {
            pou,
            kind,
            span,
            instance: frame.instance.map(|instance| instance.this),
            statements: Statements::Lowered(statements),
        })
    }
}

/// The name `body` declares.
fn body_name<'a>(body: Body<'a>) -> &'a Id {
    match body {
        Body::Program(program) => &program.name,
        Body::FunctionBlock(block) => &block.name.name,
        Body::Function(function) => &function.name,
        Body::Method { method, .. } => &method.name,
    }
}

/// The declarations of `body`, in the order it declares them.
fn declarations<'a>(body: Body<'a>) -> &'a [VarDecl] {
    match body {
        Body::Program(program) => &program.variables,
        Body::FunctionBlock(block) => &block.variables,
        Body::Function(function) => &function.variables,
        Body::Method { method, .. } => &method.variables,
    }
}

/// Whether `decl`, a declaration of `body`, is a variable of its own or a
/// global's, rather than a field of the body's instance, which the
/// instance's type declares, or an access path, which names another
/// variable.
fn is_own_variable(body: Body<'_>, decl: &VarDecl) -> bool {
    match (body, &decl.var_type) {
        (_, VariableType::Access) => false,
        (
            Body::Program(_) | Body::FunctionBlock(_),
            VariableType::Var | VariableType::Input | VariableType::Output | VariableType::InOut,
        ) => false,
        (
            Body::Program(_) | Body::FunctionBlock(_),
            VariableType::VarTemp | VariableType::External | VariableType::Global,
        ) => true,
        (
            Body::Function(_) | Body::Method { .. },
            VariableType::Var
            | VariableType::Input
            | VariableType::Output
            | VariableType::InOut
            | VariableType::VarTemp
            | VariableType::External
            | VariableType::Global,
        ) => true,
    }
}

/// The top-level statements of `body`; `None` for an SFC body, which
/// lowering does not express yet.
fn statements_of<'a>(body: Body<'a>) -> Option<&'a [StmtKind]> {
    let block_body = |kind: &'a FunctionBlockBodyKind| match kind {
        FunctionBlockBodyKind::Statements(statements) => Some(statements.body.as_slice()),
        FunctionBlockBodyKind::Empty => Some(&[][..]),
        FunctionBlockBodyKind::Sfc(_) => None,
    };
    match body {
        Body::Program(program) => block_body(&program.body),
        Body::FunctionBlock(block) => block_body(&block.body),
        Body::Function(function) => Some(&function.body),
        Body::Method { method, .. } => Some(&method.body),
    }
}

/// Where `stmt` is, or, for a statement the AST keeps no span for, the
/// span of the POU it is in, `pou`.
fn statement_span(stmt: &StmtKind, pou: &SourceSpan) -> SourceSpan {
    let span = stmt.span();
    if span == SourceSpan::default() {
        pou.clone()
    } else {
        span
    }
}
