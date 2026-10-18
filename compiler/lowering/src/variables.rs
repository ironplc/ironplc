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
    AddressArea, AddressSize, Declared, DirectAddress, Field, Nullability, Owner, Place, PouId,
    Retention, Section, TypeEntry, TypeId, TypeKind, VarId,
};

use crate::{Body, Lowering};

/// What lowering knows of the body a statement belongs to.
#[derive(Clone, Debug)]
pub(crate) struct BodyFrame {
    pub(crate) pou: PouId,
    /// The scope the body's names are resolved from.
    pub(crate) scope: ScopeKind,
    /// The instance a program, function block or method body works on;
    /// `None` for a function.
    pub(crate) instance: Option<Instance>,
}

/// The instance parameter of a body and the fields of its type.
#[derive(Clone, Debug)]
pub(crate) struct Instance {
    pub(crate) this: VarId,
    /// The type of the instance `this` refers to.
    pub(crate) instance_type: TypeId,
    /// The name of each field of the instance's type, by position.
    field_names: Vec<Id>,
    /// The scope that declares the fields: the body's own for a program or
    /// function block, and its block's for a method.
    scope: ScopeKind,
}

/// A value lowering made, `None` when it cannot express it yet, or a P9998.
type Made<T> = Result<Option<T>, Diagnostic>;

impl<'a> Lowering<'a> {
    /// The frame of `body`, made the first time a statement of it is
    /// lowered; `None` when lowering cannot express the body's instance yet.
    pub(crate) fn frame(&mut self, body: Body<'a>) -> Made<BodyFrame> {
        let (pou, path) = match body {
            Body::Program(program) => (self.named_pou(&program.name), vec![program.name.clone()]),
            Body::FunctionBlock(block) => (
                self.named_pou(&block.name.name),
                vec![block.name.name.clone()],
            ),
            Body::Function(function) => {
                (self.named_pou(&function.name), vec![function.name.clone()])
            }
            Body::Method { block, method } => (
                self.method_pou(&block.name.name, &method.name),
                vec![block.name.name.clone(), method.name.clone()],
            ),
        };
        if let Some(frame) = self.bodies.get(&pou) {
            return Ok(frame.clone());
        }
        let scope = ScopeKind::Named(ScopePath::new(path));
        let instance = match body {
            // A program's `VAR_ACCESS` paths name other variables, and no
            // statement names one, so they are no fields of its instance.
            Body::Program(program) => self
                .program_instance(pou, &program.name, &program.variables)?
                .map(Some),
            Body::FunctionBlock(block) => self
                .block_instance(pou, &block.name.name, scope.clone())?
                .map(Some),
            Body::Function(_) => Some(None),
            // A method works on an instance of its block, whose fields its
            // block's scope declares.
            Body::Method { block, .. } => {
                let fields = ScopeKind::Named(ScopePath::new(vec![block.name.name.clone()]));
                self.block_instance(pou, &block.name.name, fields)?
                    .map(Some)
            }
        };
        let frame = instance.map(|instance| BodyFrame {
            pou,
            scope,
            instance,
        });
        self.bodies.insert(pou, frame.clone());
        Ok(frame)
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
    fn program_instance(&mut self, pou: PouId, name: &Id, variables: &[VarDecl]) -> Made<Instance> {
        let Some(fields) = self.instance_fields(variables)? else {
            return Ok(None);
        };
        let program = self.program.push_type(
            TypeEntry {
                name: Some(DebugName::new(name.to_string(), name.span())),
                kind: TypeKind::Program { fields, body: pou },
            },
            &name.span(),
        )?;
        let scope = ScopeKind::Named(ScopePath::new(vec![name.clone()]));
        self.instance(pou, name, program, scope).map(Some)
    }

    /// The instance of a function block, the body of `pou` works on: the
    /// block's own type, whose fields `scope` declares.
    fn block_instance(&mut self, pou: PouId, name: &Id, scope: ScopeKind) -> Made<Instance> {
        let types = self.analysis.context().types();
        let Some(id) = types.id_of(&TypeName::from(&name.to_string())) else {
            return Ok(None);
        };
        let Some(block) = self.lowered_type(id)? else {
            return Ok(None);
        };
        self.instance(pou, name, block, scope).map(Some)
    }

    /// The fields of the instance of a program that declares `variables`:
    /// one for each variable, in the order they are declared, as a function
    /// block's are (see `block_fields` in `types.rs`), other than a
    /// `VAR_TEMP`, a `VAR_EXTERNAL`, a global variable or an access path,
    /// which a PLCopen program declares in its interface and statements
    /// reach in global scope, and a located variable with no name, which no
    /// statement names (REQ-LOW-lowering-170). `None` when lowering cannot
    /// express the type of one of them yet.
    fn instance_fields(&mut self, variables: &[VarDecl]) -> Made<Vec<Field>> {
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
            let Some(type_id) = decl.type_id else {
                return Ok(None);
            };
            let Some(ty) = self.lowered_type(type_id)? else {
                return Ok(None);
            };
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
        Ok(Some(fields))
    }

    /// The instance parameter of the body of `pou`, a non-null reference to
    /// `instance_type`, whose fields `scope` declares.
    fn instance(
        &mut self,
        pou: PouId,
        name: &Id,
        instance_type: TypeId,
        scope: ScopeKind,
    ) -> Result<Instance, Diagnostic> {
        let field_names = self
            .program
            .types()
            .fields(instance_type)
            .unwrap_or_default()
            .iter()
            .map(|field| Id::from(&field.name.to_string()))
            .collect();
        let this_type = self.non_null_reference(instance_type, &name.span())?;
        let this = self.program.push_declared_variable(
            DebugName::new("THIS", name.span()),
            Owner::Pou(pou),
            Declared::in_section(Section::Instance),
            this_type,
        )?;
        Ok(Instance {
            this,
            instance_type,
            field_names,
            scope,
        })
    }

    /// The place `name` refers to in the body of `frame`; `None` when it is
    /// not a variable lowering can reach yet.
    pub(crate) fn named_place(
        &mut self,
        frame: &BodyFrame,
        name: &Id,
        span: &SourceSpan,
    ) -> Made<Place> {
        let symbols = self.analysis.context().symbols();
        let (decl, info) = match symbols.resolve_variable(name, &frame.scope) {
            Resolution::Variable { decl, info } => (decl, info),
            Resolution::GlobalNotVisible { .. }
            | Resolution::NotAVariable
            | Resolution::Undeclared => return Ok(None),
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
                Some(VariableType::VarTemp | VariableType::InOut) => return Ok(None),
                Some(_) | None => {}
            }
            let position = instance
                .field_names
                .iter()
                .position(|field| *field == decl.name);
            let Some(field) = position
                .and_then(|position| self.program.field_idx(instance.instance_type, position))
            else {
                return Ok(None);
            };
            let this = self.program.var(instance.this, span)?;
            let instance_place = self.program.deref(this, span)?;
            let place = self.program.field(instance_place, field, span)?;
            return Ok(Some(place));
        }
        if decl.scope != frame.scope {
            // A field a function block inherits through `EXTENDS`.
            return Ok(None);
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
            let Some(parameter) = self.declared_variable(decl, info, owner, true, span)? else {
                return Ok(None);
            };
            return self.program.deref(parameter, span).map(Some);
        }
        self.declared_variable(decl, info, owner, false, span)
    }

    /// The variable of the global `name`, declared in global scope; `None`
    /// when it is not one lowering can express yet. A `VAR_GLOBAL` a body
    /// reaches only through a `VAR_EXTERNAL` is the same variable.
    pub(crate) fn global_place(&mut self, name: &Id, span: &SourceSpan) -> Made<Place> {
        let symbols = self.analysis.context().symbols();
        match symbols.resolve_variable(name, &ScopeKind::Global) {
            Resolution::Variable { decl, info } | Resolution::GlobalNotVisible { decl, info } => {
                self.global_variable(decl, info, span)
            }
            Resolution::NotAVariable | Resolution::Undeclared => Ok(None),
        }
    }

    /// The variable of the global `decl` declares, which `info` describes.
    fn global_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        span: &SourceSpan,
    ) -> Made<Place> {
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
    /// result is not: the route that reads the AST does not compile a method
    /// whose result is a string, and a method whose result is a structure is
    /// not lowered yet. A function whose result is an array is not lowered:
    /// the route that reads the AST does not compile one.
    fn is_result_through_parameter(
        &mut self,
        frame: &BodyFrame,
        info: &SymbolInfo,
    ) -> Result<bool, Diagnostic> {
        if frame.instance.is_some() || info.kind != SymbolKind::ResultVariable {
            return Ok(false);
        }
        let Some(type_id) = info.type_id else {
            return Ok(false);
        };
        let Some(ty) = self.lowered_type(type_id)? else {
            return Ok(false);
        };
        Ok(matches!(
            self.program.types().kind(ty),
            Some(TypeKind::String(_) | TypeKind::Structure { .. })
        ))
    }

    /// The whole of the variable `decl` declares, allocated the first time
    /// it is reached: of its declared type, or, `by_reference`, of a non-null
    /// reference to it. `None` when lowering cannot express its type yet.
    fn declared_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        owner: Owner,
        by_reference: bool,
        span: &SourceSpan,
    ) -> Made<Place> {
        let id = match self.declared.get(&decl) {
            Some(id) => *id,
            None => {
                let (Some(type_id), Some(declared)) = (info.type_id, declared(info, owner)) else {
                    return Ok(None);
                };
                let Some(mut ty) = self.lowered_type(type_id)? else {
                    return Ok(None);
                };
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
        self.program.var(id, span).map(Some)
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
