//! Derive macros for the IronPLC bytecode container newtype identifiers.
//!
//! The container format uses many single-field `u16` tuple-struct newtypes
//! (`FunctionId`, `TaskId`, `VarIndex`, ...) that all need the same small
//! inherent API and `Display` impl. `#[derive(U16Id)]` generates that shared
//! boilerplate while leaving each struct definition — its derives, its doc
//! comment, and its `u16` field — fully hand-written and visible.

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Error, Fields, Type};

/// Derives the shared newtype API for a single-field `u16` tuple struct:
///
/// - `pub const fn new(raw: u16) -> Self`
/// - `pub const fn raw(self) -> u16`
/// - `pub const fn to_le_bytes(self) -> [u8; 2]`
/// - `impl core::fmt::Display` writing the raw value
///
/// Applying it to anything other than a tuple struct with exactly one `u16`
/// field is a compile error.
#[proc_macro_derive(U16Id)]
pub fn u16_id_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    match expand(&ast) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(ast: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &ast.ident;

    let data_struct = match &ast.data {
        Data::Struct(data_struct) => data_struct,
        _ => {
            return Err(Error::new(
                ast.span(),
                "#[derive(U16Id)] is only supported for a tuple struct with one `u16` field",
            ))
        }
    };

    let field = match &data_struct.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0],
        _ => {
            return Err(Error::new(
                data_struct.fields.span(),
                "#[derive(U16Id)] requires a tuple struct with exactly one field",
            ))
        }
    };

    if !is_u16(&field.ty) {
        return Err(Error::new(
            field.ty.span(),
            "#[derive(U16Id)] requires the single field to be `u16`",
        ));
    }

    Ok(quote! {
        impl #name {
            /// Creates a new identifier from a raw `u16`.
            pub const fn new(raw: u16) -> Self {
                Self(raw)
            }
            /// Returns the raw `u16` value.
            pub const fn raw(self) -> u16 {
                self.0
            }
            /// Returns the little-endian byte representation.
            pub const fn to_le_bytes(self) -> [u8; 2] {
                self.0.to_le_bytes()
            }
        }

        impl core::fmt::Display for #name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    })
}

/// Returns `true` if the type is the path `u16`.
fn is_u16(ty: &Type) -> bool {
    matches!(ty, Type::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("u16"))
}
