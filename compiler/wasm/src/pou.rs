//! Function blocks, functions and program instances: their layouts and
//! their bodies.

use std::collections::HashMap;
use std::rc::Rc;

use ironplc_dsl::common::{
    DeclarationQualifier, FunctionBlockBodyKind, FunctionBlockDeclaration, FunctionDeclaration,
    FunctionReturnType, InitialValueAssignmentKind, LocationPrefix, ProgramDeclaration,
    ReferenceInitialValue, TypeName, VarDecl, VariableIdentifier, VariableType,
};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_ir::{Addr, Base, FuncId, FuncKind, ObjId, Region, Stmt, StmtKind};

use crate::body::Body;
use crate::layout::{string_cap, Image};
use crate::lower::{FbType, FuncType, Lowerer, Member, Section, Var};
use crate::types::{Field, StructTy, Ty, U32};

fn nyi(span: &SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(
        span.clone(),
        format!("{what} in the WebAssembly target"),
    ))
}

fn section(vt: &VariableType) -> Option<Section> {
    match vt {
        VariableType::Var => Some(Section::Var),
        VariableType::VarTemp => Some(Section::Temp),
        VariableType::Input => Some(Section::Input),
        VariableType::Output => Some(Section::Output),
        VariableType::InOut => Some(Section::InOut),
        VariableType::Global => Some(Section::Global),
        // External variables are the globals of the same name.
        VariableType::External | VariableType::Access => None,
    }
}

/// Where the members of a list of declarations go.
pub(crate) struct Laid {
    pub members: Vec<Member>,
    pub image: Image,
    pub temps: Image,
    /// References initialized to the address of a variable, which is known
    /// only once the code generator has placed the objects.
    pub ref_inits: Vec<(String, ironplc_dsl::textual::Variable)>,
}

impl Laid {
    pub fn no_ref_inits(&self, span: &SourceSpan) -> Result<(), Diagnostic> {
        match self.ref_inits.is_empty() {
            true => Ok(()),
            false => Err(nyi(span, "A reference initialized with REF() here")),
        }
    }
}

/// Lays out declarations: temporaries apart, located variables in the image
/// areas, `VAR_IN_OUT` as 4-byte address slots.
pub(crate) fn lay_out(
    l: &mut Lowerer,
    decls: &[VarDecl],
    temps_apart: bool,
) -> Result<Laid, Diagnostic> {
    let mut laid = Laid {
        members: vec![],
        image: Image::default(),
        temps: Image::default(),
        ref_inits: vec![],
    };
    for d in decls {
        let Some(section) = section(&d.var_type) else {
            continue;
        };
        let span = d.identifier.span();
        let (name, location) = match &d.identifier {
            VariableIdentifier::Symbol(id) => (id.to_string().to_uppercase(), None),
            VariableIdentifier::Direct(dv) => {
                let name = dv
                    .name
                    .as_ref()
                    .map(|n| n.to_string().to_uppercase())
                    .ok_or_else(|| nyi(&span, "A located variable without a name"))?;
                (name, Some(&dv.address_assignment))
            }
        };
        if let InitialValueAssignmentKind::Reference(r) = &d.initializer {
            if let Some(ReferenceInitialValue::Ref(v)) = &r.initial_value {
                laid.ref_inits.push((name.clone(), v.clone()));
            }
        }
        let ty = l.decl_ty(&d.initializer, &span)?;
        let init = l.init_image(&d.initializer, &ty, &span)?;
        let (size, align) = l.size_align(&ty);
        let constant = d.qualifier == DeclarationQualifier::Constant;
        let (offset, location) = if let Some(a) = location {
            let prefix = match a.location {
                LocationPrefix::I => 'I',
                LocationPrefix::Q => 'Q',
                LocationPrefix::M => 'M',
            };
            let o = l.area(prefix);
            let off = append(l, o, &init, align);
            (off, Some((a.to_string(), o)))
        } else if section == Section::InOut {
            (laid.image.alloc(4, 4), None)
        } else if section == Section::Temp && temps_apart {
            let off = laid.temps.alloc(size, align);
            laid.temps.put(off, &init);
            (off, None)
        } else {
            let off = laid.image.alloc(size, align);
            laid.image.put(off, &init);
            (off, None)
        };
        laid.members.push(Member {
            name,
            section,
            offset,
            ty,
            span,
            constant,
            location,
        });
    }
    Ok(laid)
}

/// Appends an initial image to an object; returns its offset.
pub(crate) fn append(l: &mut Lowerer, o: ObjId, bytes: &[u8], align: u32) -> u32 {
    let obj = &mut l.m.objects[o as usize];
    let align = align.max(1) as usize;
    let off = obj.init.len().div_ceil(align) * align;
    obj.init.resize(off, 0);
    obj.init.extend_from_slice(bytes);
    obj.align = obj.align.max(align as u32);
    off as u32
}

/// The variables of a body: its members at their addresses, then the
/// globals it does not hide.
pub(crate) fn scope(
    l: &Lowerer,
    members: &[Member],
    base: impl Fn(&Member) -> Addr,
) -> HashMap<String, Var> {
    let mut vars = l.globals.clone();
    for m in members {
        let addr = match &m.location {
            Some((_, o)) => Addr::object(*o, m.offset),
            None => base(m),
        };
        vars.insert(
            m.name.clone(),
            Var {
                addr,
                ty: m.ty.clone(),
                by_ref: m.section == Section::InOut,
            },
        );
    }
    vars
}

pub(crate) fn declare_fb(
    l: &mut Lowerer,
    decl: &FunctionBlockDeclaration,
) -> Result<FbType, Diagnostic> {
    if decl.oop.is_some() || !decl.methods.is_empty() {
        return Err(nyi(&decl.name.span(), "An object-oriented function block"));
    }
    if !decl.edge_variables.is_empty() {
        return Err(nyi(&decl.name.span(), "An edge-triggered input"));
    }
    let name = decl.name.to_string().to_uppercase();
    let func = l.declare_function(&name, FuncKind::FunctionBlock, None);
    let laid = lay_out(l, &decl.variables, true)?;
    laid.no_ref_inits(&decl.name.span())?;
    let temps = if laid.temps.bytes.is_empty() {
        None
    } else {
        let temps: Vec<Member> = laid
            .members
            .iter()
            .filter(|m| m.section == Section::Temp)
            .cloned()
            .collect();
        let image = laid.temps.finish();
        let o = l.object(
            &format!("{name}.TEMP"),
            Region::Static,
            image.bytes,
            image.align,
        );
        Some((o, temps))
    };
    let members: Vec<Member> = laid
        .members
        .into_iter()
        .filter(|m| m.section != Section::Temp)
        .collect();
    Ok(instance_type(name, func, members, laid.image, temps))
}

/// A function block type from its members and the image of an instance.
pub(crate) fn instance_type(
    name: String,
    func: FuncId,
    members: Vec<Member>,
    image: Image,
    temps: Option<(ObjId, Vec<Member>)>,
) -> FbType {
    let image = image.finish();
    let fields = members
        .iter()
        .map(|m| Field {
            name: m.name.clone(),
            offset: m.offset,
            ty: if m.section == Section::InOut {
                Ty::scalar(U32, "UDINT")
            } else {
                m.ty.clone()
            },
        })
        .collect();
    let layout = Rc::new(StructTy {
        name: name.clone(),
        fields,
        size: image.bytes.len() as u32,
        align: image.align.max(1),
    });
    FbType {
        name,
        func,
        members,
        layout,
        init: image.bytes,
        temps,
    }
}

pub(crate) fn define_fb(
    l: &mut Lowerer,
    decl: &FunctionBlockDeclaration,
    fb: &FbType,
) -> Result<(), Diagnostic> {
    let mut vars = scope(l, &fb.members, |m| Addr {
        base: Base::SelfPart(0),
        offset: m.offset,
    });
    let mut prologue = vec![];
    if let Some((o, temps)) = &fb.temps {
        vars.extend(scope(l, temps, |m| Addr::object(*o, m.offset)));
        prologue.push(reset(*o));
    }
    let (body, temps) = lower_body(l, vars, &decl.body, prologue)?;
    l.define_function(fb.func, body, temps, &decl.name.span());
    Ok(())
}

fn reset(o: ObjId) -> Stmt {
    Stmt {
        kind: StmtKind::Reset(o),
        span: Default::default(),
    }
}

fn lower_body(
    l: &mut Lowerer,
    vars: HashMap<String, Var>,
    body: &FunctionBlockBodyKind,
    prologue: Vec<Stmt>,
) -> Result<(Vec<Stmt>, Vec<ironplc_wasm_ir::Scalar>), Diagnostic> {
    let mut b = Body::new(l, vars);
    let mut out = prologue;
    match body {
        FunctionBlockBodyKind::Statements(s) => out.extend(b.stmts(&s.body)?),
        FunctionBlockBodyKind::Empty => {}
        FunctionBlockBodyKind::Sfc(_) => {
            return Err(Diagnostic::not_implemented(Label::file(
                ironplc_dsl::core::FileId::default(),
                "Sequential function charts in the WebAssembly target",
            )))
        }
    }
    Ok((out, b.temps))
}

pub(crate) fn declare_function(
    l: &mut Lowerer,
    decl: &FunctionDeclaration,
) -> Result<FuncType, Diagnostic> {
    if !decl.edge_variables.is_empty() {
        return Err(nyi(&decl.name.span(), "An edge-triggered input"));
    }
    let name = decl.name.to_string().to_uppercase();
    let mut laid = lay_out(l, &decl.variables, false)?;
    laid.no_ref_inits(&decl.name.span())?;
    let result = match &decl.return_type {
        FunctionReturnType::Named(tn) if is_void(tn) => None,
        FunctionReturnType::Named(tn) => Some(l.ty_of_name(tn)?),
        FunctionReturnType::String(s) => Some(Ty::Str {
            cap: string_cap(s.length.as_ref(), &decl.name.span())?,
            wide: false,
        }),
        FunctionReturnType::WString(s) => Some(Ty::Str {
            cap: string_cap(s.length.as_ref(), &decl.name.span())?,
            wide: true,
        }),
    };
    let result = match result {
        Some(ty) => {
            let (size, align) = l.size_align(&ty);
            let off = laid.image.alloc(size, align);
            let init = l.default_image(&ty)?;
            laid.image.put(off, &init);
            Some((off, ty))
        }
        None => None,
    };
    let image = laid.image.finish();
    let frame = l.object(&name, Region::Static, image.bytes, image.align);
    let func = l.declare_function(&name, FuncKind::Function, Some(frame));
    Ok(FuncType {
        name,
        func,
        frame,
        members: laid.members,
        result,
    })
}

fn is_void(tn: &TypeName) -> bool {
    tn.to_string().eq_ignore_ascii_case("VOID")
}

pub(crate) fn define_function(
    l: &mut Lowerer,
    decl: &FunctionDeclaration,
    f: &FuncType,
) -> Result<(), Diagnostic> {
    let frame = f.frame;
    let mut vars = scope(l, &f.members, |m| Addr::object(frame, m.offset));
    if let Some((off, ty)) = &f.result {
        vars.insert(
            f.name.clone(),
            Var {
                addr: Addr::object(frame, *off),
                ty: ty.clone(),
                by_ref: false,
            },
        );
    }
    let mut b = Body::new(l, vars);
    let body = b.stmts(&decl.body)?;
    let temps = b.temps;
    l.define_function(f.func, body, temps, &decl.name.span());
    Ok(())
}

/// A program instance: its object, its members and its IR function.
pub(crate) struct ProgramInstance {
    pub path: String,
    pub func: FuncId,
    pub object: ObjId,
    pub members: Vec<Member>,
}

/// Lays out one instance of a program and lowers its body with absolute
/// addresses.
pub(crate) fn program_instance(
    l: &mut Lowerer,
    decl: &ProgramDeclaration,
    path: &str,
) -> Result<ProgramInstance, Diagnostic> {
    let laid = lay_out(l, &decl.variables, true)?;
    if laid.members.iter().any(|m| m.section == Section::InOut) {
        return Err(nyi(&decl.name.span(), "A program with VAR_IN_OUT"));
    }
    let image = laid.image.finish();
    let object = l.object(path, Region::Static, image.bytes, image.align);
    // Named after the instance: the code generator exports each program
    // as `plc_program_<name>` (ABI-014), so two instances of one program
    // type need two names.
    let func = l.declare_function(path, FuncKind::Program, None);
    let mut vars = scope(l, &laid.members, |m| Addr::object(object, m.offset));
    let mut prologue = crate::stdfb::uptime(l);
    if !laid.temps.bytes.is_empty() {
        let image = laid.temps.finish();
        let o = l.object(
            &format!("{path}.TEMP"),
            Region::Static,
            image.bytes,
            image.align,
        );
        let temps: Vec<Member> = laid
            .members
            .iter()
            .filter(|m| m.section == Section::Temp)
            .cloned()
            .collect();
        vars.extend(scope(l, &temps, |m| Addr::object(o, m.offset)));
        prologue.push(reset(o));
    }
    if !laid.ref_inits.is_empty() {
        let first = first_cycle(l, path, &vars, &laid.ref_inits)?;
        prologue.push(first);
    }
    let (body, temps) = lower_body(l, vars, &decl.body, prologue)?;
    l.define_function(func, body, temps, &decl.name.span());
    Ok(ProgramInstance {
        path: path.to_string(),
        func,
        object,
        members: laid
            .members
            .into_iter()
            .filter(|m| m.section != Section::Temp)
            .collect(),
    })
}

/// `IF NOT done THEN done := TRUE; r := REF(x); ... END_IF`: the
/// references initialized with the address of a variable, on the first
/// cycle after `plc_init` (which clears `done`).
fn first_cycle(
    l: &mut Lowerer,
    path: &str,
    vars: &HashMap<String, Var>,
    inits: &[(String, ironplc_dsl::textual::Variable)],
) -> Result<Stmt, Diagnostic> {
    use crate::body::{ex, not, stmt};
    use crate::types::{BOOL, U32};
    use ironplc_wasm_ir::{Const, ExprKind, Place};
    let done = l.object(&format!("{path}.INIT"), Region::Static, vec![0], 1);
    let flag = Place {
        addr: Addr::object(done, 0),
        ty: BOOL,
    };
    let mut body = vec![stmt(StmtKind::Assign(
        flag.clone(),
        ex(ExprKind::Const(Const::Bool(true)), BOOL),
    ))];
    let mut b = Body::new(l, vars.clone());
    for (name, target) in inits {
        let slot = vars[name].addr.clone();
        let (addr, _) = b.place(target)?;
        body.push(stmt(StmtKind::Assign(
            Place {
                addr: slot,
                ty: U32,
            },
            ex(ExprKind::AddrOf(addr), U32),
        )));
    }
    let loaded = ex(ExprKind::Load(flag), BOOL);
    Ok(stmt(StmtKind::If(not(loaded), body, vec![])))
}
