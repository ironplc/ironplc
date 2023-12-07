//! Provides a derive macro that implements recursive visit and
//! fold operations onto structs and enumerations.

//! This derive macro make several assumptions about the the
//! visit/fold structs and language elements:

//! 1. for the visit struct, for each type, there exist a method
//!    with the prototype `visit_type_name`
//! 2. for the fold struct, for each type, there exist a method
//!    with the prototype `visit_type_name`
//! 3. fields in a struct use at most one container type (one
//!    Box, Option, Vec)
//! 4. variants in a struct are either unity or have a single
//!    item (no tuples)

//! Satisfying the above, this macro generates appropriate
//! visit and fold functions to recursively walk the syntax tree
//! for each item within a struct and each variant in an enumeration.

//! Any item that should be not walked must be marked with the
//! attribute:

//! `
//! #[derive(ignore)]
//! `

//! I am unaware of a way to enforce the assumptions other
//! than at build time.
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::quote;

use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::DataEnum;
use syn::DeriveInput;
use syn::Error;
use syn::Field;
use syn::Fields;
use syn::FieldsNamed;
use syn::Ident;
use syn::Result;

#[proc_macro_derive(Recurse, attributes(recurse))]
pub fn recurse_macro_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let visit_res: Result<TokenStream> = match &ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(named_fields) => expand_struct_recurse_visit(name, named_fields),
            _ => {
                unimplemented!("#[derive(Recurse)] is only supported for structs with named types")
            }
        },
        syn::Data::Enum(data_enum) => expand_enum_recurse_visit(name, data_enum),
        syn::Data::Union(_) => {
            unimplemented!("#[derive(Recurse)] is not supported for union types")
        }
    };
    let mut visit_res = visit_res.expect("Error generating visit implementation");

    let fold_res: Result<TokenStream> = match &ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(named_fields) => expand_struct_recurse_fold(name, named_fields),
            _ => {
                unimplemented!("#[derive(Recurse)] is only supported for structs with named types")
            }
        },
        syn::Data::Enum(data_enum) => expand_enum_recurse_fold(name, data_enum),
        syn::Data::Union(_) => {
            unimplemented!("#[derive(Recurse)] is not supported for union types")
        }
    };
    let fold_res = fold_res.expect("Error generating fold implementation");

    visit_res.extend(fold_res);
    visit_res
}

/// Returns a stream of tokens that implement recursive visit for an enumeration.
fn expand_enum_recurse_visit(name: &Ident, data_enum: &DataEnum) -> Result<TokenStream> {
    // Generate the matcher and dispatch for each variant
    let matchers: Result<Vec<proc_macro2::TokenStream>> = data_enum
        .variants
        .iter()
        .map(|v| {
            let variant_name = &v.ident;

            // An ignored variant does not recurse, but we need to include is so that all have a
            // defined match.
            if is_ignored(&v.attrs).unwrap() {
                return Ok(quote! {
                    #name::#variant_name => Ok(V::Value::default())
                });
            }

            let variant_contained_type = extract_type_ident_from_fields(&v.fields)?;

            let method_name = type_to_visitor_method_name(variant_contained_type.0);
            let method_name = syn::Ident::new(&method_name, name.span());

            match variant_contained_type.1 {
                // So far there are no enumerations with an Option value, thus not implemented
                DeclaredType::Option => unimplemented!(),
                DeclaredType::Vec => {
                    Ok(quote! {
                        #name::#variant_name(nodes) => {
                            match nodes.iter().map(|x| v.#method_name(x)).find(|r| r.is_err()) {
                                Some(err) => {
                                    // At least one of the items returned an error, so
                                    // return the first error.
                                    err
                                }
                                None => {
                                    // There were no errors, so return the default value
                                    Ok(V::Value::default())
                                }
                            }
                        }
                    })
                }
                DeclaredType::Simple => Ok(quote! {
                    #name::#variant_name(node) => v.#method_name(node)
                }),
                DeclaredType::Box => Ok(quote! {
                    #name::#variant_name(node) => v.#method_name(node.as_ref())
                }),
            }
        })
        .collect();
    let matchers = matchers?;

    // Create the recurse implementation
    let gen = quote! {
        impl #name {
            pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(
                &self,
                v: &mut V,
            ) -> Result<V::Value, E> {
                match self {
                    #(#matchers,)*
                }
            }
        }
    };

    Ok(gen.into())
}

/// Returns a stream of tokens that implement recursive visit for a struct.
fn expand_struct_recurse_visit(name: &Ident, fields: &FieldsNamed) -> Result<TokenStream> {
    // Filter out all fields that are marked as do not included
    let included_fields: Result<Vec<&Field>> = fields
        .named
        .iter()
        .filter_map(|f| match is_ignored(&f.attrs) {
            Ok(ignored) => {
                if ignored {
                    return None;
                }
                Some(Ok(f))
            }
            Err(err) => Some(Err(err)),
        })
        .collect();
    let included_fields = included_fields?;

    // Generate the dispatch methods for each of the items in the type.
    let visit_methods = included_fields.iter().map(|f| {
        let name = &f.ident;

        let ty = &f.ty;
        let ty_ident = extract_type_ident_from_path(ty);

        let method_name = type_to_visitor_method_name(ty_ident.0);
        let method_name = syn::Ident::new(&method_name, name.span());

        match ty_ident.1 {
            DeclaredType::Option => quote! {
                self.#name.as_ref().map_or_else(
                    || Ok(V::Value::default()),
                    |val| v.#method_name(val),
                )?
            },
            DeclaredType::Vec => {
                quote! {
                    match self.#name.iter().map(|x| v.#method_name(x)).find(|r| r.is_err()) {
                        Some(err) => {
                            // At least one of the items returned an error, so
                            // return the first error.
                            err
                        }
                        None => {
                            // There were no errors, so return the default value
                            Ok(V::Value::default())
                        }
                    }?
                }
            }
            DeclaredType::Box => quote! {
                v.#method_name(&self.#name.as_ref())?
            },
            DeclaredType::Simple => quote! {
                v.#method_name(&self.#name)?
            },
        }
    });

    // Create the recurse implementation method for the type.
    let gen = quote! {
        impl #name {
            pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(
                &self,
                v: &mut V,
            ) -> Result<V::Value, E> {
                #(#visit_methods;)*
                Ok(V::Value::default())
            }
        }
    };

    Ok(gen.into())
}

/// Returns a stream of tokens that implement recursive visit for an enumeration.
fn expand_enum_recurse_fold(name: &Ident, data_enum: &DataEnum) -> Result<TokenStream> {
    // Generate the matcher and dispatch for each variant
    let matchers: Result<Vec<proc_macro2::TokenStream>> = data_enum
        .variants
        .iter()
        .map(|v| {
            let variant_name = &v.ident;

            // An ignored variant does not recurse, but we need to include is so that all have a
            // defined match.
            if is_ignored(&v.attrs).unwrap() {
                return Ok(quote! {
                    #name::#variant_name => Ok(#name::#variant_name)
                });
            }

            let variant_contained_type = extract_type_ident_from_fields(&v.fields)?;

            let method_name = type_to_fold_method_name(variant_contained_type.0);
            let method_name = syn::Ident::new(&method_name, name.span());

            match variant_contained_type.1 {
                DeclaredType::Option => unimplemented!("fold enum with option"),
                DeclaredType::Vec => Ok(quote! {
                    #name::#variant_name(node) => {
                        let folds : Result<Vec<_>, E> = node.into_iter().map(|x| f.#method_name(x)).collect();
                        Ok(#name::#variant_name(folds?))
                    }
                }),
                DeclaredType::Simple => Ok(quote! {
                    #name::#variant_name(node) => { Ok(#name::#variant_name(f.#method_name(node)?)) }
                }),
                DeclaredType::Box => Ok(quote! {
                    #name::#variant_name(node) => { Ok(#name::#variant_name(Box::new(f.#method_name(*node)?))) }
                }),
            }
        })
        .collect();
    let matchers = matchers?;

    // Create the recurse implementation
    let gen = quote! {
        impl #name {
            pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<#name, E> {
                match self {
                    #(#matchers,)*
                }
            }
        }
    };

    Ok(gen.into())
}

/// Returns a stream of tokens that implement recursive fold for a struct.
fn expand_struct_recurse_fold(name: &Ident, fields: &FieldsNamed) -> Result<TokenStream> {
    // Generate the dispatch methods for each of the items in the type.
    let fold_items = fields.named.iter().map(|f| {
        let name = &f.ident;

        if is_ignored(&f.attrs).expect("Attribute not permitted") {
            return quote! {
                // TODO this probably doesn't handle errors
                #name: self.#name
            }
        }

        let ty = &f.ty;
        let ty_ident = extract_type_ident_from_path(ty);

        let method_name = type_to_fold_method_name(ty_ident.0);
        let method_name = syn::Ident::new(&method_name, name.span());

        match ty_ident.1 {
            DeclaredType::Option => quote! {
                #name: self.#name.map(|x| f.#method_name(x)).transpose()?
            },
            DeclaredType::Vec => {
                quote! {
                    #name: {
                        let folds : Result<Vec<_>, E> = self.#name.into_iter().map(|x| f.#method_name(x)).collect();
                        folds?
                    }
                }
            }
            DeclaredType::Box => quote! {
                #name: Box::new(f.#method_name(*self.#name)?)
            },
            DeclaredType::Simple => quote! {
                #name: f.#method_name(self.#name)?
            },
        }
    });

    // Create the recurse implementation method for the type.
    let gen = quote! {
        impl #name {
            pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<#name, E> {
                Ok(#name {
                    #(#fold_items,)*
                })
            }
        }
    };

    Ok(gen.into())
}

/// Defines the types of containers objects.
/// The containing object determines how we recurse into each field.
enum DeclaredType {
    Option,
    Vec,
    Box,
    // No container
    Simple,
}

/// Returns the name of the visitor method for the provided type.
///
/// For example, if the type name is ExampleType, then this returns
/// visit_example_type
fn type_to_visitor_method_name(ty: &syn::Type) -> String {
    let name = if let syn::Type::Path(ty) = ty {
        ty.path.segments.last()
    } else {
        panic!("Only works for structs");
    };

    let name = if let Some(n) = name {
        &n.ident
    } else {
        panic!("Only works for structs");
    };

    let name = name.to_string().to_case(Case::Snake);
    format!("visit_{}", name)
}

fn type_to_fold_method_name(ty: &syn::Type) -> String {
    let name = if let syn::Type::Path(ty) = ty {
        ty.path.segments.last()
    } else {
        panic!("Only works for structs");
    };

    let name = if let Some(n) = name {
        &n.ident
    } else {
        panic!("Only works for structs");
    };

    let name = name.to_string().to_case(Case::Snake);
    format!("fold_{}", name)
}

/// Returns the "interior" type from the given type. This works for well-defined
/// set of containers.
///
/// For `Option<T>`, returns `T`.
/// For `Vec<T>`, returns `T`.
/// For `T`, returns `T`.
///
/// If none of the above, then returns an error.
fn extract_type_ident_from_path(ty: &syn::Type) -> (&syn::Type, DeclaredType) {
    let option_nested = extract_type_from_container(
        ty,
        vec!["Option|", "std|option|Option|", "core|option|Option|"],
    );
    if let Some(ident) = option_nested {
        return (ident, DeclaredType::Option);
    }

    let vec_nested = extract_type_from_container(ty, vec!["Vec|", "std|vec|Vec|"]);
    if let Some(ident) = vec_nested {
        return (ident, DeclaredType::Vec);
    }

    let vec_nested = extract_type_from_container(ty, vec!["Box|", "std|alloc|Box|"]);
    if let Some(ident) = vec_nested {
        return (ident, DeclaredType::Box);
    }

    (ty, DeclaredType::Simple)
}

/// Returns the type within a container or returns `None` when the type hierarchy
/// does not match the container type of interest.
/// Adapted from https://stackoverflow.com/questions/55271857/how-can-i-get-the-t-from-an-optiont-when-using-syn
fn extract_type_from_container<'a>(
    ty: &'a syn::Type,
    container_ty: Vec<&str>,
) -> Option<&'a syn::Type> {
    use syn::{GenericArgument, Path, PathArguments, PathSegment};

    fn extract_type_path(ty: &syn::Type) -> Option<&Path> {
        match *ty {
            syn::Type::Path(ref typepath) if typepath.qself.is_none() => Some(&typepath.path),
            _ => None,
        }
    }

    // TODO maybe optimization, reverse the order of segments
    fn extract_container_segment<'a>(path: &'a Path, tys: Vec<&str>) -> Option<&'a PathSegment> {
        let idents_of_path = path.segments.iter().fold(String::new(), |mut acc, v| {
            acc.push_str(&v.ident.to_string());
            acc.push('|');
            acc
        });
        tys.into_iter()
            .find(|s| idents_of_path == **s)
            .and_then(|_| path.segments.last())
    }

    extract_type_path(ty)
        .and_then(|path| extract_container_segment(path, container_ty))
        .and_then(|path_seg| {
            let type_params = &path_seg.arguments;
            // It should have only on angle-bracketed param ("<String>"):
            match *type_params {
                PathArguments::AngleBracketed(ref params) => params.args.first(),
                _ => None,
            }
        })
        .and_then(|generic_arg| match *generic_arg {
            GenericArgument::Type(ref ty) => Some(ty),
            _ => None,
        })
}

/// Returns the type identifier from the fields (from an enumeration).
fn extract_type_ident_from_fields(fields: &Fields) -> Result<(&syn::Type, DeclaredType)> {
    match fields {
        Fields::Unnamed(unnamed_fields) => {
            if unnamed_fields.unnamed.len() != 1 {
                todo!()
            }

            Ok(extract_type_ident_from_path(
                &unnamed_fields.unnamed.first().unwrap().ty,
            ))
        }
        Fields::Named(named) => Err(Error::new(
            named.span(),
            "Enum field must be unnamed and have a single item",
        )),
        Fields::Unit => Err(Error::new(
            fields.span(),
            "Enum field must be unnamed and have a single item",
        )),
    }
}

/// Returns if the field attributes indicate that the field is ignored
/// (that is, do not recurse into the field).
fn is_ignored(attrs: &Vec<Attribute>) -> Result<bool> {
    let mut ignored = false;
    for attr in attrs {
        if attr.path().is_ident("recurse") {
            attr.parse_nested_meta(|meta| {
                // #[recurse(ignore)]
                if meta.path.is_ident("ignore") {
                    ignored = true;
                    return Ok(());
                }
                Err(meta.error("unrecognized value in recurse"))
            })?;
        }
    }
    Ok(ignored)
}
