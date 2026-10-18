//! The text form of a whole lowered program, which lowering's tests of a
//! whole program compare against.
//!
//! The program prints as its types, its variables, each POU with its parts
//! and body, `init`, then the execution model's bindings. Each entry is on
//! one line, led by its id; a structure's, block's or program's fields and a
//! body's statements are on the lines after it, indented two spaces, and a
//! body closes with `end`. Statements print as [`Stmt`] does.

use std::fmt;

use super::ids::PouId;
use super::print::write_stmt;
use super::stmt::Stmt;
use super::types::{
    AddressArea, AddressSize, DirectAddress, Encoding, Field, Nullability, Retention, Section,
    TemporalKind, TypeEntry, TypeKind,
};
use super::variables::{Owner, PouKind, Variable};
use super::whole::Program;
use crate::execution::InstanceOf;

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("types\n")?;
        for (id, entry) in self.types().iter() {
            write!(f, "  {id}")?;
            write_type(f, entry)?;
            f.write_str("\n")?;
        }
        f.write_str("variables\n")?;
        for (id, variable) in self.variables().iter() {
            write!(f, "  {id}")?;
            write_variable(f, variable)?;
            f.write_str("\n")?;
        }
        for pou in self.pous().ids() {
            write_pou(f, self, pou)?;
        }
        f.write_str("init\n")?;
        write_body(f, self.init())?;
        f.write_str("end\n")?;
        write_bindings(f, self)
    }
}

fn write_type(f: &mut fmt::Formatter<'_>, entry: &TypeEntry) -> fmt::Result {
    if let Some(name) = &entry.name {
        write!(f, " \"{name}\"")?;
    }
    match &entry.kind {
        TypeKind::Bool => f.write_str(" bool"),
        TypeKind::Integer { width, signed } => {
            let sign = if *signed { "i" } else { "u" };
            write!(f, " {sign}{}", width.bits())
        }
        TypeKind::BitString { width } => write!(f, " bits{}", width.bits()),
        TypeKind::Real { width } => write!(f, " f{}", width.bits()),
        TypeKind::Temporal { kind, width } => {
            let kind = match kind {
                TemporalKind::Time => "time",
                TemporalKind::Date => "date",
                TemporalKind::TimeOfDay => "tod",
                TemporalKind::DateAndTime => "dt",
            };
            write!(f, " {kind}{}", width.bits())
        }
        TypeKind::Enumeration { values, underlying } => {
            write!(f, " enum {underlying}")?;
            for value in values {
                write!(f, " {}={}", value.name, value.ordinal)?;
            }
            Ok(())
        }
        TypeKind::Subrange { base, min, max } => write!(f, " subrange {base} {min}..{max}"),
        TypeKind::String(shape) => {
            let encoding = match shape.encoding {
                Encoding::Narrow => "string",
                Encoding::Wide => "wstring",
            };
            write!(f, " {encoding} {}", shape.capacity)
        }
        TypeKind::Array {
            element,
            dimensions,
        } => {
            let dimensions: Vec<String> = dimensions
                .iter()
                .map(|(lower, upper)| format!("{lower}..{upper}"))
                .collect();
            write!(f, " array [{}] of {element}", dimensions.join(", "))
        }
        TypeKind::Structure { fields } => {
            f.write_str(" struct")?;
            write_fields(f, fields)
        }
        TypeKind::FunctionBlock { fields, body, base } => {
            write!(f, " function-block {body}")?;
            if let Some(base) = base {
                write!(f, " extends {base}")?;
            }
            write_fields(f, fields)
        }
        TypeKind::Program { fields, body } => {
            write!(f, " program {body}")?;
            write_fields(f, fields)
        }
        TypeKind::Reference {
            target,
            nullability,
        } => match nullability {
            Nullability::Nullable => write!(f, " ref {target}"),
            Nullability::NonNull => write!(f, " ref non-null {target}"),
        },
    }
}

/// Each field on a line of its own, after the type's.
fn write_fields(f: &mut fmt::Formatter<'_>, fields: &[Field]) -> fmt::Result {
    for (index, field) in fields.iter().enumerate() {
        write!(f, "\n    f{index} {} : {}", field.name, field.ty)?;
        if let Some(section) = field.section {
            write!(f, " {}", section_text(section))?;
        }
        write_declared(f, field.location.as_ref(), field.retention)?;
    }
    Ok(())
}

fn write_variable(f: &mut fmt::Formatter<'_>, variable: &Variable) -> fmt::Result {
    write!(
        f,
        " {} : {} {}",
        variable.name(),
        variable.ty(),
        section_text(variable.section())
    )?;
    match variable.owner() {
        Owner::Global => {}
        Owner::Pou(pou) => write!(f, " of {pou}")?,
    }
    write_declared(f, variable.location(), variable.retention())
}

/// The direct address and retention of a declaration, when it states them.
fn write_declared(
    f: &mut fmt::Formatter<'_>,
    location: Option<&DirectAddress>,
    retention: Retention,
) -> fmt::Result {
    if let Some(location) = location {
        write!(f, " at {}", address_text(location))?;
    }
    match retention {
        Retention::None => Ok(()),
        Retention::Retain => f.write_str(" retain"),
        Retention::NonRetain => f.write_str(" non-retain"),
        Retention::Persistent => f.write_str(" persistent"),
    }
}

/// `address` as IEC 61131-3 writes it: `%IX0.1`, `%QW4`, `%M*`.
fn address_text(address: &DirectAddress) -> String {
    let area = match address.area {
        AddressArea::Input => "I",
        AddressArea::Output => "Q",
        AddressArea::Memory => "M",
    };
    let size = match address.size {
        AddressSize::Bit => "X",
        AddressSize::Byte => "B",
        AddressSize::Word => "W",
        AddressSize::DoubleWord => "D",
        AddressSize::LongWord => "L",
        AddressSize::Unspecified => return format!("%{area}*"),
    };
    let path: Vec<String> = address.path.iter().map(ToString::to_string).collect();
    format!("%{area}{size}{}", path.join("."))
}

fn section_text(section: Section) -> &'static str {
    match section {
        Section::Global => "global",
        Section::Input => "input",
        Section::Output => "output",
        Section::InOut => "in-out",
        Section::Local => "local",
        Section::Temporary => "temporary",
        Section::Instance => "instance",
        Section::Internal => "internal",
    }
}

/// The POU `pou`: its kind, name and parts on one line, then its body.
fn write_pou(f: &mut fmt::Formatter<'_>, program: &Program, pou: PouId) -> fmt::Result {
    let pous = program.pous();
    let (Some(name), Some(parts)) = (pous.name(pou), pous.parts(pou)) else {
        return Err(fmt::Error);
    };
    let kind = match parts.kind {
        PouKind::Program => "program",
        PouKind::Function => "function",
        PouKind::FunctionBlock => "function-block",
        PouKind::Method => "method",
    };
    write!(f, "{pou} {kind} {name}")?;
    if let Some(block) = pous.method_of(pou) {
        write!(f, " of {block}")?;
    }
    if let Some(instance) = parts.instance {
        write!(f, " instance {instance}")?;
    }
    if let Some(signature) = pous.function(pou) {
        write_list(f, " parameters", signature.parameters())?;
        if let Some(result) = signature.result() {
            write!(f, " result {result}")?;
        }
    }
    write_list(f, " locals", &parts.locals)?;
    f.write_str("\n")?;
    write_body(f, pous.body(pou).unwrap_or_default())?;
    f.write_str("end\n")
}

/// `label` and each of `items`, separated by commas; nothing when there are
/// none.
fn write_list<T: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    label: &str,
    items: &[T],
) -> fmt::Result {
    if items.is_empty() {
        return Ok(());
    }
    let items: Vec<String> = items.iter().map(ToString::to_string).collect();
    write!(f, "{label} {}", items.join(", "))
}

/// Each statement of a body, indented one level, each on its own line.
fn write_body(f: &mut fmt::Formatter<'_>, body: &[Stmt]) -> fmt::Result {
    for stmt in body {
        write_stmt(f, stmt, 1)?;
        f.write_str("\n")?;
    }
    Ok(())
}

/// What each entry of the execution model is bound to, in the model's order.
fn write_bindings(f: &mut fmt::Formatter<'_>, program: &Program) -> fmt::Result {
    let (execution, bindings) = (program.execution(), program.bindings());
    for (id, program_type) in execution.programs() {
        if let Some(body) = bindings.program(id) {
            writeln!(f, "program {} {body}", program_type.name)?;
        }
    }
    for (id, global) in execution.globals() {
        if let Some(var) = bindings.global(id) {
            writeln!(f, "global {} {var}", global.name)?;
        }
    }
    for (position, instance) in execution.instances().enumerate() {
        let name = instance
            .name
            .as_ref()
            .map_or_else(|| "(implicit)".to_string(), ToString::to_string);
        match (&instance.program, bindings.instance(position)) {
            (InstanceOf::Program(_), Some(var)) => writeln!(f, "instance {name} {var}")?,
            (InstanceOf::Unresolved(type_name), _) => {
                writeln!(f, "instance {name} unresolved {type_name}")?
            }
            (InstanceOf::Program(_), None) => return Err(fmt::Error),
        }
    }
    Ok(())
}
