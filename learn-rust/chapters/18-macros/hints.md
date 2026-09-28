# Chapter 18 hints

## c18_e01_macro_rules_basics
Start with the first error. Unlike functions, `macro_rules!` macros are
*textually scoped*: a macro only exists after the line where it is defined.
Where are the macros defined relative to the code that calls them?

Next, `newtype!` uses its first argument as the name in `struct $name(...)`.
A struct name is an identifier, not an expression. Which fragment specifier
matches an identifier? (`expr`, `ident`, `ty`, `tt`, `literal`, `path`, …)

Last, the tests show that `square!(next())` runs `next()` twice. The template
`$x * $x` pastes the *expression* twice, so it is evaluated twice. Evaluate
it once into a local variable inside a block: `{{ let value = $x; value * value }}`.
The doubled braces make the expansion a single block expression. Do the same
in `max_of!` (bind `$a` and `$b` to locals first).

## c18_e02_repetition
The error "no rules expected `,`" means that no rule's pattern accepts a
comma at that position: each rule only matches one item.

Wrap the repeated part in `$( ... ),*` in the pattern, and repeat the
template part with `$( ... )*`. For example, `$($key:expr => $value:expr),*`
matches `a => 1, b => 2`. Add `$(,)?` after it to allow a trailing comma.

`count!`: every token becomes `+ 1`. A metavariable must appear inside a
repetition for the compiler to know how many times to repeat it, so write
`0usize $(+ { stringify!($x); 1usize })*`.

`signal_paths!`: nest the repetitions, `$($branch:ident => [$($leaf:ident),*]),*`.
In the template, the inner `$( ... )*` can use `$branch` from the outer level.
Emit `concat!(stringify!($branch), ".", stringify!($leaf)),` with the comma
*inside* the inner repetition, so that empty branches don't produce `,,`.

## c18_e03_recursive_and_tt_munching
`max_of!`: add a second rule, `($x:expr, $($rest:expr),+ $(,)?)`, that
evaluates `$x` once, calls `max_of!($($rest),+)` for the rest, and returns
the larger value.

`glob!` hits the recursion limit because an `@acc` call that matches no
internal rule falls through to the public catch-all `($($tokens:tt)+)`,
which wraps it in *another* `@acc`, forever. You need `@acc` rules for `*`
and for `**`. `**` is lexed as two `*` tokens, and rules are tried top to
bottom, so the `* *` rule must come *before* the single `*` rule.

`signals!`: copy the `actuator` rule for `sensor` and `attribute`. Then write
the `@decl` rule. Its pattern after the accumulator and `$kind:expr,` is
`$($segment:ident).+ : $ty:ty = $description:literal; $($rest:tt)*`. It
recurses into `@acc` with one more `SignalDecl { path: stringify!($($segment).+), ... },`
appended to the accumulator.

## c18_e04_macros_that_generate_items
Both macros must expand to *items* (impls, an enum), so the templates
contain `impl ... { ... }` blocks directly, repeated with `$( ... )*`.

`impl_signal_value!`: per pair, write `impl SignalValue for $t` (with
`TYPE_NAME = stringify!($t)`, `into_data_value` → `DataValue::$variant(self)`,
`from_data_value` matching `DataValue::$variant(v)`), plus
`impl From<$t> for DataValue`. Use `Clone::clone(v)` rather than `v.clone()`
or `*v`: it works for `String` and for `Copy` types without clippy warnings.

`define_data_types!`: the same list becomes the enum body
(`$($variant),+`), the `ALL` slice, the `name()` match
(`$(DataType::$variant => $name),+`), and the `FromStr` match
(`$($name => Ok(DataType::$variant),)+ other => Err(...)`). String literals
work as match patterns.

## c18_e05_reading_derive_output
This is what the derives would write for you:

- `Debug`: `f.debug_struct("Datapoint").field("path", &self.path)...finish()`.
  It handles `{:#?}` automatically.
- `Clone`: clone each field (`self.value` is `Copy`, so a plain copy works).
- `PartialEq`: compare each field with `&&`.
- `Default`: `String::default()`, `i64::default()`, `None`.

`Shared<T>`: write `impl<T> Clone for Shared<T>` with *no* `T: Clone` bound,
and clone the `Arc` with `Arc::clone(&self.inner)`. `#[derive(Clone)]` would
have added `T: Clone`, and `NotClone` in the test would then fail.

`SignalId`: deserialize an `i32` first with `i32::deserialize(deserializer)?`,
then validate it. Build the error with `D::Error::custom(format!(...))`.
`custom` comes from the `serde::de::Error` trait, which is already imported
as `_`.

## c18_e06_write_a_derive_macro
Edit `learn-rust/chapters/18-macros/exercise-macros/src/lib.rs`, not the test file. While you
work, remember that every exercise depends on that crate, so a compile error
there breaks them all.

Structure: write `fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream>`
and call it from the entry point with
`expand(&input).unwrap_or_else(syn::Error::into_compile_error).into()`.

Attributes: loop over `input.attrs` (and `field.attrs`), keep those where
`attr.path().is_ident("signal")`, and call `attr.parse_nested_meta(|meta| { ... })`.
Inside, `meta.path.is_ident("prefix")` and
`let v: syn::LitStr = meta.value()?.parse()?;` read `prefix = "..."`. A bare
`skip` is just `meta.path.is_ident("skip")`.

Fields: `match &input.data { Data::Struct(d) => match &d.fields { Fields::Named(n) => &n.named, ... } }`.
Collect the paths as `Vec<String>` and the field identifiers as
`Vec<&Ident>`. Convert `dome_light` to `DomeLight` by splitting on `_` and
uppercasing the first char of each part.

Output: `let (impl_g, ty_g, where_c) = input.generics.split_for_impl();`, then
`quote! { impl #impl_g #name #ty_g #where_c { pub const SIGNAL_PATHS: &'static [&'static str] = &[#(#paths),*]; ... } }`.
A `Vec<String>` interpolates as string literals. In `signal_values`, repeat
`(#paths, ::std::string::ToString::to_string(&self.#idents))`. Both
vectors are iterated in lockstep. The full reference is in
`learn-rust/chapters/18-macros/solution-macros/src/lib.rs`.

## c18_e07_review_macro_hygiene
Bug 1: read `count_tt!()`'s first arm on its own — an empty invocation
matches it directly, no recursion involved. If it says `1usize`, then
"zero items" is already reporting one before any `+ 1usize` from a
recursive call ever runs. It should be `0usize`.

Bug 2: `smaller_of!($a, $b)` expands `$a` and `$b` as raw expressions
wherever they appear in the template — once in `$a < $b`, and *again* in
whichever branch runs. If `$a` is `read_reading(&reads, 3)`, that call
appears twice in the expanded code and runs twice. Bind each argument to a
`let` once, at the top of the expansion, and compare/return the bindings
instead of the raw `$a`/`$b`.

Bug 3: inside `$( ... )*`, every metavariable that's actually part of the
repetition (here, `$rest`) advances one element per iteration; anything
else you write is a plain, unchanged value. `path.push_str($first)` inside
the loop pushes the *same* first segment every time instead of the
repetition's current element — it should push `$rest`.
