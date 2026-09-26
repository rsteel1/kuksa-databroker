//! Chapter 18's procedural macro — YOU edit this file in exercise c18_e06.
//!
//! A proc-macro crate is compiled for the *host* and loaded into the
//! compiler. Its functions receive the tokens of the item they're applied to
//! and return new tokens, which are compiled as if you had typed them.
//!
//! Goal: `#[derive(SignalNames)]` turns
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
//!
//! IMPORTANT: every exercise depends on this crate, so keep it compiling.
//! If you break it mid-edit, *all* exercises stop compiling until it's fixed.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

// `attributes(signal)` registers `#[signal(...)]` as a *helper attribute*,
// so the compiler accepts it on the struct and its fields.
#[proc_macro_derive(SignalNames, attributes(signal))]
pub fn derive_signal_names(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // TODO(c18_e06): replace the placeholder below with a real expansion.
    //   1. Read `#[signal(prefix = "...")]` from `input.attrs`
    //      (`attr.path().is_ident("signal")`, then `attr.parse_nested_meta`).
    //   2. Match `input.data` to get the named fields (`Data::Struct`,
    //      `Fields::Named`); for anything else return a `syn::Error` built with
    //      `syn::Error::new_spanned(&input.ident, "...")`.
    //   3. For each field: honour `#[signal(skip)]` and `#[signal(rename = "..")]`,
    //      otherwise turn `dome_light` into `DomeLight`; join with the prefix.
    //   4. Build the impl with `quote!`, repeating with `#(#paths),*`, and use
    //      `input.generics.split_for_impl()` so generic structs work.
    //   5. Turn errors into tokens with `syn::Error::into_compile_error`.
    let name = &input.ident;
    let message = format!(
        "c18_e06: #[derive(SignalNames)] on `{name}` is not implemented yet — \
         edit learn-rust/chapters/18-macros/exercise-macros/src/lib.rs"
    );
    quote! {
        compile_error!(#message);
    }
    .into()
}
