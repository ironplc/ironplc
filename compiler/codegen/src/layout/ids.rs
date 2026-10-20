//! The lowered program's ids, each matched once to what this backend's
//! layout knows it as (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! The layout is still built from the AST, so it knows each variable, POU
//! and field by the scope that declares it and its name. Once it is built,
//! [`LoweredIds::matched`] matches each lowered variable, POU and function
//! block or program field to that entry, by owner and name, and the bridge's
//! lookups take the lowered id from then on.
//!
//! What the layout keeps no entry for is left out: a temporary lowering
//! makes for a call's output or result, which has no storage of its own, a
//! program instance, a POU no compiled body reaches, and a field a block
//! inherits, whose storage this backend's layout does not reserve.

use std::collections::HashMap;

use ironplc_dsl::core::Id;
use ironplc_ir::lowered::{FieldIdx, Owner, PouId, TypeId, TypeKind, VarId};
use ironplc_lowering::PartialProgram;

use super::names::ScopeId;
use crate::compile::CompileContext;

/// What the layout knows a lowered POU as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LaidOut {
    /// The program the container runs.
    Program,
    /// The user function of this name, in lower case.
    Function(String),
    /// The function block of this name, in upper case: a user's, or a
    /// standard one, which has no body in the lowered program.
    Block(String),
    /// The method `method`, in lower case, of the user function block
    /// `block`, in upper case.
    Method { block: String, method: String },
}

/// A lowered POU as the layout knows it, and the scope its body is
/// compiled in; `None` for a standard function block, which has no body.
#[derive(Clone, Debug)]
struct Pou {
    laid_out: LaidOut,
    scope: Option<ScopeId>,
}

/// The layout's entry for each lowered id that has one.
#[derive(Default)]
pub(crate) struct LoweredIds {
    pous: HashMap<PouId, Pou>,
    /// The scope that declares each variable, and its name there.
    variables: HashMap<VarId, (ScopeId, Id)>,
    /// The name of the slot each field of a function block or program
    /// instance is kept in while its body runs, in that body's scope.
    fields: HashMap<(TypeId, FieldIdx), Id>,
}

impl LoweredIds {
    /// Matches each id of `lowering` to the entry `ctx`'s layout has for it,
    /// once the layout is built. `program` is the name of the program the
    /// container runs.
    pub(crate) fn matched(lowering: &PartialProgram, ctx: &CompileContext, program: &Id) -> Self {
        let mut ids = Self::default();
        let pous = lowering.pous();
        for pou in pous.ids() {
            if let Some(matched) = match_pou(lowering, ctx, program, pou) {
                ids.pous.insert(pou, matched);
            }
        }
        for (var, variable) in lowering.variables().iter() {
            // A global the program declares in its own interface is laid out
            // with the program's variables.
            let scopes = match variable.owner() {
                Owner::Global => vec![ScopeId::Global, ScopeId::Program],
                Owner::Pou(pou) => ids.scope(pou).into_iter().collect(),
            };
            let name = Id::from(&variable.name().to_string());
            if let Some(scope) = scopes
                .into_iter()
                .find(|scope| ctx.variables.declares(*scope, &name))
            {
                ids.variables.insert(var, (scope, name));
            }
        }
        let types = lowering.types();
        for (ty, entry) in types.iter() {
            let (fields, body) = match &entry.kind {
                TypeKind::FunctionBlock { fields, body, .. }
                | TypeKind::Program { fields, body } => (fields, *body),
                TypeKind::Bool
                | TypeKind::Integer { .. }
                | TypeKind::BitString { .. }
                | TypeKind::Real { .. }
                | TypeKind::Temporal { .. }
                | TypeKind::Enumeration { .. }
                | TypeKind::Subrange { .. }
                | TypeKind::String(_)
                | TypeKind::Array { .. }
                | TypeKind::Structure { .. }
                | TypeKind::Reference { .. } => continue,
            };
            let Some(scope) = ids.scope(body) else {
                continue;
            };
            for (position, field) in fields.iter().enumerate() {
                let name = Id::from(&field.name.to_string());
                let Some(idx) = types.field_idx(ty, position) else {
                    continue;
                };
                if ctx.variables.declares(scope, &name) {
                    ids.fields.insert((ty, idx), name);
                }
            }
        }
        ids
    }

    /// What the layout knows `pou` as; `None` for a POU it keeps no entry
    /// for.
    pub(crate) fn pou(&self, pou: PouId) -> Option<&LaidOut> {
        self.pous.get(&pou).map(|matched| &matched.laid_out)
    }

    /// The scope the body of `pou` is compiled in.
    pub(crate) fn scope(&self, pou: PouId) -> Option<ScopeId> {
        self.pous.get(&pou)?.scope
    }

    /// The POU whose body is compiled in `scope`.
    pub(crate) fn pou_in(&self, scope: ScopeId) -> Option<PouId> {
        self.pous
            .iter()
            .find(|(_, matched)| matched.scope == Some(scope))
            .map(|(pou, _)| *pou)
    }

    /// The scope that declares `var`, and its name there.
    pub(crate) fn variable(&self, var: VarId) -> Option<&(ScopeId, Id)> {
        self.variables.get(&var)
    }

    /// The name of the slot the field `field` of an instance of the
    /// function block or program `ty` is kept in while its body runs.
    pub(crate) fn field(&self, ty: TypeId, field: FieldIdx) -> Option<&Id> {
        self.fields.get(&(ty, field))
    }
}

/// What the layout of `ctx` knows `pou` as, and the scope its body is
/// compiled in: a method of a user block, a user function, a user or
/// standard function block, or the program `program`.
fn match_pou(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    program: &Id,
    pou: PouId,
) -> Option<Pou> {
    let pous = lowering.pous();
    let name = pous.name(pou)?.to_string();
    if let Some(block) = pous.method_of(pou) {
        let block = pous.name(block)?.to_string().to_uppercase();
        let method = name.to_lowercase();
        let laid_out = ctx.user_fb_type(&block)?;
        let scope = ScopeId::Method {
            block: laid_out.function_id,
            method: laid_out.methods.get(&method)?.function_id,
        };
        return Some(Pou {
            laid_out: LaidOut::Method { block, method },
            scope: Some(scope),
        });
    }
    let lower = name.to_lowercase();
    if let Some(function) = ctx.user_function(&lower) {
        return Some(Pou {
            scope: Some(ScopeId::Function(function.function_id)),
            laid_out: LaidOut::Function(lower),
        });
    }
    let upper = name.to_uppercase();
    if let Some(block) = ctx.user_fb_type(&upper) {
        return Some(Pou {
            scope: Some(ScopeId::Block(block.function_id)),
            laid_out: LaidOut::Block(upper),
        });
    }
    if ctx.standard_blocks.contains_key(&upper) {
        return Some(Pou {
            laid_out: LaidOut::Block(upper),
            scope: None,
        });
    }
    (Id::from(&name) == *program).then_some(Pou {
        laid_out: LaidOut::Program,
        scope: Some(ScopeId::Program),
    })
}
