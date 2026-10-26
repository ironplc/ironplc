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
//! It keeps what each declaration of each body declares, in declaration
//! order, and which variables the declarations made, for a backend to lay
//! out its storage from (REQ-LOW-lowering-197).
//!
//! Then it lowers `init` (`init.rs`), the stores of each global's and each
//! instance field's starting value, and, at the head of each body, its
//! prologue.
//!
//! Then it lowers each top-level statement of each body. A statement that
//! reaches a variable finds the one already declared.
//!
//! What it makes is a [`PartialProgram`]: lowering does not express every
//! declaration and statement yet. Each boundary of the walk, a global, a
//! body's declarations, a starting value and a top-level statement, records
//! a construct lowering does not express yet as a P9999 there and goes on
//! (REQ-LOW-lowering-110, REQ-LOW-lowering-111); a broken invariant is a
//! P9998 that ends the walk. [`PartialProgram::into_program`] gives the
//! whole program once every one is lowered, and otherwise every P9999.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::common::TypeName;
use ironplc_dsl::common::{FunctionBlockBodyKind, LibraryElementKind, VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::StmtKind;
use ironplc_ir::execution::{DebugName, ExecutionModel, InstanceOf};
use ironplc_ir::lowered::{
    Declaration, Declared, ExecutionBindings, Owner, PouId, PouKind, PouParts, PouTable, Program,
    ProgramBuilder, Projection, Section, StandardFunctionBlock, Stmt, TypeTable, VarId,
    VariableTable,
};

use crate::execution_model;
use crate::init::{Stores, Values};
use crate::variables::BodyFrame;
use crate::{Body, Lowered, Lowering, Unlowered};

/// What one walk of [`Lowering::lower_program`] made: the program's tables,
/// what each top-level statement of each body lowered to, and the execution
/// model bound to the program's POUs and variables.
#[derive(Clone, Debug)]
pub struct PartialProgram {
    builder: ProgramBuilder,
    execution: ExecutionModel,
    bindings: ExecutionBindings,
    bodies: Vec<LoweredBody>,
    /// A P9999 at each global and each body whose declarations lowering
    /// cannot express yet.
    incomplete: Vec<Diagnostic>,
    /// The variables a declaration declares, made before any temporary.
    declared: HashSet<VarId>,
    /// The variable of each method whose result is an array or a structure
    /// that holds the result.
    aggregate_results: HashMap<PouId, VarId>,
    /// The standard function block each POU with no body in the library
    /// stands for, which lowering does not generate yet (issue 2288).
    standard_blocks: HashMap<PouId, StandardFunctionBlock>,
    init: Stores,
}

/// One body of the program and what its statements lowered to.
#[derive(Clone, Debug)]
struct LoweredBody {
    pou: PouId,
    kind: PouKind,
    /// Its declarations, in the order it declares them.
    declarations: Vec<Declaration>,
    /// The span of the POU's name.
    span: SourceSpan,
    /// The body's instance parameter; `None` for a function's.
    instance: Option<VarId>,
    /// The statements at the head of the body, before its own.
    prologue: Stores,
    statements: Statements,
}

/// What the statements of one body lowered to.
#[derive(Clone, Debug)]
enum Statements {
    /// Each top-level statement, with its span, by index: the statements it
    /// runs, or the P9999 at the statement when lowering does not express
    /// it yet.
    Lowered(Vec<(SourceSpan, Result<Vec<Stmt>, Diagnostic>)>),
    /// An SFC body, which lowering does not express yet: the P9999 at the
    /// body.
    Sfc(Diagnostic),
    /// The body of a POU whose declarations do not lower, which
    /// [`PartialProgram::incomplete`] records.
    Undeclared,
}

impl PartialProgram {
    /// What the top-level statement at `index` of the body of `pou` lowered
    /// to: the statements it runs, or the P9999 at it; `None` when the body
    /// has no such statement, or none lowered.
    pub fn statement(&self, pou: PouId, index: usize) -> Option<Result<&[Stmt], &Diagnostic>> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        match &body.statements {
            Statements::Lowered(statements) => statements
                .get(index)
                .map(|(_, lowered)| lowered.as_ref().map(Vec::as_slice)),
            Statements::Sfc(_) | Statements::Undeclared => None,
        }
    }

    /// Where the top-level statement at `index` of the body of `pou` is in
    /// the source: an empty span for a statement the AST keeps no span for,
    /// a `RETURN`'s; `None` when the body has no such statement.
    pub fn statement_span(&self, pou: PouId, index: usize) -> Option<&SourceSpan> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        match &body.statements {
            Statements::Lowered(statements) => statements.get(index).map(|(span, _)| span),
            Statements::Sfc(_) | Statements::Undeclared => None,
        }
    }

    /// The statements of `init`, which give every global and every field of
    /// every program instance the value it starts with; `None` when one of
    /// those values does not lower yet.
    pub fn init(&self) -> Option<&[Stmt]> {
        self.init.complete()
    }

    /// The statements at the head of the body of `pou`, which give each of
    /// its variables that starts again on every call its value: a
    /// function's or method's locals and result, and any body's `VAR_TEMP`;
    /// `None` when one of those values does not lower yet, or the body does
    /// not.
    pub fn prologue(&self, pou: PouId) -> Option<&[Stmt]> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        match body.statements {
            Statements::Lowered(_) => body.prologue.complete(),
            Statements::Sfc(_) | Statements::Undeclared => None,
        }
    }

    /// Whether every declaration of every reachable POU, and every global,
    /// lowered.
    pub fn declarations_complete(&self) -> bool {
        self.incomplete.is_empty()
    }

    /// Whether a declaration declares `var`, or a program instance is it,
    /// rather than lowering making it for a statement or a starting value.
    pub fn is_declared(&self, var: VarId) -> bool {
        self.declared.contains(&var)
    }

    /// The declarations of the body of `pou`, in the order it declares
    /// them; `None` for a POU with no body in the library, and empty for a
    /// body whose declarations did not lower.
    pub fn declarations(&self, pou: PouId) -> Option<&[Declaration]> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        Some(&body.declarations)
    }

    /// The variable that holds the result of the function or method `pou`:
    /// the one its signature returns, its result parameter, or, for a
    /// method whose result is an array or a structure, the variable of its
    /// body no call reads (REQ-LOW-lowering-187). `None` for a method that
    /// returns none.
    pub fn result(&self, pou: PouId) -> Option<VarId> {
        let signature = self.pous().function(pou)?;
        signature
            .result()
            .or(signature.result_parameter())
            .or_else(|| self.aggregate_results.get(&pou).copied())
    }

    /// The execution model the program runs under.
    pub fn execution(&self) -> &ExecutionModel {
        &self.execution
    }

    /// The execution model bound to the program's POUs and variables.
    pub fn bindings(&self) -> &ExecutionBindings {
        &self.bindings
    }

    /// What the declaration of the body of `pou` gives it: what it is, its
    /// instance parameter, and its locals; `None` for a POU with no body in
    /// the library, a standard function block's. Its locals include the
    /// temporaries lowering makes for its statements.
    pub fn parts(&self, pou: PouId) -> Option<PouParts> {
        let body = self.bodies.iter().find(|body| body.pou == pou)?;
        Some(self.parts_of(body))
    }

    /// The parts of the POU of `body`.
    fn parts_of(&self, body: &LoweredBody) -> PouParts {
        PouParts {
            kind: body.kind,
            instance: body.instance,
            locals: locals(&self.builder, body.pou, body.instance),
            declarations: body.declarations.clone(),
            result: self.result(body.pou),
        }
    }

    /// The standard function block the POU `pou`, which has no body in the
    /// library, stands for; `None` for any other POU.
    pub fn standard_block(&self, pou: PouId) -> Option<StandardFunctionBlock> {
        self.standard_blocks.get(&pou).copied()
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

    /// The whole program; otherwise the P9999 at each declaration, body,
    /// statement and starting value lowering does not express yet, so that
    /// every one is reported (REQ-LOW-lowering-110, 111, 112). A POU with no
    /// body in the library stands for the standard function block of its
    /// name (REQ-LOW-ir-203).
    pub fn into_program(self) -> Result<Program, Vec<Diagnostic>> {
        let mut unsupported: Vec<Diagnostic> = self
            .incomplete
            .iter()
            .chain(&self.init.unsupported)
            .chain(
                self.bodies
                    .iter()
                    .flat_map(|body| &body.prologue.unsupported),
            )
            .cloned()
            .collect();
        for body in &self.bodies {
            match &body.statements {
                Statements::Lowered(statements) => unsupported.extend(
                    statements
                        .iter()
                        .filter_map(|(_, lowered)| lowered.as_ref().err().cloned()),
                ),
                Statements::Sfc(diagnostic) => unsupported.push(diagnostic.clone()),
                Statements::Undeclared => {}
            }
        }
        let pous = self.builder.pous();
        for pou in pous.ids() {
            let declared = self.bodies.iter().any(|body| body.pou == pou)
                || self.standard_blocks.contains_key(&pou);
            if !declared {
                let span = pous.name(pou).map(|name| name.span()).unwrap_or_default();
                unsupported.push(Diagnostic::todo_with_span(span));
            }
        }
        if !unsupported.is_empty() {
            return Err(unsupported);
        }
        let all_parts: Vec<PouParts> = self.bodies.iter().map(|body| self.parts_of(body)).collect();
        let mut builder = self.builder;
        for stmt in self.init.statements {
            builder.push_init(stmt);
        }
        for (pou, block) in self.standard_blocks {
            let span = builder
                .pous()
                .name(pou)
                .map(|name| name.span())
                .unwrap_or_default();
            builder
                .declare_standard_block(pou, block, &span)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        for (body, parts) in self.bodies.into_iter().zip(all_parts) {
            builder
                .declare_parts(body.pou, parts, &body.span)
                .map_err(|diagnostic| vec![diagnostic])?;
            let statements = match body.statements {
                Statements::Lowered(statements) => statements,
                Statements::Sfc(_) | Statements::Undeclared => Vec::new(),
            };
            let lowered: Vec<Stmt> = statements
                .into_iter()
                .filter_map(|(_, lowered)| lowered.ok())
                .flatten()
                .collect();
            builder
                .set_body(body.pou, body.prologue.statements, lowered, &body.span)
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
        let (execution, declarations) = execution_model::resolve_with_declarations(
            self.analysis.library(),
            self.analysis.context().compiler_options(),
        );
        let mut bindings = ExecutionBindings::new();
        let mut incomplete = Vec::new();
        for (id, global) in execution.globals() {
            let name = Id::from(&global.name.to_string());
            match self.global_place(&name, &global.name.span()) {
                Ok(place) => bindings.bind_global(id, place.root()),
                Err(unlowered) => incomplete.push(unlowered.not_yet_at(global.name.span())?),
            }
        }
        let mut frames = Vec::with_capacity(bodies.len());
        for body in &bodies {
            let frame = match self.declare_body(*body) {
                Ok(frame) => Some(frame),
                Err(unlowered) => {
                    incomplete.push(unlowered.not_yet_at(body_name(*body).span())?);
                    None
                }
            };
            frames.push(frame);
        }
        for (id, _) in execution.programs() {
            if let Some(program) = declarations.program(id) {
                bindings.bind_program(id, self.body_pou(Body::Program(program)));
            }
        }
        for (position, instance) in execution.instances().enumerate() {
            let InstanceOf::Program(id) = &instance.program else {
                continue;
            };
            let Some(program) = declarations.program(*id) else {
                continue;
            };
            let pou = self.body_pou(Body::Program(program));
            let Some(Ok(frame)) = self.bodies.get(&pou) else {
                continue;
            };
            let Some(instance_type) = frame.instance.as_ref().map(|instance| instance.root.ty())
            else {
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
        let declared = self
            .program
            .variables()
            .iter()
            .map(|(var, _)| var)
            .collect();
        let values = Values::of(self.analysis.context().types());
        let init = self.lower_init(&execution, &declarations, &bindings, &values)?;
        let mut lowered = Vec::with_capacity(bodies.len());
        for (body, frame) in bodies.into_iter().zip(frames) {
            lowered.push(self.lower_body(body, frame, &values)?);
        }
        let standard_blocks = self.standard_blocks(&lowered);
        Ok(PartialProgram {
            builder: std::mem::replace(&mut self.program, ProgramBuilder::new()),
            execution,
            bindings,
            bodies: lowered,
            incomplete,
            declared,
            aggregate_results: std::mem::take(&mut self.aggregate_results),
            standard_blocks,
            init,
        })
    }

    /// The standard function block each POU that is neither a method nor
    /// one of `bodies` stands for: the one the type of its name is, as the
    /// type environment records it. Lowering does not generate the standard
    /// blocks yet (issue 2288), so a call of one names a POU after the
    /// block, and its name is what matches it to its block, here, once.
    fn standard_blocks(&self, bodies: &[LoweredBody]) -> HashMap<PouId, StandardFunctionBlock> {
        let types = self.analysis.context().types();
        let pous = self.program.pous();
        pous.ids()
            .filter(|pou| pous.method_of(*pou).is_none())
            .filter(|pou| !bodies.iter().any(|body| body.pou == *pou))
            .filter_map(|pou| {
                let name = TypeName::from(&pous.name(pou)?.to_string());
                Some((pou, types.get(&name)?.standard_block?))
            })
            .collect()
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
                    if reachable.contains(&block.name.id) =>
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
            Body::FunctionBlock(block) => self.pou(&block.name.to_string(), &block.name.span()),
            Body::Function(function) => self.pou(&function.name.to_string(), &function.name.span()),
            Body::Method { block, method } => self.method_pou(&block.name.id, &method.name),
        }
    }

    /// Declares the frame of `body`, each of its declarations that is no
    /// field of its instance, in declaration order, and, for a function or
    /// method, its signature, and gives what each declaration declares. A
    /// body with a declaration of its own that lowering does not express
    /// yet is not declared.
    fn declare_body(&mut self, body: Body<'a>) -> Lowered<(BodyFrame<'static>, Vec<Declaration>)> {
        let frame = self.frame(body)?;
        let mut declared = Vec::new();
        for decl in declarations(body) {
            // A located variable with no name, which no statement names.
            let Some(name) = decl.identifier.symbolic_id() else {
                continue;
            };
            let own = is_own_variable(body, decl);
            let place = match self.named_place(&frame, name, &name.span()) {
                Ok(place) => place,
                // A field of the instance lowering does not name yet, such
                // as a `VAR_IN_OUT`, declares nothing of the body's own.
                Err(Unlowered::NotYet) if !own => continue,
                Err(unlowered) => return Err(unlowered),
            };
            let declaration = match place.path() {
                [] if decl.var_type == VariableType::External => {
                    Declaration::External(place.root())
                }
                // A `VAR_IN_OUT` or `VAR_OUTPUT` of a function or method is a
                // reference to its argument.
                [] | [Projection::Deref] => Declaration::Variable(place.root()),
                [Projection::Deref, Projection::Field(field)] => Declaration::Field(*field),
                // No declaration names a place deeper in its instance.
                _ => continue,
            };
            declared.push(declaration);
        }
        match body {
            Body::Function(_) | Body::Method { .. } => {
                self.signature_pou(body, &body_name(body).span())?;
            }
            Body::Program(_) | Body::FunctionBlock(_) => {}
        }
        Ok((frame, declared))
    }

    /// Lowers each top-level statement of `body`, whose frame is `frame`,
    /// or none when its declarations did not lower.
    fn lower_body(
        &mut self,
        body: Body<'a>,
        frame: Option<(BodyFrame<'static>, Vec<Declaration>)>,
        values: &Values,
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
            declarations: Vec::new(),
            span: span.clone(),
            instance: None,
            prologue: Stores::default(),
            statements,
        };
        let Some((frame, declarations)) = frame else {
            return Ok(unsupported(Statements::Undeclared));
        };
        let Some(source) = statements_of(body) else {
            let sfc = Diagnostic::todo_with_span(span.clone());
            return Ok(unsupported(Statements::Sfc(sfc)));
        };
        let prologue = self.lower_prologue(body, &frame, values)?;
        let mut statements = Vec::with_capacity(source.len());
        for stmt in source {
            let lowered = match self.statement(&frame, stmt) {
                Ok(lowered) => Ok(lowered),
                Err(unlowered) => Err(unlowered.not_yet_at(reported(&stmt.span(), &span))?),
            };
            statements.push((stmt.span(), lowered));
        }
        Ok(LoweredBody {
            pou,
            kind,
            declarations,
            span,
            instance: frame.instance.map(|instance| instance.this),
            prologue,
            statements: Statements::Lowered(statements),
        })
    }
}

/// The name `body` declares.
fn body_name<'a>(body: Body<'a>) -> &'a Id {
    match body {
        Body::Program(program) => &program.name,
        Body::FunctionBlock(block) => &block.name.id,
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

/// Where a problem with the statement at `span` is reported: there, or, for
/// a statement the AST keeps no span for, at the POU it is in, `pou`. A
/// `SourceSpan` equals any other, so an empty span is the one with none.
fn reported(span: &SourceSpan, pou: &SourceSpan) -> SourceSpan {
    if span.start == span.end {
        pou.clone()
    } else {
        span.clone()
    }
}
