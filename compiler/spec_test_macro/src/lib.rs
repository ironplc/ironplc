use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

/// Marks a function as a spec conformance test for a given requirement.
///
/// Adds `#[test]` and injects a compile-time reference to the generated
/// constant in `crate::spec_requirements`. If the requirement does not
/// exist in the spec markdown, compilation fails.
///
/// # Usage
///
/// ```ignore
/// #[spec_test(REQ_CF_001)]
/// fn container_spec_req_cf_001_header_size() {
///     assert_eq!(std::mem::size_of::<FileHeader>(), 256);
/// }
/// ```
#[proc_macro_attribute]
pub fn spec_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let req_id = parse_macro_input!(attr as Ident);
    let mut func = parse_macro_input!(item as ItemFn);

    // Inject a reference to the generated constant at the top of the body.
    // If the constant does not exist, the test fails to compile.
    let original_block = &func.block;
    func.block = syn::parse_quote!({
        let _ = crate::spec_requirements::#req_id;
        let __inner = || #original_block;
        __inner()
    });

    // Add #[test] if not already present.
    let has_test = func.attrs.iter().any(|a| a.path().is_ident("test"));
    if !has_test {
        func.attrs.insert(0, syn::parse_quote!(#[test]));
    }

    quote!(#func).into()
}
