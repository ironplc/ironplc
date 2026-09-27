//! The lowering of an analysed library to one IR module.
//!
//! [`Lowerer`] owns the module being built. Function blocks and functions
//! are declared on first use (their layout and their IR function index) and
//! their bodies are lowered from a work list afterwards, so the order of the
//! library elements does not matter.

use std::collections::HashMap;
use std::rc::Rc;

use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::{
    DataTypeDeclarationKind, FunctionBlockDeclaration, FunctionDeclaration, Library,
    LibraryElementKind, ProgramDeclaration,
};
use ironplc_dsl::core::{FileId, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_ir::{
    Addr, FuncId, FuncKind, Function, Label as IrLabel, Module, ObjId, Object, Region, SiteId,
    Span, Stmt,
};

use crate::types::{StructTy, Ty};

/// Section of a member of an instance or a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Input,
    Output,
    InOut,
    Var,
    Temp,
    Global,
    /// Internal state of a standard function block (SYM-012).
    Hidden,
}

/// A variable as the code of a POU sees it.
#[derive(Clone, Debug)]
pub(crate) struct Var {
    /// Where it is stored; for a `VAR_IN_OUT`, the slot holding its address.
    pub addr: Addr,
    pub ty: Ty,
    /// The slot holds the address of the variable (`VAR_IN_OUT`).
    pub by_ref: bool,
}

impl Var {
    /// The address of the value.
    pub fn place(&self) -> Addr {
        if self.by_ref {
            Addr {
                base: ironplc_wasm_ir::Base::Deref(Box::new(self.addr.clone())),
                offset: 0,
            }
        } else {
            self.addr.clone()
        }
    }
}

/// A member laid out in an instance, a frame or an object.
#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub name: String,
    pub section: Section,
    pub offset: u32,
    pub ty: Ty,
    pub span: SourceSpan,
    pub constant: bool,
    /// Direct address of a located variable, with its object.
    pub location: Option<(String, ObjId)>,
}

/// A function block type: standard or declared in the library.
#[derive(Debug)]
pub(crate) struct FbType {
    pub name: String,
    pub func: FuncId,
    pub members: Vec<Member>,
    pub layout: Rc<StructTy>,
    /// Initial image of an instance.
    pub init: Vec<u8>,
    /// Frame of the `VAR_TEMP` variables.
    pub temps: Option<(ObjId, Vec<Member>)>,
}

impl FbType {
    pub fn member(&self, name: &str) -> Option<&Member> {
        self.members
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }
}

/// A function declared in the library.
#[derive(Debug)]
pub(crate) struct FuncType {
    pub name: String,
    pub func: FuncId,
    pub frame: ObjId,
    /// Inputs, outputs, in-outs, locals and temporaries of the frame.
    pub members: Vec<Member>,
    /// Offset and type of the result.
    pub result: Option<(u32, Ty)>,
}

enum Pending<'a> {
    Fb(&'a FunctionBlockDeclaration, Rc<FbType>),
    Func(&'a FunctionDeclaration, Rc<FuncType>),
}

/// The state of the lowering.
pub(crate) struct Lowerer<'a> {
    pub lib: &'a Library,
    pub ctx: &'a SemanticContext,
    pub m: Module,
    pub bounds_checks: bool,
    /// Calls of `plc_rt.debug_hook` before each statement.
    pub debug_hooks: bool,
    files: Vec<FileId>,
    pub(crate) fbs: HashMap<String, Rc<FbType>>,
    pub(crate) funcs: HashMap<String, Rc<FuncType>>,
    pub(crate) globals: HashMap<String, Var>,
    pub(crate) enums: crate::enums::Enums,
    areas: HashMap<char, ObjId>,
    pending: Vec<Pending<'a>>,
    next_label: IrLabel,
    /// `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME`, when the program uses
    /// them.
    pub(crate) uptime: Option<(Addr, Addr)>,
    /// Constant objects of string literals, by content.
    pub(crate) literals: HashMap<(Vec<u8>, bool), ObjId>,
}

impl<'a> Lowerer<'a> {
    pub fn new(lib: &'a Library, ctx: &'a SemanticContext, files: Vec<FileId>) -> Self {
        Lowerer {
            lib,
            ctx,
            m: Module::default(),
            bounds_checks: true,
            debug_hooks: false,
            files,
            fbs: HashMap::new(),
            funcs: HashMap::new(),
            globals: HashMap::new(),
            enums: crate::enums::Enums::new(lib),
            areas: HashMap::new(),
            pending: vec![],
            next_label: 0,
            literals: HashMap::new(),
            uptime: None,
        }
    }

    /// The IR span of a source span; files not given as sources are added.
    pub fn span(&mut self, s: &SourceSpan) -> Span {
        let file = match self.files.iter().position(|f| *f == s.file_id) {
            Some(i) => i,
            None => {
                self.files.push(s.file_id.clone());
                self.files.len() - 1
            }
        };
        Span {
            file: file as u32,
            start: s.start as u32,
            end: s.end as u32,
        }
    }

    /// The files named by the spans, in the order of their index.
    pub fn files(&self) -> &[FileId] {
        &self.files
    }

    /// A new site of the symbol map.
    pub fn site(&mut self, s: &SourceSpan) -> SiteId {
        let span = self.span(s);
        self.m.sites.push(span);
        (self.m.sites.len() - 1) as SiteId
    }

    pub fn label(&mut self) -> IrLabel {
        self.next_label += 1;
        self.next_label
    }

    /// A new object.
    pub fn object(&mut self, name: &str, region: Region, init: Vec<u8>, align: u32) -> ObjId {
        self.m.objects.push(Object {
            name: name.to_string(),
            region,
            align: align.max(1),
            init,
        });
        (self.m.objects.len() - 1) as ObjId
    }

    /// A function whose body is lowered later.
    pub fn declare_function(&mut self, name: &str, kind: FuncKind, frame: Option<ObjId>) -> FuncId {
        self.m.functions.push(Function {
            name: name.to_uppercase(),
            kind,
            frame,
            temps: vec![],
            body: vec![],
            span: Span::default(),
        });
        (self.m.functions.len() - 1) as FuncId
    }

    pub fn define_function(
        &mut self,
        id: FuncId,
        body: Vec<Stmt>,
        temps: Vec<ironplc_wasm_ir::Scalar>,
        span: &SourceSpan,
    ) {
        let span = self.span(span);
        let f = &mut self.m.functions[id as usize];
        f.body = body;
        f.temps = temps;
        f.span = span;
    }

    /// The object of the image area of a located variable (`%I`, `%Q`, `%M`).
    pub fn area(&mut self, prefix: char) -> ObjId {
        if let Some(o) = self.areas.get(&prefix) {
            return *o;
        }
        let region = match prefix {
            'I' => Region::Input,
            'Q' => Region::Output,
            _ => Region::Marker,
        };
        let o = self.object(&format!("%{prefix}"), region, vec![], 8);
        self.areas.insert(prefix, o);
        o
    }

    /// The function block type of that name, declared on first use.
    pub fn fb_type(&mut self, name: &str, span: &SourceSpan) -> Result<Rc<FbType>, Diagnostic> {
        let key = name.to_uppercase();
        if let Some(fb) = self.fbs.get(&key) {
            return Ok(fb.clone());
        }
        let fb = if let Some(decl) = self.fb_decl(&key) {
            let fb = Rc::new(crate::pou::declare_fb(self, decl)?);
            self.pending.push(Pending::Fb(decl, fb.clone()));
            fb
        } else if let Some(fb) = crate::stdfb::declare(self, &key)? {
            Rc::new(fb)
        } else {
            return Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                format!("Function block {name} in the WebAssembly target"),
            )));
        };
        self.fbs.insert(key, fb.clone());
        Ok(fb)
    }

    /// The function of that name declared in the library, declared on first
    /// use; `None` for a standard function.
    pub fn func_type(&mut self, name: &str) -> Result<Option<Rc<FuncType>>, Diagnostic> {
        let key = name.to_uppercase();
        if let Some(f) = self.funcs.get(&key) {
            return Ok(Some(f.clone()));
        }
        let Some(decl) = self.func_decl(&key) else {
            return Ok(None);
        };
        let f = Rc::new(crate::pou::declare_function(self, decl)?);
        self.pending.push(Pending::Func(decl, f.clone()));
        self.funcs.insert(key, f.clone());
        Ok(Some(f))
    }

    fn fb_decl(&self, key: &str) -> Option<&'a FunctionBlockDeclaration> {
        self.lib.elements.iter().find_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb)
                if fb.name.to_string().eq_ignore_ascii_case(key) =>
            {
                Some(fb)
            }
            _ => None,
        })
    }

    fn func_decl(&self, key: &str) -> Option<&'a FunctionDeclaration> {
        self.lib.elements.iter().find_map(|e| match e {
            LibraryElementKind::FunctionDeclaration(f)
                if f.name.to_string().eq_ignore_ascii_case(key) =>
            {
                Some(f)
            }
            _ => None,
        })
    }

    /// A type declaration of the library by name.
    pub fn type_decl(&self, name: &str) -> Option<&'a DataTypeDeclarationKind> {
        self.lib.elements.iter().find_map(|e| match e {
            LibraryElementKind::DataTypeDeclaration(d)
                if decl_name(d).eq_ignore_ascii_case(name) =>
            {
                Some(d)
            }
            _ => None,
        })
    }

    /// The program declarations of the library, in source order.
    pub fn programs(&self) -> Vec<&'a ProgramDeclaration> {
        let mut programs: Vec<&ProgramDeclaration> = self
            .lib
            .elements
            .iter()
            .filter_map(|e| match e {
                LibraryElementKind::ProgramDeclaration(p) => Some(p),
                _ => None,
            })
            .collect();
        programs.sort_by_key(|p| (p.name.span.file_id.to_string(), p.name.span.start));
        programs
    }

    /// Lowers the bodies of the declared function blocks and functions until
    /// none is left.
    pub fn drain(&mut self) -> Result<(), Diagnostic> {
        while let Some(p) = self.pending.pop() {
            match p {
                Pending::Fb(decl, fb) => crate::pou::define_fb(self, decl, &fb)?,
                Pending::Func(decl, f) => crate::pou::define_function(self, decl, &f)?,
            }
        }
        Ok(())
    }
}

fn decl_name(d: &DataTypeDeclarationKind) -> String {
    match d {
        DataTypeDeclarationKind::Enumeration(e) => e.type_name.to_string(),
        DataTypeDeclarationKind::Subrange(s) => s.type_name.to_string(),
        DataTypeDeclarationKind::Simple(s) => s.type_name.to_string(),
        DataTypeDeclarationKind::Array(a) => a.type_name.to_string(),
        DataTypeDeclarationKind::Structure(s) => s.type_name.to_string(),
        DataTypeDeclarationKind::StructureInitialization(s) => s.type_name.to_string(),
        DataTypeDeclarationKind::String(s) => s.type_name.to_string(),
        DataTypeDeclarationKind::Reference(r) => r.type_name.to_string(),
        _ => String::new(),
    }
}
