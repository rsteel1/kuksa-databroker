//! Reference solution for chapter 18's procedural macro (c18_e06).
//!
//! A proc-macro crate is compiled for the *host* and loaded into the
//! compiler. Its functions receive the tokens of the item they're applied to
//! and return new tokens, which are compiled as if you had typed them.
//!
//! `#[derive(SignalNames)]` turns
//!
//! ```ignore
//! #[derive(SignalNames)]
//! #[signal(prefix = "Vehicle.Cabin")]
//! struct Cabin {
//!     temperature: f32,
//!     #[signal(rename = "IsDomeOn")]
//!     dome_light: bool,
//!     #[signal(skip)]
//!     cache: u8,
//! }
//! ```
//!
//! into
//!
//! ```ignore
//! impl Cabin {
//!     pub const SIGNAL_PATHS: &'static [&'static str] =
//!         &["Vehicle.Cabin.Temperature", "Vehicle.Cabin.IsDomeOn"];
//!
//!     pub fn signal_values(&self) -> Vec<(&'static str, String)> {
//!         vec![
//!             ("Vehicle.Cabin.Temperature", self.temperature.to_string()),
//!             ("Vehicle.Cabin.IsDomeOn", self.dome_light.to_string()),
//!         ]
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

#[proc_macro_derive(SignalNames, attributes(signal))]
pub fn derive_signal_names(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // WHY: turn a `syn::Error` into a `compile_error!` that points at the
    // offending tokens, instead of panicking (a panic in a proc macro
    // becomes an ugly "proc macro panicked" error without a useful span).
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let prefix = struct_prefix(input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "SignalNames can only be derived for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "SignalNames can only be derived for structs with named fields",
            ))
        }
    };

    let mut paths = Vec::new();
    let mut idents = Vec::new();
    for field in fields {
        let ident = field
            .ident
            .as_ref()
            .expect("named fields always have an identifier");
        let options = field_options(field)?;
        if options.skip {
            continue;
        }
        let leaf = options
            .rename
            .unwrap_or_else(|| to_upper_camel(&ident.to_string()));
        let path = match &prefix {
            Some(prefix) => format!("{prefix}.{leaf}"),
            None => leaf,
        };
        paths.push(path);
        idents.push(ident);
    }

    let name = &input.ident;
    // WHY: `split_for_impl` copies the struct's generics (and where-clause)
    // onto the impl, so `struct Wrapper<T: Display>` works too.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// VSS paths of this struct's signals, in field order.
            pub const SIGNAL_PATHS: &'static [&'static str] = &[#(#paths),*];

            /// Each signal's path with its current value rendered via `Display`.
            pub fn signal_values(&self) -> ::std::vec::Vec<(&'static str, ::std::string::String)> {
                ::std::vec![#((#paths, ::std::string::ToString::to_string(&self.#idents))),*]
            }
        }
    })
}

/// Reads `#[signal(prefix = "...")]` on the struct itself.
fn struct_prefix(input: &DeriveInput) -> syn::Result<Option<String>> {
    let mut prefix = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("signal") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let value: LitStr = meta.value()?.parse()?;
                prefix = Some(value.value());
                Ok(())
            } else {
                Err(meta.error("unknown struct attribute; expected `prefix = \"...\"`"))
            }
        })?;
    }
    Ok(prefix)
}

#[derive(Default)]
struct FieldOptions {
    rename: Option<String>,
    skip: bool,
}

/// Reads `#[signal(rename = "...")]` and `#[signal(skip)]` on a field.
fn field_options(field: &syn::Field) -> syn::Result<FieldOptions> {
    let mut options = FieldOptions::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("signal") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value: LitStr = meta.value()?.parse()?;
                options.rename = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("skip") {
                options.skip = true;
                Ok(())
            } else {
                Err(meta.error("unknown field attribute; expected `rename = \"...\"` or `skip`"))
            }
        })?;
    }
    Ok(options)
}

/// `dome_light` → `DomeLight` (VSS leaf names are UpperCamelCase).
fn to_upper_camel(snake: &str) -> String {
    snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}
