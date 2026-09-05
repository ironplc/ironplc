//! Provides a derive macro that implements recursive visit and
//! fold operations onto structs and enumerations.
//!
//! This derive macro make several assumptions about the the
//! visit/fold structs and language elements:
//!
//! 1. for the visit struct, for each type, there exist a method
//!    with the prototype `visit_type_name`
//! 2. for the fold struct, for each type, there exist a method
//!    with the prototype `visit_type_name`
//! 3. fields in a struct use at most one container type (one
//!    Box, Option, Vec)
//! 4. variants in a struct are either unity or have a single
//!    item (no tuples)
//!
//! Satisfying the above, this macro generates appropriate
//! visit and fold functions to recursively walk the syntax tree
//! for each item within a struct and each variant in an enumeration.
//!
//! Any item that should be not walked must be marked with the
//! attribute:
//!
//! `
//! #[derive(ignore)]
//! `
//!
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

    let scope = match scope_attr(&ast.attrs) {
        Ok(scope) => scope,
        Err(err) => return err.to_compile_error().into(),
    };

    if let syn::Data::Struct(data_struct) = &ast.data {
        if let syn::Fields::Named(named_fields) = &data_struct.fields {
            if let Err(err) = check_scope_declared(name, named_fields, scope) {
                return err.to_compile_error().into();
            }
        }
    }

    let visit_res: Result<TokenStream> = match &ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(named_fields) => {
                expand_struct_recurse_visit(name, named_fields, scope)
            }
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
            syn::Fields::Named(named_fields) => {
                expand_struct_recurse_fold(name, named_fields, scope)
            }
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

/// Whether a declaration says it opens a lexical scope.
///
/// See `ironplc_dsl::scope` for what a scope is and why the traversal
/// rather than each pass is what opens one.
#[derive(Clone, Copy, PartialEq)]
enum ScopeAttr {
    /// `#[recurse(scope)]` -- the derived traversal brackets the
    /// recursion with `enter_scope`/`exit_scope`.
    Scope,
    /// `#[recurse(no_scope)]` -- it deliberately does not.
    NoScope,
    /// Neither, which is only allowed for a declaration that holds no
    /// variables of its own. See `check_scope_declared`.
    Absent,
}

/// Returns the scope attribute declared on a type, or `Absent`.
fn scope_attr(attrs: &[Attribute]) -> Result<ScopeAttr> {
    let mut scope = ScopeAttr::Absent;
    for attr in attrs {
        if attr.path().is_ident("recurse") {
            attr.parse_nested_meta(|meta| {
                // #[recurse(scope)]
                if meta.path.is_ident("scope") {
                    scope = ScopeAttr::Scope;
                    return Ok(());
                }
                // #[recurse(no_scope)]
                if meta.path.is_ident("no_scope") {
                    scope = ScopeAttr::NoScope;
                    return Ok(());
                }
                Err(meta.error("unrecognized value in recurse"))
            })?;
        }
    }
    Ok(scope)
}

/// Rejects a declaration that owns variables without saying whether it
/// scopes them.
///
/// A struct holding `variables: Vec<VarDecl>` is a program organization
/// unit or something shaped like one, so whether those variables are
/// visible outside its own body is a question that has to be answered
/// deliberately. Left unstated, it answers itself as "not a scope",
/// silently and in every pass at once -- which is how a `METHOD`'s
/// locals came to leak into its siblings
/// (https://github.com/ironplc/ironplc/issues/1439). Requiring the
/// attribute turns that omission into a build error at the declaration
/// itself.
///
/// The check is syntactic, so a declaration that holds variables under
/// another field name (`ResourceDeclaration::global_vars`,
/// `ConfigurationDeclaration::global_var`) is not reached by it.
fn check_scope_declared(name: &Ident, fields: &FieldsNamed, scope: ScopeAttr) -> Result<()> {
    if scope != ScopeAttr::Absent {
        return Ok(());
    }

    for field in &fields.named {
        let Some(ident) = field.ident.as_ref().filter(|id| *id == "variables") else {
            continue;
        };
        let (inner, container) = extract_type_ident_from_path(&field.ty);
        if container != DeclaredType::Vec || !type_is_named(inner, "VarDecl") {
            continue;
        }
        return Err(Error::new(
            ident.span(),
            format!(
                "`{name}` declares `variables: Vec<VarDecl>`, so it must say whether it opens a \
                 lexical scope: add `#[recurse(scope)]` to bracket its contents with \
                 `enter_scope`/`exit_scope`, or `#[recurse(no_scope)]` to state that its \
                 variables are visible to the enclosing scope"
            ),
        ));
    }

    Ok(())
}

/// Returns whether a type is the named path type, ignoring any qualifier.
fn type_is_named(ty: &syn::Type, name: &str) -> bool {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name),
        _ => false,
    }
}

/// Derives an implementation of the `Located` trait for a struct.
///
/// The generated `span` method returns a `SourceSpan` derived from one of the
/// struct's fields. Which field (and how) is selected as follows:
///
/// 1. A field marked `#[located(position)]` is treated as a `SourceSpan`
///    field, generating `self.<field>.clone()`.
/// 2. A field marked `#[located(delegate)]` is treated as a sub-node that
///    itself implements `Located`, generating `self.<field>.span()`.
/// 3. With no attribute, the struct must have a field named `position`, and
///    the body becomes `self.position.clone()`.
///
/// Only structs with named fields are supported. Types whose span requires
/// real logic (combining spans, matching over variants, etc.) must implement
/// `Located` by hand.
#[proc_macro_derive(Located, attributes(located))]
pub fn located_macro_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    expand_located(&ast).unwrap_or_else(|err| err.to_compile_error().into())
}

/// Selects how a field contributes to the derived `span` implementation.
enum LocatedKind {
    /// The field is a `SourceSpan`; clone it.
    Position,
    /// The field is a sub-node implementing `Located`; delegate to it.
    Delegate,
}

/// Returns a stream of tokens that implement `Located` for the given type.
fn expand_located(ast: &DeriveInput) -> Result<TokenStream> {
    let name = &ast.ident;

    let fields = match &ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(named_fields) => named_fields,
            _ => {
                return Err(Error::new(
                    ast.span(),
                    "#[derive(Located)] is only supported for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(Error::new(
                ast.span(),
                "#[derive(Located)] is only supported for structs with named fields",
            ))
        }
    };

    let body = located_body(fields)?;

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let gen = quote! {
        impl #impl_generics crate::core::Located for #name #ty_generics #where_clause {
            fn span(&self) -> crate::core::SourceSpan {
                #body
            }
        }
    };

    Ok(gen.into())
}

/// Returns the body of the derived `span` method for the given fields.
fn located_body(fields: &FieldsNamed) -> Result<proc_macro2::TokenStream> {
    // Collect every field that carries a `#[located(...)]` attribute.
    let mut annotated: Vec<(&Ident, LocatedKind)> = Vec::new();
    for field in &fields.named {
        if let Some(kind) = located_field_kind(field)? {
            let ident = field
                .ident
                .as_ref()
                .expect("named field always has an identifier");
            annotated.push((ident, kind));
        }
    }

    match annotated.as_slice() {
        // No attribute: fall back to a field named `position`.
        [] => {
            let has_position = fields
                .named
                .iter()
                .any(|f| f.ident.as_ref().is_some_and(|i| i == "position"));
            if has_position {
                Ok(quote! { self.position.clone() })
            } else {
                Err(Error::new(
                    fields.span(),
                    "#[derive(Located)] requires a field named `position` or a field annotated \
                     with #[located(position)] or #[located(delegate)]",
                ))
            }
        }
        [(ident, kind)] => match kind {
            LocatedKind::Position => Ok(quote! { self.#ident.clone() }),
            LocatedKind::Delegate => Ok(quote! { self.#ident.span() }),
        },
        _ => Err(Error::new(
            fields.span(),
            "#[derive(Located)] permits at most one #[located(...)] field attribute",
        )),
    }
}

/// Returns the `Located` kind requested by a field's `#[located(...)]`
/// attribute, or `None` when the field carries no such attribute.
fn located_field_kind(field: &Field) -> Result<Option<LocatedKind>> {
    let mut kind = None;
    for attr in &field.attrs {
        if attr.path().is_ident("located") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("position") {
                    kind = Some(LocatedKind::Position);
                    return Ok(());
                }
                if meta.path.is_ident("delegate") {
                    kind = Some(LocatedKind::Delegate);
                    return Ok(());
                }
                Err(meta.error("unrecognized value in located; expected `position` or `delegate`"))
            })?;
        }
    }
    Ok(kind)
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
                let has_fields = !v.fields.is_empty();
                if has_fields {
                    return Ok(quote! {
                        #name::#variant_name(..) => Ok(V::Value::default())
                    });
                } else {
                    return Ok(quote! {
                        #name::#variant_name => Ok(V::Value::default())
                    });
                }
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
fn expand_struct_recurse_visit(
    name: &Ident,
    fields: &FieldsNamed,
    scope: ScopeAttr,
) -> Result<TokenStream> {
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

    let body = quote! {
        #(#visit_methods;)*
        Ok(V::Value::default())
    };

    // Create the recurse implementation method for the type. A scope-bearing
    // declaration brackets the recursion with the visitor's scope hooks. The
    // body moves into a private method so that `exit_scope` runs even when it
    // returns early through `?`, which is what makes the pair impossible for a
    // visitor to leave unbalanced.
    let gen = if scope == ScopeAttr::Scope {
        quote! {
            impl #name {
                pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(
                    &self,
                    v: &mut V,
                ) -> Result<V::Value, E> {
                    v.enter_scope(ScopeBearing::as_scope_node(self))?;
                    let result = self.recurse_visit_inner(v);
                    v.exit_scope();
                    result
                }

                fn recurse_visit_inner<V: Visitor<E> + ?Sized, E>(
                    &self,
                    v: &mut V,
                ) -> Result<V::Value, E> {
                    #body
                }
            }
        }
    } else {
        quote! {
            impl #name {
                pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(
                    &self,
                    v: &mut V,
                ) -> Result<V::Value, E> {
                    #body
                }
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
                let has_fields = !v.fields.is_empty();
                if has_fields {
                    return Ok(quote! {
                        #name::#variant_name(inner) => Ok(#name::#variant_name(inner))
                    });
                } else {
                    return Ok(quote! {
                        #name::#variant_name => Ok(#name::#variant_name)
                    });
                }
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
fn expand_struct_recurse_fold(
    name: &Ident,
    fields: &FieldsNamed,
    scope: ScopeAttr,
) -> Result<TokenStream> {
    // Generate the dispatch methods for each of the items in the type.
    let fold_items = fields.named.iter().map(|f| {
        let name = &f.ident;

        if is_ignored(&f.attrs).expect("Attribute not permitted") {
            return quote! {
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

    let body = quote! {
        Ok(#name {
            #(#fold_items,)*
        })
    };

    // Create the recurse implementation method for the type. See
    // `expand_struct_recurse_visit` for why a scope-bearing declaration splits
    // the body into a private method. The scope node borrows `self` only for
    // the duration of the `enter_scope` call, which is why the by-value fold
    // signature still works.
    let gen = if scope == ScopeAttr::Scope {
        quote! {
            impl #name {
                pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<#name, E> {
                    f.enter_scope(ScopeBearing::as_scope_node(&self))?;
                    let result = self.recurse_fold_inner(f);
                    f.exit_scope();
                    result
                }

                fn recurse_fold_inner<F: Fold<E> + ?Sized, E>(
                    self,
                    f: &mut F,
                ) -> Result<#name, E> {
                    #body
                }
            }
        }
    } else {
        quote! {
            impl #name {
                pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<#name, E> {
                    #body
                }
            }
        }
    };

    Ok(gen.into())
}

/// Defines the types of containers objects.
/// The containing object determines how we recurse into each field.
#[derive(PartialEq)]
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
    format!("visit_{name}")
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
    format!("fold_{name}")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the scope guard over a struct definition, as
    /// `recurse_macro_derive` does.
    fn check(source: &str) -> Result<()> {
        let ast: DeriveInput = syn::parse_str(source).expect("test source must parse");
        let scope = scope_attr(&ast.attrs)?;
        match &ast.data {
            syn::Data::Struct(data_struct) => match &data_struct.fields {
                syn::Fields::Named(named_fields) => {
                    check_scope_declared(&ast.ident, named_fields, scope)
                }
                _ => panic!("test source must have named fields"),
            },
            _ => panic!("test source must be a struct"),
        }
    }

    #[test]
    fn check_scope_declared_when_holds_variables_and_no_attribute_then_error() {
        let result = check(
            "struct PouDeclaration {
                pub name: Id,
                pub variables: Vec<VarDecl>,
            }",
        );
        let message = result.expect_err("must reject").to_string();
        assert!(message.contains("PouDeclaration"), "{message}");
        assert!(message.contains("#[recurse(scope)]"), "{message}");
        assert!(message.contains("#[recurse(no_scope)]"), "{message}");
    }

    #[test]
    fn check_scope_declared_when_holds_variables_and_scope_then_ok() {
        assert!(check(
            "#[recurse(scope)]
            struct PouDeclaration {
                pub variables: Vec<VarDecl>,
            }",
        )
        .is_ok());
    }

    #[test]
    fn check_scope_declared_when_holds_variables_and_no_scope_then_ok() {
        assert!(check(
            "#[recurse(no_scope)]
            struct PouDeclaration {
                pub variables: Vec<VarDecl>,
            }",
        )
        .is_ok());
    }

    #[test]
    fn check_scope_declared_when_holds_no_variables_then_ok() {
        assert!(check(
            "struct NotAPou {
                pub name: Id,
                pub body: Vec<StmtKind>,
            }",
        )
        .is_ok());
    }

    /// The guard is deliberately syntactic: it asks about
    /// `variables: Vec<VarDecl>` and nothing else, so a declaration
    /// holding some other kind of variable is not swept in.
    #[test]
    fn check_scope_declared_when_variables_are_not_var_decls_then_ok() {
        assert!(check(
            "struct NotAPou {
                pub variables: Vec<TypeName>,
            }",
        )
        .is_ok());
    }

    /// A single `VarDecl` rather than a collection of them is a field of
    /// a declaration, not the declaration's variable block.
    #[test]
    fn check_scope_declared_when_variables_field_is_not_a_vec_then_ok() {
        assert!(check(
            "struct NotAPou {
                pub variables: VarDecl,
            }",
        )
        .is_ok());
    }

    #[test]
    fn scope_attr_when_unrecognized_value_then_error() {
        let ast: DeriveInput = syn::parse_str(
            "#[recurse(nonsense)]
            struct Thing {
                pub name: Id,
            }",
        )
        .expect("test source must parse");
        assert!(scope_attr(&ast.attrs).is_err());
    }

    #[test]
    fn scope_attr_when_no_attribute_then_absent() {
        let ast: DeriveInput =
            syn::parse_str("struct Thing { pub name: Id }").expect("test source must parse");
        assert!(scope_attr(&ast.attrs).expect("must parse") == ScopeAttr::Absent);
    }
}
