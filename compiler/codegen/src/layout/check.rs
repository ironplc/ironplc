//! The check that the layout built from the lowered declarations is the
//! one the walk of the AST declarations builds
//! (`specs/design/lowered-program.md`, REQ-LOW-codegen-198).

use std::collections::HashMap;

use ironplc_container::ContainerBuilder;
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use super::lowered::Layout;
use crate::compile::CompileContext;

/// Checks `lowered`, the layout built from the lowered declarations, equal
/// to the one `ctx` laid out from the AST, for the program `program`: each
/// slot, the type it is operated at, each string's run of the data region,
/// where the region ends, how each array and what each reference refers to
/// is reached, how each function block
/// instance is, what a call of each user function, function block and
/// method needs, what the temporary buffers are sized for, the array
/// descriptors `builder` holds, and the debug entries of the variables and
/// the strings; and which slots hold a `VAR_IN_OUT`. An internal error
/// names each entry that differs.
pub(crate) fn check(
    lowered: &Layout,
    ctx: &CompileContext,
    builder: &ContainerBuilder,
    program: &Id,
) -> Result<(), Diagnostic> {
    let mut differences: Vec<String> = ctx
        .variables
        .differences(&lowered.slots)
        .into_iter()
        .map(|difference| format!("slot {difference}"))
        .chain(
            ctx.var_types
                .differences(&lowered.var_types)
                .into_iter()
                .map(|difference| format!("type {difference}")),
        )
        .chain(
            ctx.string_vars
                .differences(&lowered.string_vars)
                .into_iter()
                .map(|difference| format!("string {difference}")),
        )
        .chain(
            ctx.array_vars
                .differences(&lowered.array_vars)
                .into_iter()
                .map(|difference| format!("array {difference}")),
        )
        .chain(
            ctx.string_refs
                .differences(&lowered.string_refs)
                .into_iter()
                .map(|difference| format!("string reference {difference}")),
        )
        .chain(
            ctx.region_refs
                .differences(&lowered.region_refs)
                .into_iter()
                .map(|difference| format!("region reference {difference}")),
        )
        .chain(
            ctx.fb_instances
                .differences(&lowered.fb_instances)
                .into_iter()
                .map(|difference| format!("instance {difference}")),
        )
        .chain(
            ctx.in_out_params
                .differences(&lowered.in_out_params)
                .into_iter()
                .map(|difference| format!("in-out {difference}")),
        )
        .chain(by_name(
            "function",
            &ctx.user_functions,
            &lowered.user_functions,
        ))
        .chain(by_name(
            "function block",
            &ctx.user_fb_types,
            &lowered.user_fb_types,
        ))
        .collect();
    let (max_string_capacity, has_wide_string) = lowered.temporary_buffers(ctx.lowering()?.types());
    if (ctx.max_string_capacity, ctx.has_wide_string) != (max_string_capacity, has_wide_string) {
        differences.push(format!(
            "temporary buffers ({}, {}) and ({max_string_capacity}, {has_wide_string})",
            ctx.max_string_capacity, ctx.has_wide_string
        ));
    }
    if builder.array_descriptors() != lowered.descriptors {
        differences.push(format!(
            "array descriptors {:?} and {:?}",
            builder.array_descriptors(),
            lowered.descriptors
        ));
    }
    differences.extend(
        lowered
            .debug
            .differences(&ctx.debug_var_names, &ctx.debug_string_layouts),
    );
    if ctx.data_region_offset != lowered.data_end {
        differences.push(format!(
            "data region ends at {}, not {}",
            lowered.data_end, ctx.data_region_offset
        ));
    }
    if differences.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::internal_error_at(Label::span(
        program.span.clone(),
        format!(
            "The layout built from the lowered declarations differs: {}",
            differences.join("; ")
        ),
    )))
}

/// Each entry one of `mine` and `theirs` records by name that the other does
/// not record the same, as [`Scoped::differences`] gives them, each led by
/// `what` it is of.
fn by_name<V: PartialEq + std::fmt::Debug>(
    what: &str,
    mine: &HashMap<String, V>,
    theirs: &HashMap<String, V>,
) -> Vec<String> {
    let mut names: Vec<&String> = mine.keys().chain(theirs.keys()).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| {
            let (mine, theirs) = (mine.get(name), theirs.get(name));
            (mine != theirs).then(|| format!("{what} {name}: {mine:?} and {theirs:?}"))
        })
        .collect()
}
