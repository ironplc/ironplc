//! Variables, POUs and the bodies statements belong to.
//!
//! Every name a body uses becomes a place rooted at a variable lowering
//! allocated, never a name (REQ-LOW-lowering-021):
//!
//! * a variable of a program or function block other than a `VAR_TEMP` is a
//!   field of its instance, reached through the body's instance parameter:
//!   `x` in the body of `Main` is `this^.x` (REQ-LOW-lowering-034);
//! * a variable of a function, and a `VAR_TEMP` of any body, is a variable
//!   of its own;
//! * a `VAR_IN_OUT` or `VAR_OUTPUT` of a function is a variable of its own
//!   of a non-null reference type, and a use of it is what it refers to: `v`
//!   is `v^` (REQ-LOW-lowering-033, REQ-LOW-lowering-092);
//! * a method has the variables a function has, and reaches the fields of
//!   its function block through its instance parameter, as the block's body
//!   does;
//! * a global is a variable of its own, and a `VAR_EXTERNAL` names the
//!   global it refers to rather than a variable of its own
//!   (REQ-LOW-lowering-022).
//!
//! Each declaration gets one variable, by its [`DeclRef`], so two variables
//! of one name in different scopes are different variables
//! (REQ-LOW-lowering-020).

use ironplc_analyzer::symbol_environment::{
    DeclRef, Resolution, ScopeKind, ScopePath, SymbolInfo, SymbolKind,
};
use ironplc_dsl::common::{
    AddressAssignment, DeclarationQualifier, LocationPrefix, SizePrefix, TypeName, VarDecl,
    VariableIdentifier, VariableType,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    AddressArea, AddressSize, Declared, DirectAddress, Field, LoopScope, Nullability, Owner, Place,
    PouId, Retention, Section, TypeEntry, TypeId, TypeKind, VarId,
};

use crate::{Body, Lowered, Lowering, Unlowered};

/// What lowering knows of the body a statement belongs to, and what encloses
/// the statement in it: the loops it is in, and the call whose outputs it
/// stores. A frame enclosed by nothing, as a top-level statement's is, is a
/// `BodyFrame<'static>`; one inside a loop or a call borrows what encloses
/// it for as long as that is lowered.
#[derive(Clone, Debug)]
pub(crate) struct BodyFrame<'e> {
    pub(crate) pou: PouId,
    /// The scope the body's names are resolved from.
    pub(crate) scope: ScopeKind,
    /// The instance a program, function block or method body works on;
    /// `None` for a function.
    pub(crate) instance: Option<Instance>,
    /// The loops enclosing the statement, innermost last.
    loops: Vec<&'e LoopScope>,
    /// Where the call whose outputs the statement stores left them, which
    /// the statement's call outputs read.
    call_outputs: Option<&'e CallOutputs>,
}

/// Where a call leaves its outputs for the assignments of its outputs to
/// read.
#[derive(Clone, Debug)]
pub(crate) enum CallOutputs {
    /// In the fields of the function block instance it called.
    Instance(Place),
    /// In the temporary it passed for each output of a function or method,
    /// by the output's name (REQ-LOW-lowering-092).
    Temporaries(Vec<(Id, Place)>),
}

/// The instance a body works on: its instance parameter, and the place of
/// the instance itself, whose fields the body's names reach.
#[derive(Clone, Debug)]
pub(crate) struct Instance {
    /// The instance parameter, a non-null reference to the instance.
    pub(crate) this: VarId,
    /// The instance itself: what `this` refers to, or, in `init`, which runs
    /// in no body, the variable that holds the instance
    /// ([`BodyFrame::held_by`]).
    pub(crate) root: Place,
    /// The scope that declares the fields: the body's own for a program or
    /// function block, and its block's for a method.
    scope: ScopeKind,
}

impl<'e> BodyFrame<'e> {
    /// This frame inside the loop `scope`.
    pub(crate) fn in_loop<'i>(&self, scope: &'i LoopScope) -> BodyFrame<'i>
    where
        'e: 'i,
    {
        let mut inner: BodyFrame<'i> = self.clone();
        inner.loops.push(scope);
        inner
    }

    /// This frame while the outputs of the call that left them in `outputs`
    /// are stored.
    pub(crate) fn storing<'i>(&self, outputs: &'i CallOutputs) -> BodyFrame<'i>
    where
        'e: 'i,
    {
        let mut storing: BodyFrame<'i> = self.clone();
        storing.call_outputs = Some(outputs);
        storing
    }

    /// The innermost loop enclosing the statement; `None` outside every
    /// loop.
    pub(crate) fn innermost_loop(&self) -> Option<&'e LoopScope> {
        self.loops.last().copied()
    }

    /// Where the call whose outputs the statement stores left them; `None`
    /// for a statement that stores no call's outputs.
    pub(crate) fn call_outputs(&self) -> Option<&'e CallOutputs> {
        self.call_outputs
    }

    /// This frame with its instance at `holder`, the variable that holds
    /// the instance, rather than at what the body's instance parameter
    /// refers to: the frame an initializer of a field of a program instance
    /// is lowered in, as `init` runs in no body.
    pub(crate) fn held_by(&self, holder: Place) -> BodyFrame<'e> {
        let mut frame = self.clone();
        if let Some(instance) = frame.instance.as_mut() {
            instance.root = holder;
        }
        frame
    }
}

impl<'a> Lowering<'a> {
    /// The frame of `body`, made the first time a statement of it is
    /// lowered. A body whose instance lowering cannot express yet has none.
    pub(crate) fn frame(&mut self, body: Body<'a>) -> Lowered<BodyFrame<'static>> {
        let (pou, path) = match body {
            Body::Program(program) => (self.named_pou(&program.name), vec![program.name.clone()]),
            Body::FunctionBlock(block) => {
                (self.named_pou(&block.name.id), vec![block.name.id.clone()])
            }
            Body::Function(function) => {
                (self.named_pou(&function.name), vec![function.name.clone()])
            }
            Body::Method { block, method } => (
                self.method_pou(&block.name.id, &method.name),
                vec![block.name.id.clone(), method.name.clone()],
            ),
        };
        if let Some(frame) = self.bodies.get(&pou) {
            return frame.clone();
        }
        let scope = ScopeKind::Named(ScopePath::new(path));
        let instance = match body {
            // A program's `VAR_ACCESS` paths name other variables, and no
            // statement names one, so they are no fields of its instance.
            Body::Program(program) => self
                .program_instance(pou, &program.name, &program.variables)
                .map(Some),
            Body::FunctionBlock(block) => self
                .block_instance(pou, &block.name.id, scope.clone())
                .map(Some),
            Body::Function(_) => Ok(None),
            // A method works on an instance of its block, whose fields its
            // block's scope declares.
            Body::Method { block, .. } => {
                let fields = ScopeKind::Named(ScopePath::new(vec![block.name.id.clone()]));
                self.block_instance(pou, &block.name.id, fields).map(Some)
            }
        };
        let frame = instance.map(|instance| BodyFrame {
            pou,
            scope,
            instance,
            loops: Vec::new(),
            call_outputs: None,
        });
        self.bodies.insert(pou, frame.clone());
        frame
    }

    /// The POU named `name`, declared at `span`, allocated the first time it
    /// is asked for.
    pub(crate) fn pou(&mut self, name: &str, span: &SourceSpan) -> PouId {
        let key = name.to_lowercase();
        if let Some(pou) = self.pou_ids.get(&key) {
            return *pou;
        }
        let pou = self.program.push_pou(DebugName::new(name, span.clone()));
        self.pou_ids.insert(key, pou);
        pou
    }

    /// The POU `name` names.
    fn named_pou(&mut self, name: &Id) -> PouId {
        self.pou(&name.to_string(), &name.span())
    }

    /// The method `method` of the function block `block`, named after the
    /// method. A method's key is no identifier, so it is no other POU's.
    pub(crate) fn method_pou(&mut self, block: &Id, method: &Id) -> PouId {
        let key = format!("{block}.{method}").to_lowercase();
        if let Some(pou) = self.pou_ids.get(&key) {
            return *pou;
        }
        let block = self.named_pou(block);
        let name = DebugName::new(method.to_string(), method.span());
        let pou = self.program.push_method_pou(name, block);
        self.pou_ids.insert(key, pou);
        pou
    }

    /// The instance of a program: a type with a field for each variable the
    /// program declares ([`Self::instance_fields`]).
    fn program_instance(
        &mut self,
        pou: PouId,
        name: &Id,
        variables: &[VarDecl],
    ) -> Lowered<Instance> {
        let fields = self.instance_fields(variables)?;
        let program = self.program.push_type(
            TypeEntry {
                name: Some(DebugName::new(name.to_string(), name.span())),
                kind: TypeKind::Program { fields, body: pou },
            },
            &name.span(),
        )?;
        let scope = ScopeKind::Named(ScopePath::new(vec![name.clone()]));
        Ok(self.instance(pou, name, program, scope)?)
    }

    /// The instance of a function block, the body of `pou` works on: the
    /// block's own type, whose fields `scope` declares.
    fn block_instance(&mut self, pou: PouId, name: &Id, scope: ScopeKind) -> Lowered<Instance> {
        let types = self.analysis.context().types();
        let id = types
            .id_of(&TypeName::from(&name.to_string()))
            .ok_or(Unlowered::NotYet)?;
        let block = self.lowered_type(id)?;
        Ok(self.instance(pou, name, block, scope)?)
    }

    /// The fields of the instance of a program that declares `variables`:
    /// one for each variable, in the order they are declared, as a function
    /// block's are (see `block_fields` in `types.rs`), other than a
    /// `VAR_TEMP`, a `VAR_EXTERNAL`, a global variable or an access path,
    /// which a PLCopen program declares in its interface and statements
    /// reach in global scope, and a located variable with no name, which no
    /// statement names (REQ-LOW-lowering-170).
    fn instance_fields(&mut self, variables: &[VarDecl]) -> Lowered<Vec<Field>> {
        let mut fields = Vec::new();
        for decl in variables {
            let section = match decl.var_type {
                VariableType::VarTemp
                | VariableType::External
                | VariableType::Global
                | VariableType::Access => continue,
                VariableType::Var => Section::Local,
                VariableType::Input => Section::Input,
                VariableType::Output => Section::Output,
                VariableType::InOut => Section::InOut,
            };
            let Some(field_name) = decl.identifier.symbolic_id() else {
                continue;
            };
            let type_id = decl.type_id.ok_or(Unlowered::NotYet)?;
            let ty = self.lowered_type(type_id)?;
            let location = match &decl.identifier {
                VariableIdentifier::Symbol(_) => None,
                VariableIdentifier::Direct(direct) => {
                    Some(direct_address(&direct.address_assignment))
                }
            };
            let field = Field::new(
                &field_name.to_string(),
                field_name.span(),
                Some(section),
                ty,
            );
            fields.push(field.declared(location, retention(&decl.qualifier)));
        }
        Ok(fields)
    }

    /// The instance of the body of `pou`, of type `instance_type`, whose
    /// fields `scope` declares, reached through the body's instance
    /// parameter, a non-null reference to it.
    fn instance(
        &mut self,
        pou: PouId,
        name: &Id,
        instance_type: TypeId,
        scope: ScopeKind,
    ) -> Result<Instance, Diagnostic> {
        let span = name.span();
        let this_type = self.non_null_reference(instance_type, &span)?;
        let this = self.program.push_declared_variable(
            DebugName::new("THIS", span.clone()),
            Owner::Pou(pou),
            Declared::in_section(Section::Instance),
            this_type,
        )?;
        let root = self.program.deref(self.program.var(this, &span)?, &span)?;
        Ok(Instance { this, root, scope })
    }

    /// The place `name` refers to in the body of `frame`. A name that is no
    /// variable the body reaches, and a variable lowering cannot reach yet,
    /// is not lowered.
    pub(crate) fn named_place(
        &mut self,
        frame: &BodyFrame,
        name: &Id,
        span: &SourceSpan,
    ) -> Lowered<Place> {
        let symbols = self.analysis.context().symbols();
        let (decl, info) = match symbols.resolve_variable(name, &frame.scope) {
            Resolution::Variable { decl, info } => (decl, info),
            Resolution::GlobalNotVisible { .. }
            | Resolution::NotAVariable
            | Resolution::Undeclared => return Err(Unlowered::NotYet),
        };
        // A global variable, or one a PLCopen program declares global in its
        // interface, which is no field of the program's instance.
        if decl.scope == ScopeKind::Global || info.variable_type == Some(VariableType::Global) {
            return self.global_variable(decl, info, span);
        }
        if let Some(instance) = frame
            .instance
            .as_ref()
            .filter(|instance| instance.scope == decl.scope)
        {
            match info.variable_type {
                // A `VAR_TEMP` of a program or function block is a variable
                // of its body; one of a block, named in a method, is not
                // lowered yet.
                Some(VariableType::VarTemp) if decl.scope == frame.scope => {
                    let owner = Owner::Pou(frame.pou);
                    return self.declared_variable(decl, info, owner, false, span);
                }
                // A `VAR_IN_OUT` of a program or function block is a field
                // its call binds, which lowering does not express yet.
                Some(VariableType::VarTemp | VariableType::InOut) => return Err(Unlowered::NotYet),
                Some(_) | None => {}
            }
            let root = instance.root.clone();
            return self.field_of(root, &decl.name, span);
        }
        if decl.scope != frame.scope {
            // A field a function block inherits through `EXTENDS` is not
            // lowered yet.
            return Err(Unlowered::NotYet);
        }
        // A variable of a function or method. Its `VAR_IN_OUT` is a non-null
        // reference to the caller's variable, and its `VAR_OUTPUT` one to the
        // temporary the call passes for it, each reached through `^`
        // (REQ-LOW-lowering-033, REQ-LOW-lowering-092). So is a function's
        // string or structure result, its result parameter
        // (REQ-LOW-lowering-079).
        let owner = Owner::Pou(frame.pou);
        let by_reference = matches!(
            info.variable_type,
            Some(VariableType::InOut | VariableType::Output)
        ) || self.is_result_through_parameter(frame, info)?;
        if by_reference {
            let parameter = self.declared_variable(decl, info, owner, true, span)?;
            return Ok(self.program.deref(parameter, span)?);
        }
        self.declared_variable(decl, info, owner, false, span)
    }

    /// The variable of the global `name`, declared in global scope. A
    /// `VAR_GLOBAL` a body reaches only through a `VAR_EXTERNAL` is the same
    /// variable.
    pub(crate) fn global_place(&mut self, name: &Id, span: &SourceSpan) -> Lowered<Place> {
        let symbols = self.analysis.context().symbols();
        match symbols.resolve_variable(name, &ScopeKind::Global) {
            Resolution::Variable { decl, info } | Resolution::GlobalNotVisible { decl, info } => {
                self.global_variable(decl, info, span)
            }
            Resolution::NotAVariable | Resolution::Undeclared => Err(Unlowered::NotYet),
        }
    }

    /// The variable of the global `decl` declares, which `info` describes.
    fn global_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        span: &SourceSpan,
    ) -> Lowered<Place> {
        // A `VAR_EXTERNAL` resolves to its global, whose declaration, not
        // the external's, says whether it is retained or located.
        let info = if decl.scope == ScopeKind::Global {
            self.analysis
                .context()
                .symbols()
                .global_variable(&decl.name)
                .map_or(info, |(_, global)| global)
        } else {
            info
        };
        self.declared_variable(decl, info, Owner::Global, false, span)
    }

    /// Whether `name`, in the body of `frame`, names a `VAR_IN_OUT`.
    pub(crate) fn is_in_out(&self, frame: &BodyFrame, name: &Id) -> bool {
        match self
            .analysis
            .context()
            .symbols()
            .resolve_variable(name, &frame.scope)
        {
            Resolution::Variable { info, .. } => info.variable_type == Some(VariableType::InOut),
            Resolution::GlobalNotVisible { .. }
            | Resolution::NotAVariable
            | Resolution::Undeclared => false,
        }
    }

    /// Whether `info` is the result of the function whose body `frame` is,
    /// and the result is a string or a structure, which the function returns
    /// through its result parameter (REQ-LOW-lowering-079). A method's
    /// result is not: a method whose result is a string or a structure is
    /// not lowered yet. Neither is a function whose result is an array.
    fn is_result_through_parameter(
        &mut self,
        frame: &BodyFrame,
        info: &SymbolInfo,
    ) -> Lowered<bool> {
        if frame.instance.is_some() || info.kind != SymbolKind::ResultVariable {
            return Ok(false);
        }
        let Some(type_id) = info.type_id else {
            return Ok(false);
        };
        let ty = self.lowered_type(type_id)?;
        Ok(matches!(
            self.program.types().kind(ty),
            Some(TypeKind::String(_) | TypeKind::Structure { .. })
        ))
    }

    /// The whole of the variable `decl` declares, allocated the first time
    /// it is reached: of its declared type, or, `by_reference`, of a non-null
    /// reference to it.
    fn declared_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        owner: Owner,
        by_reference: bool,
        span: &SourceSpan,
    ) -> Lowered<Place> {
        let id = match self.declared.get(&decl) {
            Some(id) => *id,
            None => {
                let type_id = info.type_id.ok_or(Unlowered::NotYet)?;
                let declared = declared(info, owner).ok_or(Unlowered::NotYet)?;
                let mut ty = self.lowered_type(type_id)?;
                if by_reference {
                    ty = self.non_null_reference(ty, &info.span)?;
                }
                let name = DebugName::new(decl.name.to_string(), info.span.clone());
                let id = self
                    .program
                    .push_declared_variable(name, owner, declared, ty)?;
                self.declared.insert(decl, id);
                id
            }
        };
        Ok(self.program.var(id, span)?)
    }

    /// A non-null reference to `target` (REQ-LOW-lowering-035).
    fn non_null_reference(
        &mut self,
        target: TypeId,
        span: &SourceSpan,
    ) -> Result<TypeId, Diagnostic> {
        self.program.push_type(
            TypeEntry {
                name: None,
                kind: TypeKind::Reference {
                    target,
                    nullability: Nullability::NonNull,
                },
            },
            span,
        )
    }
}

/// What the declaration `info` describes says of its variable, which `owner`
/// owns: its section, its direct address and its retention
/// (REQ-LOW-lowering-023, REQ-LOW-lowering-024). A variable the program
/// owns is a global, a compiler-provided one included, which records no
/// section. A function's or method's result is an output, as the debug
/// section shows it. `None` for a declaration that is no variable lowering
/// expresses yet, a `VAR_ACCESS`.
fn declared(info: &SymbolInfo, owner: Owner) -> Option<Declared> {
    let section = if owner == Owner::Global {
        Section::Global
    } else if matches!(info.kind, SymbolKind::ResultVariable) {
        Section::Output
    } else {
        match &info.variable_type {
            Some(VariableType::Global | VariableType::External) => Section::Global,
            Some(VariableType::Input) => Section::Input,
            Some(VariableType::Output) => Section::Output,
            Some(VariableType::InOut) => Section::InOut,
            Some(VariableType::Var) => Section::Local,
            Some(VariableType::VarTemp) => Section::Temporary,
            Some(VariableType::Access) | None => return None,
        }
    };
    Some(Declared {
        section,
        location: info.address.as_ref().map(direct_address),
        retention: info.qualifier.as_ref().map_or(Retention::None, retention),
    })
}

/// The retention a declaration with `qualifier` states.
pub(crate) fn retention(qualifier: &DeclarationQualifier) -> Retention {
    match qualifier {
        DeclarationQualifier::Retain => Retention::Retain,
        DeclarationQualifier::NonRetain => Retention::NonRetain,
        DeclarationQualifier::Persistent => Retention::Persistent,
        DeclarationQualifier::Unspecified | DeclarationQualifier::Constant => Retention::None,
    }
}

/// The lowered form of the direct address `address`.
pub(crate) fn direct_address(address: &AddressAssignment) -> DirectAddress {
    DirectAddress {
        area: match address.location {
            LocationPrefix::I => AddressArea::Input,
            LocationPrefix::Q => AddressArea::Output,
            LocationPrefix::M => AddressArea::Memory,
        },
        size: match address.size {
            SizePrefix::Nil | SizePrefix::X => AddressSize::Bit,
            SizePrefix::B => AddressSize::Byte,
            SizePrefix::W => AddressSize::Word,
            SizePrefix::D => AddressSize::DoubleWord,
            SizePrefix::L => AddressSize::LongWord,
            SizePrefix::Unspecified => AddressSize::Unspecified,
        },
        path: address.address.clone(),
    }
}
