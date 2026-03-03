#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, ItemStruct, parse_macro_input};

/// In-memory cache of field definitions, keyed by struct name.
///
/// Both [`derive_fields`] and [`combine_fields`] run in the same proc-macro
/// dylib process during a single crate compilation.  `#[derive(Fields)]`
/// writes here; `#[combine_fields]` reads.
static FIELD_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Derive macro that caches a struct's field definitions for later merging
/// by [`combine_fields`].
///
/// The field definitions (including all attributes, doc comments, and
/// visibility) are stored in an in-memory cache shared with
/// `#[combine_fields]`.
///
/// # Panics
///
/// Panics if applied to an enum or union (only named structs are supported).
#[proc_macro_derive(Fields)]
pub fn derive_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident.to_string();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => panic!("Fields derive only supports structs with named fields"),
        },
        _ => panic!("Fields derive only supports structs"),
    };

    // Collect each field's tokens: attributes + vis + name + type
    let field_tokens: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let attrs = &f.attrs;
            let vis = &f.vis;
            let name = f.ident.as_ref().expect("named field must have ident");
            let ty = &f.ty;
            quote! {
                #(#attrs)*
                #vis #name: #ty,
            }
        })
        .collect();

    // Serialize as a parseable struct so combine_fields can read it back.
    let content = quote! { struct __Fields { #(#field_tokens)* } }.to_string();

    FIELD_CACHE
        .lock()
        .expect("FIELD_CACHE lock poisoned")
        .insert(struct_name, content);

    // No output tokens — all communication happens via the in-memory cache.
    TokenStream::new()
}

/// Attribute macro that merges fields from other `#[derive(Fields)]` structs
/// into the annotated struct.
///
/// # Example
///
/// ```ignore
/// #[combine_fields(CoreVocabulary, ApplicatorVocabulary)]
/// #[derive(Debug, Clone, Default)]
/// pub struct Schema {
///     // extra fields defined here
///     pub markdown_description: Option<String>,
/// }
/// ```
///
/// The macro reads cached field definitions (stored by `#[derive(Fields)]`)
/// and emits the target struct with all fields merged in.
///
/// # Panics
///
/// Panics if the attribute arguments are not a comma-separated list of
/// identifiers, or if the annotated item is not a struct with named fields.
#[proc_macro_attribute]
pub fn combine_fields(attr: TokenStream, item: TokenStream) -> TokenStream {
    let source_names = parse_macro_input!(
        attr with syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated
    );
    let input = parse_macro_input!(item as ItemStruct);

    let struct_attrs = &input.attrs;
    let struct_vis = &input.vis;
    let struct_name = &input.ident;
    let struct_generics = &input.generics;

    let cache = FIELD_CACHE.lock().expect("FIELD_CACHE lock poisoned");

    // Collect fields from each source struct's cached definition.
    let mut all_fields: Vec<TokenStream2> = Vec::new();

    for name in &source_names {
        let key = name.to_string();
        let content = cache.get(&key).unwrap_or_else(|| {
            panic!(
                "combine_fields: no cached fields for `{key}`.\n\
                 Make sure `{key}` derives `combine_structs::Fields` and its \
                 module is declared before the target struct."
            )
        });

        let parsed: ItemStruct = syn::parse_str(content).unwrap_or_else(|e| {
            panic!("combine_fields: failed to parse cached fields for `{key}`: {e}")
        });

        if let Fields::Named(named) = parsed.fields {
            for field in named.named {
                let attrs = &field.attrs;
                let vis = &field.vis;
                let ident = &field.ident;
                let ty = &field.ty;
                all_fields.push(quote! { #(#attrs)* #vis #ident: #ty, });
            }
        }
    }

    drop(cache);

    // Append the target struct's own fields.
    if let Fields::Named(ref named) = input.fields {
        for field in &named.named {
            let attrs = &field.attrs;
            let vis = &field.vis;
            let ident = &field.ident;
            let ty = &field.ty;
            all_fields.push(quote! { #(#attrs)* #vis #ident: #ty, });
        }
    }

    let expanded = quote! {
        #(#struct_attrs)*
        #struct_vis struct #struct_name #struct_generics {
            #(#all_fields)*
        }
    };

    expanded.into()
}
