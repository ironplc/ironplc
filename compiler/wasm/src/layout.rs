//! Types of declarations, sizes, and initial images.
//!
//! Every variable has a static address, so every initial value is known at
//! compile time and written into the initial image of its object; `plc_init`
//! copies the images (ABI-011) and a function frame is reset to its image
//! before each call.

use std::rc::Rc;

use ironplc_analyzer::IntermediateType;
use ironplc_container::DEFAULT_STRING_MAX_LENGTH;
use ironplc_dsl::common::{
    ArrayElementType, ArrayInitialElementKind, ArraySpecificationKind, ConstantKind,
    DataTypeDeclarationKind, InitialValueAssignmentKind, IntegerRef, SpecificationKind, StringType,
    StructInitialValueAssignmentKind, StructureElementInit, TypeName,
};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_ir::Scalar;

use crate::lower::Lowerer;
use crate::types::{elementary, int_bytes, real_bytes, ArrayTy, Field, StructTy, Ty, I32};

/// An initial image being laid out.
#[derive(Clone, Debug, Default)]
pub(crate) struct Image {
    pub bytes: Vec<u8>,
    pub align: u32,
}

impl Image {
    /// Reserves `size` bytes at `align`, zeroed; returns their offset.
    pub fn alloc(&mut self, size: u32, align: u32) -> u32 {
        let align = align.max(1);
        let off = (self.bytes.len() as u32).div_ceil(align) * align;
        self.bytes.resize((off + size) as usize, 0);
        self.align = self.align.max(align);
        off
    }

    pub fn put(&mut self, off: u32, data: &[u8]) {
        let off = off as usize;
        self.bytes[off..off + data.len()].copy_from_slice(data);
    }

    /// The image padded to its alignment, as a structure is (ABI-042).
    pub fn finish(mut self) -> Image {
        let align = self.align.max(1) as usize;
        let size = self.bytes.len().div_ceil(align) * align;
        self.bytes.resize(size.max(1), 0);
        self
    }
}

fn nyi(span: &SourceSpan, what: &str) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(
        span.clone(),
        format!("{what} in the WebAssembly target"),
    ))
}

impl Lowerer<'_> {
    /// Size and alignment of a stored value.
    pub fn size_align(&self, ty: &Ty) -> (u32, u32) {
        match ty {
            Ty::Scalar { sc, .. } => (sc.size(), sc.size()),
            Ty::Ref(_) => (4, 4),
            Ty::Str { cap, wide } => {
                let unit = if *wide { 2 } else { 1 };
                ((cap + 1) * unit, unit)
            }
            Ty::Array(a) => {
                let (_, align) = self.size_align(&a.elem);
                (a.stride * a.count() as u32, align)
            }
            Ty::Struct(s) => (s.size, s.align),
            Ty::Fb(name) => self
                .fbs
                .get(name)
                .map(|fb| (fb.layout.size, fb.layout.align))
                .unwrap_or((0, 1)),
        }
    }

    /// The storage of a named type.
    pub fn ty_of_name(&mut self, tn: &TypeName) -> Result<Ty, Diagnostic> {
        let name = tn.to_string().to_uppercase();
        let it = self
            .ctx
            .types()
            .get(tn)
            .map(|a| a.representation.clone())
            .ok_or_else(|| nyi(&tn.span(), &format!("Type {name}")))?;
        let mut ty = self.ty_of_it(&it, &tn.span())?;
        match (&mut ty, &it) {
            (Ty::Struct(s), _) if s.name.is_empty() => {
                let mut named = (**s).clone();
                named.name = name;
                ty = Ty::Struct(Rc::new(named));
            }
            (
                Ty::Scalar { name: n, .. },
                IntermediateType::Enumeration { .. } | IntermediateType::Subrange { .. },
            ) => *n = name,
            _ => {}
        }
        Ok(ty)
    }

    /// The storage of a type of the analyzer.
    pub fn ty_of_it(&mut self, it: &IntermediateType, span: &SourceSpan) -> Result<Ty, Diagnostic> {
        if let Some(ty) = elementary(it) {
            return Ok(ty);
        }
        match it {
            IntermediateType::Array {
                element_type,
                dimensions,
            } => {
                let elem = self.ty_of_it(element_type, span)?;
                let dims = dimensions
                    .iter()
                    .map(|d| (d.lower as i64, d.upper as i64))
                    .collect();
                Ok(self.array(elem, dims))
            }
            IntermediateType::Structure { fields } => {
                let mut members = vec![];
                for f in fields {
                    members.push((
                        f.name.to_string().to_uppercase(),
                        self.ty_of_it(&f.field_type, span)?,
                    ));
                }
                Ok(Ty::Struct(Rc::new(self.structure(String::new(), members))))
            }
            IntermediateType::Reference { target_type } => {
                Ok(Ty::Ref(Box::new(self.ty_of_it(target_type, span)?)))
            }
            IntermediateType::FunctionBlock { name, .. } => {
                let fb = self.fb_type(name, span)?;
                Ok(Ty::Fb(fb.name.clone()))
            }
            _ => Err(nyi(span, "This type")),
        }
    }

    pub fn array(&self, elem: Ty, dims: Vec<(i64, i64)>) -> Ty {
        let (size, align) = self.size_align(&elem);
        let stride = size.div_ceil(align) * align;
        Ty::Array(Rc::new(ArrayTy { elem, dims, stride }))
    }

    /// A structure with its members at their natural alignment (ABI-042).
    pub fn structure(&self, name: String, members: Vec<(String, Ty)>) -> StructTy {
        let mut image = Image::default();
        let mut fields = vec![];
        for (n, ty) in members {
            let (size, align) = self.size_align(&ty);
            let offset = image.alloc(size, align);
            fields.push(Field {
                name: n,
                offset,
                ty,
            });
        }
        let image = image.finish();
        StructTy {
            name,
            fields,
            size: image.bytes.len() as u32,
            align: image.align.max(1),
        }
    }

    /// The storage of a declared variable.
    pub fn decl_ty(
        &mut self,
        init: &InitialValueAssignmentKind,
        span: &SourceSpan,
    ) -> Result<Ty, Diagnostic> {
        match init {
            InitialValueAssignmentKind::Simple(s) => self.ty_of_name(&s.type_name),
            InitialValueAssignmentKind::String(s) => Ok(Ty::Str {
                cap: string_cap(s.length.as_ref(), span)?,
                wide: s.width == StringType::WString,
            }),
            InitialValueAssignmentKind::EnumeratedValues(_) => Ok(Ty::scalar(I32, "DINT")),
            InitialValueAssignmentKind::EnumeratedType(e) => self.ty_of_name(&e.type_name),
            InitialValueAssignmentKind::FunctionBlock(f) => self.ty_of_name(&f.type_name),
            InitialValueAssignmentKind::FunctionBlockCall(f) => self.ty_of_name(&f.type_name),
            InitialValueAssignmentKind::Structure(s) => self.ty_of_name(&s.type_name),
            InitialValueAssignmentKind::Subrange(SpecificationKind::Named(tn)) => {
                self.ty_of_name(tn)
            }
            InitialValueAssignmentKind::Subrange(SpecificationKind::Inline(s)) => {
                let tn: TypeName = s.type_name.clone().into();
                self.ty_of_name(&tn)
            }
            InitialValueAssignmentKind::Array(a) => self.array_spec_ty(&a.spec, span),
            InitialValueAssignmentKind::Reference(r) => {
                let declaring = TypeName::from(format!("{}", span.start).as_str());
                let it = self
                    .ctx
                    .types()
                    .resolve_reference_target(&declaring, &r.target)?;
                Ok(Ty::Ref(Box::new(self.ty_of_it(&it, span)?)))
            }
            other => Err(nyi(
                span,
                &format!("A declaration of kind {}", kind_name(other)),
            )),
        }
    }

    fn array_spec_ty(
        &mut self,
        spec: &ArraySpecificationKind,
        span: &SourceSpan,
    ) -> Result<Ty, Diagnostic> {
        match spec {
            SpecificationKind::Named(tn) => self.ty_of_name(tn),
            SpecificationKind::Inline(sub) => {
                let elem = match &sub.type_name {
                    ArrayElementType::Named(tn) => self.ty_of_name(tn)?,
                    ArrayElementType::String(s) => Ty::Str {
                        cap: string_cap(s.length.as_ref(), span)?,
                        wide: false,
                    },
                    ArrayElementType::WString(s) => Ty::Str {
                        cap: string_cap(s.length.as_ref(), span)?,
                        wide: true,
                    },
                };
                let elem = match sub.ref_to {
                    Some(_) => Ty::Ref(Box::new(elem)),
                    None => elem,
                };
                let mut dims = vec![];
                for r in &sub.ranges {
                    dims.push((signed_ref(&r.start, span)?, signed_ref(&r.end, span)?));
                }
                Ok(self.array(elem, dims))
            }
        }
    }

    // ----- initial images

    /// The initial image of a value of type `ty` without an initializer:
    /// zeros, the defaults of structure members and the initial image of
    /// function block instances.
    pub fn default_image(&mut self, ty: &Ty) -> Result<Vec<u8>, Diagnostic> {
        let (size, _) = self.size_align(ty);
        let mut bytes = vec![0; size as usize];
        match ty {
            Ty::Scalar { name, sc } => {
                if let Some(d) = self.named_default(name)? {
                    bytes = d;
                } else if let Some(low) = self.subrange_low(name) {
                    // A subrange starts at its lower bound, as the bytecode
                    // initializes it.
                    bytes = int_bytes(low, *sc);
                }
            }
            Ty::Array(a) => {
                let elem = self.default_image(&a.elem)?;
                for i in 0..a.count() as usize {
                    let off = i * a.stride as usize;
                    bytes[off..off + elem.len()].copy_from_slice(&elem);
                }
            }
            Ty::Struct(s) => {
                for f in &s.fields {
                    let fb = self.default_image(&f.ty)?;
                    let off = f.offset as usize;
                    bytes[off..off + fb.len()].copy_from_slice(&fb);
                }
                self.struct_defaults(s, &mut bytes)?;
            }
            Ty::Fb(name) => {
                if let Some(fb) = self.fbs.get(name) {
                    bytes = fb.init.clone();
                }
            }
            Ty::Str { .. } | Ty::Ref(_) => {}
        }
        Ok(bytes)
    }

    /// The lower bound of a named subrange type.
    fn subrange_low(&self, name: &str) -> Option<i128> {
        let it = &self.ctx.types().get(&TypeName::from(name))?.representation;
        match it {
            IntermediateType::Subrange { min_value, .. } => Some(*min_value),
            _ => None,
        }
    }

    /// The default of a named elementary or enumerated type, when the type
    /// declaration gives one.
    fn named_default(&mut self, name: &str) -> Result<Option<Vec<u8>>, Diagnostic> {
        match self.type_decl(name) {
            Some(DataTypeDeclarationKind::Enumeration(_)) => {
                Ok(Some(int_bytes(self.enums.default(name) as i128, I32)))
            }
            Some(DataTypeDeclarationKind::Simple(s)) => {
                let ty = self.ty_of_name(&s.type_name)?;
                let init = s.spec_and_init.clone();
                match &init {
                    InitialValueAssignmentKind::Simple(inner) if inner.type_name == s.type_name => {
                        Ok(None)
                    }
                    _ => self.init_image(&init, &ty, &s.type_name.span()).map(Some),
                }
            }
            Some(DataTypeDeclarationKind::Subrange(s)) => match &s.default {
                Some(d) => {
                    let ty = self.ty_of_name(&s.type_name)?;
                    let v = if d.is_neg {
                        -(d.value.value as i128)
                    } else {
                        d.value.value as i128
                    };
                    Ok(ty.sc().map(|sc| int_bytes(v, sc)))
                }
                None => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn struct_defaults(&mut self, s: &StructTy, bytes: &mut [u8]) -> Result<(), Diagnostic> {
        let Some(DataTypeDeclarationKind::Structure(decl)) = self.type_decl(&s.name) else {
            return Ok(());
        };
        for e in &decl.elements {
            let Some(f) = s.field(&e.name.to_string()) else {
                continue;
            };
            if matches!(e.init, InitialValueAssignmentKind::None(_)) {
                continue;
            }
            let image = self.init_image(&e.init, &f.ty, &e.name.span())?;
            let off = f.offset as usize;
            bytes[off..off + image.len()].copy_from_slice(&image);
        }
        Ok(())
    }

    /// The initial image of a declared variable of type `ty`.
    pub fn init_image(
        &mut self,
        init: &InitialValueAssignmentKind,
        ty: &Ty,
        span: &SourceSpan,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = self.default_image(ty)?;
        match init {
            InitialValueAssignmentKind::Simple(s) => match &s.initial_value {
                Some(c) => bytes = self.const_image(c, ty, span)?,
                None => {
                    // The default of the named type, which the storage type
                    // of a member may have lost (a subrange member).
                    let named = self.ty_of_name(&s.type_name)?;
                    let image = self.default_image(&named)?;
                    if image.len() == bytes.len() {
                        bytes = image;
                    }
                }
            },
            InitialValueAssignmentKind::String(s) => {
                if let Some(v) = &s.initial_value {
                    bytes =
                        self.const_image(&ConstantKind::CharacterString(v.clone()), ty, span)?;
                }
            }
            InitialValueAssignmentKind::EnumeratedValues(e) => {
                if let Some(v) = &e.initial_value {
                    let o = self.enums.ordinal(v).unwrap_or(0);
                    bytes = int_bytes(o as i128, I32);
                }
            }
            InitialValueAssignmentKind::EnumeratedType(e) => {
                if let Some(v) = &e.initial_value {
                    let o = self
                        .enums
                        .ordinal(v)
                        .ok_or_else(|| nyi(&v.span(), "This enumerated value"))?;
                    bytes = int_bytes(o as i128, I32);
                }
            }
            InitialValueAssignmentKind::FunctionBlock(f) => {
                self.member_inits(ty, &f.init, &mut bytes, span)?;
            }
            InitialValueAssignmentKind::Structure(s) => {
                self.member_inits(ty, &s.elements_init, &mut bytes, span)?;
            }
            InitialValueAssignmentKind::Array(a) => {
                if let Ty::Array(at) = ty {
                    self.array_inits(at, &a.initial_values, &mut bytes, span)?;
                }
            }
            InitialValueAssignmentKind::Subrange(SpecificationKind::Named(tn)) => {
                let named = self.ty_of_name(tn)?;
                let image = self.default_image(&named)?;
                if image.len() == bytes.len() {
                    bytes = image;
                }
            }
            InitialValueAssignmentKind::Subrange(SpecificationKind::Inline(s)) => {
                if let (Some(sc), Ok(low)) = (ty.sc(), signed_ref(&s.subrange.start, span)) {
                    bytes = int_bytes(low as i128, sc);
                }
            }
            InitialValueAssignmentKind::FunctionBlockCall(f) if !f.params.is_empty() => {
                return Err(nyi(span, "A function block initializer with a call form"));
            }
            _ => {}
        }
        Ok(bytes)
    }

    fn member_inits(
        &mut self,
        ty: &Ty,
        inits: &[StructureElementInit],
        bytes: &mut [u8],
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        for e in inits {
            let (off, fty) = match ty {
                Ty::Struct(s) => match s.field(&e.name.to_string()) {
                    Some(f) => (f.offset, f.ty.clone()),
                    None => return Err(nyi(&e.name.span(), "This member")),
                },
                Ty::Fb(name) => {
                    let fb = self.fbs.get(name).cloned();
                    match fb.as_ref().and_then(|fb| fb.member(&e.name.to_string())) {
                        Some(m) => (m.offset, m.ty.clone()),
                        None => return Err(nyi(&e.name.span(), "This member")),
                    }
                }
                _ => return Err(nyi(span, "This initializer")),
            };
            let image = self.struct_member_image(&e.init, &fty, span)?;
            let off = off as usize;
            bytes[off..off + image.len()].copy_from_slice(&image);
        }
        Ok(())
    }

    fn struct_member_image(
        &mut self,
        init: &StructInitialValueAssignmentKind,
        ty: &Ty,
        span: &SourceSpan,
    ) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = self.default_image(ty)?;
        match init {
            StructInitialValueAssignmentKind::Constant(c) => {
                bytes = self.const_image(c, ty, span)?
            }
            StructInitialValueAssignmentKind::EnumeratedValue(v) => {
                let o = self
                    .enums
                    .ordinal(v)
                    .ok_or_else(|| nyi(&v.span(), "This enumerated value"))?;
                bytes = int_bytes(o as i128, I32);
            }
            StructInitialValueAssignmentKind::Array(elems) => {
                if let Ty::Array(at) = ty {
                    self.array_inits(at, elems, &mut bytes, span)?;
                }
            }
            StructInitialValueAssignmentKind::Structure(inits) => {
                self.member_inits(ty, inits, &mut bytes, span)?;
            }
            _ => return Err(nyi(span, "This member initializer")),
        }
        Ok(bytes)
    }

    fn array_inits(
        &mut self,
        at: &ArrayTy,
        elems: &[ArrayInitialElementKind],
        bytes: &mut [u8],
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let mut values: Vec<Vec<u8>> = vec![];
        for e in elems {
            match e {
                ArrayInitialElementKind::Constant(c) => {
                    values.push(self.const_image(c, &at.elem, span)?)
                }
                ArrayInitialElementKind::EnumValue(v) => {
                    let o = self
                        .enums
                        .ordinal(v)
                        .ok_or_else(|| nyi(&v.span(), "This enumerated value"))?;
                    values.push(int_bytes(o as i128, I32));
                }
                ArrayInitialElementKind::Repeated(r) => {
                    let one = match r.init.as_ref() {
                        Some(ArrayInitialElementKind::Constant(c)) => {
                            self.const_image(c, &at.elem, span)?
                        }
                        Some(ArrayInitialElementKind::EnumValue(v)) => {
                            let o = self.enums.ordinal(v).unwrap_or(0);
                            int_bytes(o as i128, I32)
                        }
                        Some(ArrayInitialElementKind::Repeated(_)) => {
                            return Err(nyi(span, "A nested repetition"))
                        }
                        None => self.default_image(&at.elem)?,
                    };
                    for _ in 0..r.size.value {
                        values.push(one.clone());
                    }
                }
            }
        }
        for (i, v) in values.iter().take(at.count() as usize).enumerate() {
            let off = i * at.stride as usize;
            bytes[off..off + v.len()].copy_from_slice(v);
        }
        Ok(())
    }

    /// The bytes of a constant stored in a value of type `ty`.
    pub fn const_image(
        &mut self,
        c: &ConstantKind,
        ty: &Ty,
        span: &SourceSpan,
    ) -> Result<Vec<u8>, Diagnostic> {
        match ty {
            Ty::Scalar { sc, .. } => {
                let e = crate::constant::constant(c, *sc, span)?;
                Ok(scalar_const_bytes(&e, *sc))
            }
            Ty::Str { cap, wide } => {
                let ConstantKind::CharacterString(s) = c else {
                    return Err(nyi(span, "This string initializer"));
                };
                let (size, _) = self.size_align(ty);
                let mut bytes = vec![0; size as usize];
                let units = crate::strings::encode(&s.value, *wide);
                let n = (*cap as usize).min(units.len() / if *wide { 2 } else { 1 });
                let unit = if *wide { 2 } else { 1 };
                bytes[..n * unit].copy_from_slice(&units[..n * unit]);
                Ok(bytes)
            }
            _ => Err(nyi(span, "This initial value")),
        }
    }
}

/// Bytes of a constant expression of a scalar type.
fn scalar_const_bytes(e: &ironplc_wasm_ir::Const, sc: Scalar) -> Vec<u8> {
    match e {
        ironplc_wasm_ir::Const::Bool(b) => vec![*b as u8],
        ironplc_wasm_ir::Const::Int(v) => int_bytes(*v, sc),
        ironplc_wasm_ir::Const::Real(v) => real_bytes(*v, sc),
        ironplc_wasm_ir::Const::Duration(v) => int_bytes(*v as i128, sc),
    }
}

/// Maximum number of characters of a string declaration.
pub(crate) fn string_cap(
    length: Option<&IntegerRef>,
    span: &SourceSpan,
) -> Result<u32, Diagnostic> {
    match length {
        None => Ok(DEFAULT_STRING_MAX_LENGTH as u32),
        Some(IntegerRef::Literal(i)) => Ok(i.value as u32),
        Some(IntegerRef::Constant(_)) => Err(nyi(span, "A string length given by a constant")),
    }
}

fn signed_ref(
    r: &ironplc_dsl::common::SignedIntegerRef,
    span: &SourceSpan,
) -> Result<i64, Diagnostic> {
    match r {
        ironplc_dsl::common::SignedIntegerRef::Literal(s) => {
            let v = s.value.value as i64;
            Ok(if s.is_neg { -v } else { v })
        }
        ironplc_dsl::common::SignedIntegerRef::Constant(_) => {
            Err(nyi(span, "An array bound given by a constant"))
        }
    }
}

fn kind_name(init: &InitialValueAssignmentKind) -> String {
    let d = format!("{init:?}");
    d.split(['(', ' ', '{'])
        .next()
        .unwrap_or_default()
        .to_string()
}
