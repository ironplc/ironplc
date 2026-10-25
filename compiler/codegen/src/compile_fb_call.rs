//! The instructions of a function block call: the instance pushed once,
//! each input stored in its field, the call, and the instance popped
//! (`FB_LOAD_INSTANCE`, `FB_STORE_PARAM`, `FB_CALL`, `POP`). A call's
//! outputs are stored after it, each in its target.

use crate::access::Named;
use std::collections::HashMap;

use ironplc_container::FbTypeId;
use ironplc_ir::lowered::FieldIdx;
use ironplc_ir::Diagnostic;
use ironplc_ir::SourceSpan;

use crate::compile::{CompileContext, VarTypeInfo};
use crate::compile_expr::emit_truncation;
use crate::emit::Emitter;

/// A call of a function block instance, whose instance is on the stack
/// from [`FbCallSite::open`] until [`FbCallSite::close`].
pub(crate) struct FbCallSite {
    /// The type `FB_CALL` dispatches on: a user-defined block's, or a
    /// standard block's in the VM.
    type_id: FbTypeId,
    /// The index of each field of the instance.
    field_indices: HashMap<FieldIdx, u8>,
    /// The declared type of each field of the instance.
    field_types: HashMap<FieldIdx, VarTypeInfo>,
}

impl FbCallSite {
    /// Opens a call of the instance named `instance` by pushing it. A name
    /// that is no instance is not implemented, at `span`, the call.
    pub(crate) fn open(
        emitter: &mut Emitter,
        ctx: &CompileContext,
        instance: &Named,
        span: &SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let info = ctx
            .fb_instance(instance)
            .ok_or_else(|| Diagnostic::todo_with_span(span.clone()))?;
        emitter.emit_fb_load_instance(info.var_index);
        Ok(Self {
            type_id: info.type_id,
            field_indices: info.field_indices.clone(),
            field_types: info.field_types.clone(),
        })
    }

    /// Stores the value on the stack in the input `field`, narrowed to the
    /// field's declared type as an assignment to the field is. A field the
    /// instance does not keep is not implemented, at `span`, the call.
    pub(crate) fn store_input(
        &self,
        emitter: &mut Emitter,
        field: FieldIdx,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let index = self
            .field_indices
            .get(&field)
            .ok_or_else(|| Diagnostic::todo_with_span(span.clone()))?;
        if let Some(field_type) = self.field_types.get(&field) {
            emit_truncation(emitter, *field_type);
        }
        emitter.emit_fb_store_param(*index);
        Ok(())
    }

    /// Calls the block and pops the instance. A call of a user-defined block
    /// is an edge of the call graph; a standard block runs in the VM and
    /// adds no frame.
    pub(crate) fn close(self, emitter: &mut Emitter, ctx: &mut CompileContext) {
        emitter.emit_fb_call(self.type_id);
        if let Some(user_fb) = ctx
            .user_fb_types
            .values()
            .find(|info| info.type_id == self.type_id)
        {
            ctx.record_call_edge(user_fb.function_id);
        }
        emitter.emit_pop();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ironplc_container::{FbTypeId, VarIndex};
    use ironplc_ir::SourceSpan;

    use super::FbCallSite;
    use crate::access::{test_variable, Stored};
    use crate::compile::{CompileContext, FbInstanceInfo};
    use crate::emit::Emitter;

    #[test]
    fn open_when_no_instance_has_name_then_not_implemented() {
        let ctx = CompileContext::new();
        let mut emitter = Emitter::new();

        let result = FbCallSite::open(
            &mut emitter,
            &ctx,
            &test_variable("missing"),
            &SourceSpan::default(),
        );

        assert_eq!(
            result.err().map(|diagnostic| diagnostic.code),
            Some("P9999".to_string())
        );
    }

    #[test]
    fn store_input_when_instance_has_no_such_field_then_not_implemented() {
        let mut ctx = CompileContext::new();
        let f = test_variable("f");
        ctx.fb_instances.insert(
            ctx.scope,
            f.stored,
            FbInstanceInfo {
                var_index: VarIndex::new(0),
                type_id: FbTypeId::default(),
                data_offset: 0,
                field_indices: HashMap::new(),
                field_types: HashMap::new(),
                held: Vec::new(),
            },
        );
        let mut emitter = Emitter::new();
        let call = FbCallSite::open(&mut emitter, &ctx, &f, &SourceSpan::default())
            .expect("the instance is declared");

        // A field of another type, which the instance does not keep.
        let Stored::Field(_, missing) = test_variable("missing").stored else {
            unreachable!("a program's variable is a field of its instance")
        };

        let result = call.store_input(&mut emitter, missing, &SourceSpan::default());

        assert_eq!(
            result.err().map(|diagnostic| diagnostic.code),
            Some("P9999".to_string())
        );
    }
}
