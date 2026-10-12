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
//! * a global is a variable of its own, and a `VAR_EXTERNAL` names the
//!   global it refers to rather than a variable of its own
//!   (REQ-LOW-lowering-022).
//!
//! Each declaration gets one variable, by its [`DeclRef`], so two variables
//! of one name in different scopes are different variables
//! (REQ-LOW-lowering-020).

use ironplc_analyzer::symbol_environment::{DeclRef, Resolution, ScopeKind, ScopePath, SymbolInfo};
use ironplc_dsl::common::{TypeName, VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    Field, Nullability, Owner, Place, PouId, Section, TypeEntry, TypeId, TypeKind, VarId,
};

use crate::{Body, Lowering};

/// What lowering knows of the body a statement belongs to.
#[derive(Clone, Debug)]
pub(crate) struct BodyFrame {
    pub(crate) pou: PouId,
    /// The scope the body's names are resolved from.
    pub(crate) scope: ScopeKind,
    /// The instance a program or function block body works on; `None` for a
    /// function.
    pub(crate) instance: Option<Instance>,
}

/// The instance parameter of a body and the fields of its type.
#[derive(Clone, Debug)]
pub(crate) struct Instance {
    this: VarId,
    /// The type of the instance `this` refers to.
    instance_type: TypeId,
    /// The name of each field of the instance's type, by position.
    field_names: Vec<Id>,
}

/// A value lowering made, `None` when it cannot express it yet, or a P9998.
type Made<T> = Result<Option<T>, Diagnostic>;

impl<'a> Lowering<'a> {
    /// The frame of `body`, made the first time a statement of it is
    /// lowered; `None` when lowering cannot express the body's instance yet.
    pub(crate) fn frame(&mut self, body: Body<'a>) -> Made<BodyFrame> {
        let name = match body {
            Body::Program(program) => program.name.clone(),
            Body::FunctionBlock(block) => block.name.name.clone(),
            Body::Function(function) => function.name.clone(),
        };
        let pou = self.pou(&name.to_string(), &name.span());
        if let Some(frame) = self.bodies.get(&pou) {
            return Ok(frame.clone());
        }
        let scope = ScopeKind::Named(ScopePath::new(vec![name.clone()]));
        let instance = match body {
            Body::Program(program) if !program.access_variables.is_empty() => None,
            Body::Program(program) => self
                .program_instance(pou, &name, &program.variables)?
                .map(Some),
            Body::FunctionBlock(_) => self.block_instance(pou, &name)?.map(Some),
            Body::Function(_) => Some(None),
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

    /// The instance of a program: a type with a field for each variable the
    /// program declares, other than a `VAR_TEMP` or a `VAR_EXTERNAL`.
    fn program_instance(&mut self, pou: PouId, name: &Id, variables: &[VarDecl]) -> Made<Instance> {
        let mut fields = Vec::new();
        let mut field_names = Vec::new();
        for decl in variables {
            let section = match decl.var_type {
                VariableType::VarTemp | VariableType::External => continue,
                VariableType::Var => Section::Local,
                VariableType::Input => Section::Input,
                VariableType::Output => Section::Output,
                VariableType::InOut => Section::InOut,
                VariableType::Global | VariableType::Access => return Ok(None),
            };
            let (Some(field_name), Some(type_id)) = (decl.identifier.symbolic_id(), decl.type_id)
            else {
                return Ok(None);
            };
            let Some(ty) = self.lowered_type(type_id)? else {
                return Ok(None);
            };
            fields.push(Field::new(
                &field_name.to_string(),
                field_name.span(),
                Some(section),
                ty,
            ));
            field_names.push(field_name.clone());
        }
        let program = self.program.push_type(
            TypeEntry {
                name: Some(DebugName::new(name.to_string(), name.span())),
                kind: TypeKind::Program { fields, body: pou },
            },
            &name.span(),
        )?;
        self.instance(pou, name, program, field_names).map(Some)
    }

    /// The instance of a function block: the block's own type, whose fields
    /// are the analyzer's, in the analyzer's order.
    fn block_instance(&mut self, pou: PouId, name: &Id) -> Made<Instance> {
        let types = self.analysis.context().types();
        let Some(id) = types.id_of(&TypeName::from(&name.to_string())) else {
            return Ok(None);
        };
        let Some(attributes) = types.get_by_id(id) else {
            return Ok(None);
        };
        let ironplc_analyzer::SemanticType::FunctionBlock { fields, .. } =
            &attributes.representation
        else {
            return Ok(None);
        };
        let field_names = fields.iter().map(|field| field.name.clone()).collect();
        let Some(block) = self.lowered_type(id)? else {
            return Ok(None);
        };
        self.instance(pou, name, block, field_names).map(Some)
    }

    /// The instance parameter of the body of `pou`, a non-null reference to
    /// `instance_type`.
    fn instance(
        &mut self,
        pou: PouId,
        name: &Id,
        instance_type: TypeId,
        field_names: Vec<Id>,
    ) -> Result<Instance, Diagnostic> {
        let this_type = self.program.push_type(
            TypeEntry {
                name: None,
                kind: TypeKind::Reference {
                    target: instance_type,
                    nullability: Nullability::NonNull,
                },
            },
            &name.span(),
        )?;
        let this = self.program.push_variable(
            DebugName::new("THIS", name.span()),
            Owner::Pou(pou),
            this_type,
        )?;
        Ok(Instance {
            this,
            instance_type,
            field_names,
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
        if decl.scope == ScopeKind::Global {
            return self.declared_variable(decl, info, Owner::Global, span);
        }
        if decl.scope != frame.scope {
            // A field a function block inherits through `EXTENDS`.
            return Ok(None);
        }
        if info.variable_type == Some(VariableType::InOut) {
            // A `VAR_IN_OUT` is a reference to the caller's variable, reached
            // through `^`, which lowering does not express until calls bind
            // one.
            return Ok(None);
        }
        let is_temp = info.variable_type == Some(VariableType::VarTemp);
        match &frame.instance {
            Some(instance) if !is_temp => {
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
                Ok(Some(place))
            }
            Some(_) | None => self.declared_variable(decl, info, Owner::Pou(frame.pou), span),
        }
    }

    /// The whole of the variable `decl` declares, allocated the first time
    /// it is reached; `None` when lowering cannot express its type yet.
    fn declared_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        owner: Owner,
        span: &SourceSpan,
    ) -> Made<Place> {
        let id = match self.declared.get(&decl) {
            Some(id) => *id,
            None => {
                let Some(type_id) = info.type_id else {
                    return Ok(None);
                };
                let Some(ty) = self.lowered_type(type_id)? else {
                    return Ok(None);
                };
                let name = DebugName::new(decl.name.to_string(), info.span.clone());
                let id = self.program.push_variable(name, owner, ty)?;
                self.declared.insert(decl, id);
                id
            }
        };
        self.program.var(id, span).map(Some)
    }
}
