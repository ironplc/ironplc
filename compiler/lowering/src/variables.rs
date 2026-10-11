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
use ironplc_dsl::common::{VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    Field, FieldIdx, Nullability, Place, PouId, Section, TypeEntry, TypeId, TypeKind, VarId,
};

use crate::{Body, Lowering};

/// What a variable belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Global,
    Pou(PouId),
}

/// A variable of the lowered program that is not a field.
#[derive(Clone, Debug)]
pub struct Variable {
    /// The name as declared, and the span of its declaration, for debug
    /// information and diagnostics.
    pub name: DebugName,
    pub owner: Owner,
    pub ty: TypeId,
}

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
    /// The type of `this`: a non-null reference to the instance's type.
    this_type: TypeId,
    /// The name of each field of the instance's type, by position.
    field_names: Vec<Id>,
}

impl<'a> Lowering<'a> {
    /// The frame of `body`, made the first time a statement of it is
    /// lowered; `None` when lowering cannot express the body's instance yet.
    pub(crate) fn frame(&mut self, body: Body<'a>) -> Option<BodyFrame> {
        let name = match body {
            Body::Program(program) => program.name.clone(),
            Body::FunctionBlock(block) => block.name.name.clone(),
            Body::Function(function) => function.name.clone(),
        };
        let pou = self.pou(&name.to_string());
        if let Some(frame) = self.bodies.get(&pou) {
            return frame.clone();
        }
        let scope = ScopeKind::Named(ScopePath::new(vec![name.clone()]));
        let instance = match body {
            Body::Program(program) => {
                if !program.access_variables.is_empty() {
                    None
                } else {
                    self.program_instance(pou, &name, &program.variables)
                        .map(Some)
                }
            }
            Body::FunctionBlock(_) => self.block_instance(pou, &name).map(Some),
            Body::Function(_) => Some(None),
        };
        let frame = instance.map(|instance| BodyFrame {
            pou,
            scope,
            instance,
        });
        self.bodies.insert(pou, frame.clone());
        frame
    }

    /// The POU named `name`, allocated the first time it is asked for.
    pub(crate) fn pou(&mut self, name: &str) -> PouId {
        let key = name.to_lowercase();
        if let Some(pou) = self.pou_ids.get(&key) {
            return *pou;
        }
        let pou = PouId::new(self.pous.len() as u32);
        self.pous.push(name.to_string());
        self.pou_ids.insert(key, pou);
        pou
    }

    /// The instance of a program: a type with a field for each variable the
    /// program declares, other than a `VAR_TEMP` or a `VAR_EXTERNAL`.
    fn program_instance(
        &mut self,
        pou: PouId,
        name: &Id,
        variables: &[VarDecl],
    ) -> Option<Instance> {
        let mut fields = Vec::new();
        let mut field_names = Vec::new();
        for decl in variables {
            let section = match decl.var_type {
                VariableType::VarTemp | VariableType::External => continue,
                VariableType::Var => Section::Local,
                VariableType::Input => Section::Input,
                VariableType::Output => Section::Output,
                VariableType::InOut => Section::InOut,
                VariableType::Global | VariableType::Access => return None,
            };
            let field_name = decl.identifier.symbolic_id()?;
            let ty = self.lowered_type(decl.type_id?)?;
            fields.push(Field::new(
                &field_name.to_string(),
                field_name.span(),
                Some(section),
                ty,
            ));
            field_names.push(field_name.clone());
        }
        let program = self.types.push(TypeEntry {
            name: Some(DebugName::new(name.to_string(), name.span())),
            kind: TypeKind::Program { fields, body: pou },
        });
        Some(self.instance(pou, name, program, field_names))
    }

    /// The instance of a function block: the block's own type, whose fields
    /// are the analyzer's, in the analyzer's order.
    fn block_instance(&mut self, pou: PouId, name: &Id) -> Option<Instance> {
        let types = self.analysis.context().types();
        let id = types.id_of(&ironplc_dsl::common::TypeName::from(&name.to_string()))?;
        let ironplc_analyzer::SemanticType::FunctionBlock { fields, .. } =
            &types.get_by_id(id)?.representation
        else {
            return None;
        };
        let field_names = fields.iter().map(|field| field.name.clone()).collect();
        let block = self.lowered_type(id)?;
        Some(self.instance(pou, name, block, field_names))
    }

    /// The instance parameter of the body of `pou`, a non-null reference to
    /// `instance_type`.
    fn instance(
        &mut self,
        pou: PouId,
        name: &Id,
        instance_type: TypeId,
        field_names: Vec<Id>,
    ) -> Instance {
        let this_type = self.types.push(TypeEntry {
            name: None,
            kind: TypeKind::Reference {
                target: instance_type,
                nullability: Nullability::NonNull,
            },
        });
        let this = self.push_variable(Variable {
            name: DebugName::new("THIS", name.span()),
            owner: Owner::Pou(pou),
            ty: this_type,
        });
        Instance {
            this,
            this_type,
            field_names,
        }
    }

    /// The place `name` refers to in the body of `frame`; `None` when it is
    /// not a variable lowering can reach yet.
    pub(crate) fn named_place(
        &mut self,
        frame: &BodyFrame,
        name: &Id,
        span: &SourceSpan,
    ) -> Result<Option<Place>, Diagnostic> {
        let symbols = self.analysis.context().symbols();
        let (decl, info) = match symbols.resolve_variable(name, &frame.scope) {
            Resolution::Variable { decl, info } => (decl, info),
            Resolution::GlobalNotVisible { .. }
            | Resolution::NotAVariable
            | Resolution::Undeclared => return Ok(None),
        };
        if decl.scope == ScopeKind::Global {
            return Ok(self.declared_variable(decl, info, Owner::Global));
        }
        if decl.scope != frame.scope {
            // A field a function block inherits through `EXTENDS`.
            return Ok(None);
        }
        let is_temp = info.variable_type == Some(VariableType::VarTemp);
        match &frame.instance {
            Some(instance) if !is_temp => {
                let Some(index) = instance
                    .field_names
                    .iter()
                    .position(|field| *field == decl.name)
                else {
                    return Ok(None);
                };
                let place = Place::var(instance.this, instance.this_type)
                    .deref(&self.types, span)?
                    .field(&self.types, FieldIdx::new(index as u32), span)?;
                Ok(Some(place))
            }
            Some(_) | None => Ok(self.declared_variable(decl, info, Owner::Pou(frame.pou))),
        }
    }

    /// The whole of the variable `decl` declares, allocated the first time
    /// it is reached; `None` when lowering cannot express its type yet.
    fn declared_variable(
        &mut self,
        decl: DeclRef,
        info: &SymbolInfo,
        owner: Owner,
    ) -> Option<Place> {
        if let Some(id) = self.declared.get(&decl) {
            let ty = self.variables[id.index() as usize].ty;
            return Some(Place::var(*id, ty));
        }
        let ty = self.lowered_type(info.type_id?)?;
        let id = self.push_variable(Variable {
            name: DebugName::new(decl.name.to_string(), info.span.clone()),
            owner,
            ty,
        });
        self.declared.insert(decl, id);
        Some(Place::var(id, ty))
    }

    fn push_variable(&mut self, variable: Variable) -> VarId {
        let id = VarId::new(self.variables.len() as u32);
        self.variables.push(variable);
        id
    }
}
