//! Globals, program instances and tasks, from the configuration when there
//! is one.

use ironplc_analyzer::system_globals::SYSTEM_UPTIME_GLOBALS;
use ironplc_dsl::common::{LibraryElementKind, ProgramDeclaration, VarDecl, VariableType};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_ir::{Addr, Region, Task};
use ironplc_wasm_symbols::flags;

use crate::leaves::leaves;
use crate::lower::{Lowerer, Section};
use crate::pou::{lay_out, program_instance, scope};

/// Name of the task that runs the programs no task names.
pub const DEFAULT_TASK: &str = "DEFAULT";

pub(crate) fn lower_project(l: &mut Lowerer) -> Result<(), Diagnostic> {
    let config = l.lib.elements.iter().find_map(|e| match e {
        LibraryElementKind::ConfigurationDeclaration(c) => Some(c),
        _ => None,
    });
    globals(l, config)?;
    match config {
        Some(c) => configured(l, c)?,
        None => unconfigured(l)?,
    }
    l.drain()
}

fn globals(l: &mut Lowerer, config: Option<&ConfigurationDeclaration>) -> Result<(), Diagnostic> {
    let mut decls: Vec<VarDecl> = vec![];
    let uptime = l.ctx.compiler_options().allow_system_uptime_global;
    if uptime {
        for g in &SYSTEM_UPTIME_GLOBALS {
            decls.push(VarDecl::simple(g.name, g.type_name).with_type(VariableType::Global));
        }
    }
    for e in &l.lib.elements {
        if let LibraryElementKind::GlobalVarDeclarations(d) = e {
            decls.extend(d.iter().cloned());
        }
    }
    if let Some(c) = config {
        decls.extend(c.global_var.iter().cloned());
        for r in &c.resource_decl {
            decls.extend(r.global_vars.iter().cloned());
        }
    }
    if decls.is_empty() {
        return Ok(());
    }
    let laid = lay_out(l, &decls, false)?;
    if let Some(d) = decls.first() {
        laid.no_ref_inits(&d.identifier.span())?;
    }
    let image = laid.image.finish();
    let o = l.object("GLOBALS", Region::Static, image.bytes, image.align);
    l.globals = scope(l, &laid.members, |m| Addr::object(o, m.offset));
    if uptime {
        let at = |i: usize| Addr::object(o, laid.members[i].offset);
        l.uptime = Some((at(0), at(1)));
    }
    for m in &laid.members {
        let mut f = flags::GLOBAL;
        if m.constant {
            f |= flags::CONSTANT;
        }
        leaves(l, &m.name, m, o, f)?;
    }
    Ok(())
}

fn find_program<'a>(
    programs: &[&'a ProgramDeclaration],
    name: &str,
    span: &SourceSpan,
) -> Result<&'a ProgramDeclaration, Diagnostic> {
    programs
        .iter()
        .find(|p| p.name.to_string().eq_ignore_ascii_case(name))
        .copied()
        .ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(span.clone(), format!("No program {name}")))
        })
}

fn configured(l: &mut Lowerer, c: &ConfigurationDeclaration) -> Result<(), Diagnostic> {
    let programs = l.programs();
    let several = c.resource_decl.len() > 1;
    let mut default = Task {
        name: DEFAULT_TASK.into(),
        interval_ns: 0,
        priority: 0,
        programs: vec![],
    };
    let mut tasks = vec![];
    for r in &c.resource_decl {
        let first = tasks.len();
        for t in &r.tasks {
            if t.single.is_some() {
                return Err(Diagnostic::not_implemented(Label::span(
                    t.name.span(),
                    "A task with SINGLE in the WebAssembly target",
                )));
            }
            let interval_ns = t
                .interval
                .as_ref()
                .map(|i| i.interval.whole_nanoseconds().max(0) as u64)
                .unwrap_or(0);
            tasks.push(Task {
                name: t.name.to_string().to_uppercase(),
                interval_ns,
                priority: t.priority,
                programs: vec![],
            });
        }
        for p in &r.programs {
            let decl = find_program(&programs, &p.type_name.to_string(), &p.name.span())?;
            let mut path = p.name.to_string().to_uppercase();
            if several {
                path = format!("{}.{path}", r.name.to_string().to_uppercase());
            }
            let inst = instance(l, decl, &path)?;
            let task = p.task_name.as_ref().and_then(|tn| {
                tasks[first..]
                    .iter()
                    .position(|t| t.name.eq_ignore_ascii_case(&tn.to_string()))
                    .map(|i| i + first)
            });
            match task {
                Some(i) => tasks[i].programs.push((inst, path)),
                None => default.programs.push((inst, path)),
            }
        }
    }
    if !default.programs.is_empty() || tasks.is_empty() {
        tasks.push(default);
    }
    l.m.tasks = tasks;
    Ok(())
}

fn unconfigured(l: &mut Lowerer) -> Result<(), Diagnostic> {
    let mut task = Task {
        name: DEFAULT_TASK.into(),
        interval_ns: 0,
        priority: 0,
        programs: vec![],
    };
    for decl in l.programs() {
        let path = decl.name.to_string().to_uppercase();
        let inst = instance(l, decl, &path)?;
        task.programs.push((inst, path));
    }
    l.m.tasks = vec![task];
    Ok(())
}

/// Lowers a program instance and publishes its variables.
fn instance(l: &mut Lowerer, decl: &ProgramDeclaration, path: &str) -> Result<u32, Diagnostic> {
    let inst = program_instance(l, decl, path)?;
    for m in &inst.members {
        let f = match m.section {
            Section::Input => flags::INPUT,
            Section::Output => flags::OUTPUT,
            _ => 0,
        } | if m.constant { flags::CONSTANT } else { 0 };
        let object = m.location.as_ref().map(|(_, o)| *o).unwrap_or(inst.object);
        leaves(l, &format!("{}.{}", inst.path, m.name), m, object, f)?;
    }
    Ok(inst.func)
}
