//! The lowered program's POUs, each matched once to what this backend's
//! layout knows it as (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! The layout keeps each variable and field by its lowered id
//! ([`crate::access::Stored`]), but still knows a POU by its name. Once it
//! is built, [`LoweredIds::matched`] matches each lowered POU to that entry,
//! and the bridge's lookups take the lowered id from then on. A POU no
//! compiled body reaches is left out.

use std::collections::HashMap;

use ironplc_dsl::core::Id;
use ironplc_ir::lowered::PouId;
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

/// The layout's entry for each lowered POU that has one.
#[derive(Default)]
pub(crate) struct LoweredIds {
    pous: HashMap<PouId, Pou>,
}

impl LoweredIds {
    /// Matches each POU of `lowering` to the entry `ctx`'s layout has for it,
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
