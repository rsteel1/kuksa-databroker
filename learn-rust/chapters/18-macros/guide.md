# 18 — Macros

> **Builds on:** chapters 04 (patterns), 08 (crates, build.rs), 09 (traits and derives).

**Why this matters.** Open almost any Rust file and you'll find macros:
`println!`, `vec!`, `#[derive(Debug)]`, `#[tokio::main]`,
`#[async_trait]`, `serde`'s `#[derive(Deserialize)]`, `tonic::include_proto!`.
In the databroker, the whole gRPC API (`databroker-proto`) comes into
existence through a macro. Every async trait method on `SignalProvider` is
rewritten by one. Until you can picture what a macro expands to, big parts of
any codebase are magic. This chapter removes the magic. You'll write
declarative macros (`macro_rules!`), read what derives generate, and build a
real derive macro with `syn` and `quote`.

---

## What a macro is (and isn't)

A function runs when your program runs. A **macro runs inside the compiler**:
it takes *tokens* (pieces of source code) and produces *more source code*,
which is then compiled as if you had typed it yourself.

```
your source ──► tokenizer ──► macro expansion ──► parsing, type checking ──► machine code
                                  ▲
                    macros run here, on tokens,
                    before types exist
```

Consequences worth memorising:

| Because macros run on tokens… | …this follows |
|---|---|
| they run before type checking | a macro can't ask "what type is this?"; it only sees the text |
| they produce code | anything a macro generates could have been written by hand |
| they're expanded at every call site | a big macro called 100 times means 100 copies to compile |
| input need not be valid Rust | macros can accept small languages (DSLs): `json!{...}`, `select!{...}`, `sql!(...)` |

Rust has two kinds:

1. **Declarative macros** (`macro_rules!`): pattern → template rules. They
   are defined in the same crate and are easy to write. Examples: `vec!`,
   `println!`, `lazy_static!`, `json!`, `tokio::select!`.
2. **Procedural macros**: ordinary Rust functions compiled into a separate
   crate, which the compiler loads and calls. They come in three kinds:
   - derive (`#[derive(Deserialize)]`) *adds* items next to a struct or enum;
   - attribute (`#[tokio::main]`, `#[async_trait]`) *replaces* the item it's on;
   - function-like (`sqlx::query!(...)`) is called like `name!(...)`.

The rule of thumb from the API guidelines: **reach for a function or a
generic first**. Use a macro only when you need something functions can't
do: variable numbers of arguments, generating items, a custom syntax, or
compile-time string handling.

## Declarative macros: macro_rules!

```rust
macro_rules! max_of {
    //  pattern (matcher)        template (transcriber)
    ($a:expr, $b:expr) => {{
        let a = $a;
        let b = $b;
        if a > b { a } else { b }
    }};
}

let biggest = max_of!(3, 7);   // expands to the block above, with 3 and 7 pasted in
```

The compiler tries each rule (`pattern => template;`) **top to bottom** and
uses the first whose pattern matches the tokens of the call. `$a:expr`
captures one *fragment* of a given kind:

| Specifier | Matches | Example |
|---|---|---|
| `expr` | an expression | `1 + 2`, `f(x)`, `vec![1]` |
| `ident` | an identifier | `SignalId`, `speed` |
| `ty` | a type | `u32`, `Vec<String>`, `&'static str` |
| `path` | a path | `std::fmt::Display` |
| `literal` | a literal | `"text"`, `42`, `true` |
| `pat` | a pattern | `Some(x)`, `1..=5` |
| `block` | a `{ ... }` block | |
| `stmt` / `item` | a statement / an item (fn, struct, impl …) | |
| `meta` | the inside of an attribute | `derive(Debug)` |
| `lifetime` | a lifetime | `'a` |
| `vis` | a visibility, possibly empty | `pub`, `pub(crate)` |
| `tt` | a single *token tree*: one token, or a whole `(...)`/`[...]`/`{...}` group | anything |

Four rules every macro writer learns by being bitten:

1. **Textual order.** A `macro_rules!` macro exists only *below* its definition
   in the file (and in child modules declared after it). Functions can be
   anywhere; macros can't. To use one across modules, put `#[macro_use]` on the
   module, or mark it `#[macro_export]`, which puts it at the crate root.
2. **Fragments keep their kind.** Once captured as `expr`, a fragment can only
   be used where an expression fits, so `struct $name` needs `$name:ident`.
   Captured expressions also keep their grouping: `square!(1 + 2)` is
   `(1 + 2) * (1 + 2)`. (C preprocessor macros famously get this wrong.)
3. **Arguments are pasted, not evaluated.** If the template mentions `$x`
   twice, the expression runs twice. Bind it once:
   `{{ let value = $x; value * value }}`. The doubled braces make the whole
   expansion one block *expression*.
4. **Hygiene.** A local variable created *inside* a macro can't clash with a
   variable of the same name at the call site: the compiler tags each
   identifier with where it came from. (Items such as functions and structs
   are *not* hygienic. They are visible to the caller, which is what lets
   `newtype!` define a struct you can use.)

## Repetition

`$( ... ) sep rep` repeats a part of the pattern:

- `rep` is `*` (zero or more), `+` (one or more) or `?` (zero or one);
- `sep` is an optional separator token, usually `,` or `;`.

```rust
macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = ::std::collections::HashMap::new();
        $( map.insert($key, $value); )*
        map
    }};
}
```

`$(,)?` accepts an optional trailing comma, which is what people expect from
Rust syntax. In the template, `$( ... )*` repeats once per match. Each
metavariable used inside it must come from a repetition at the same depth,
which is how the compiler knows how many times to repeat. Nested
repetitions (`$( $branch => [ $( $leaf ),* ] ),*`) can use outer-level
variables inside the inner level.

Two built-ins make compile-time strings possible:

- `stringify!(tokens)` turns tokens into a `&'static str` *without* evaluating
  them: `stringify!(Vehicle.Speed)` is `"Vehicle.Speed"`.
- `concat!(...)` glues literals into one `&'static str` at compile time.

The databroker uses `concat!` to build its `--version` text
(`databroker/src/main.rs:245`) and `option_env!` to read build-time
environment variables set by `vergen` in `build.rs` (`main.rs:233`).

Watch out for lints inside generated code. If `hashmap!{}` is called with zero
pairs, `let mut map` becomes an `unused_mut` warning *in the caller's crate*.
Put targeted `#[allow(...)]`s on generated statements, as std does.

## Recursion and tt munching

A macro may call itself. That gives you loops at compile time:

```rust
macro_rules! max_of {
    ($x:expr $(,)?) => { $x };
    ($x:expr, $($rest:expr),+ $(,)?) => {{
        let first = $x;
        let rest = max_of!($($rest),+);   // recurse on the tail
        if first > rest { first } else { rest }
    }};
}
```

**TT munching** is the general form. The macro matches the first token tree
or two, handles them, and recurses with `$($rest:tt)*`, usually carrying an
**accumulator** of output built so far. By convention, internal rules start
with a marker like `@acc` so they can't be confused with the public syntax:

```rust
macro_rules! glob {
    (@acc [$($out:expr),*]) => { concat!($($out),*) };                                // done
    (@acc [$($out:expr),*] * * $($rest:tt)*) => { glob!(@acc [$($out,)* "**"] $($rest)*) };
    (@acc [$($out:expr),*] *   $($rest:tt)*) => { glob!(@acc [$($out,)* "*"]  $($rest)*) };
    (@acc [$($out:expr),*] .   $($rest:tt)*) => { glob!(@acc [$($out,)* "."]  $($rest)*) };
    (@acc [$($out:expr),*] $s:ident $($rest:tt)*) => { glob!(@acc [$($out,)* stringify!($s)] $($rest)*) };
    ($($tokens:tt)+) => { glob!(@acc [] $($tokens)+) };                               // entry
}
const P: &str = glob!(Vehicle.*.IsOpen);   // "Vehicle.*.IsOpen", at compile time
```

Things to notice:

- **Rule order is part of the logic.** `**` is lexed as two `*` tokens, so
  the `* *` rule must come before the `*` rule.
- **A catch-all public rule can loop forever.** If an `@acc` call matches no
  internal rule, it falls into `($($tokens:tt)+)`, which wraps it again, and
  you get "recursion limit reached while expanding". Put internal rules first
  and make them exhaustive.
- **`compile_error!("...")`** in a rule turns bad input into a readable
  error. Its span points at the macro definition, not the caller, which is one
  of the limits of `macro_rules!` that proc macros fix.
- Every recursive step costs compile time and counts towards the recursion
  limit (128 by default; `#![recursion_limit = "256"]` raises it).

`serde_json::json!` is a large tt-muncher (see "In the wild").

## Generating items

At module level a macro can expand to **items**: structs, enums, `impl`
blocks, functions, even `#[test]` functions. This is the main use of macros
in real code: removing copy-paste across many types.

```rust
macro_rules! impl_signal_value {
    ($($t:ty => $variant:ident),* $(,)?) => {
        $(
            impl SignalValue for $t {
                fn into_data_value(self) -> DataValue { DataValue::$variant(self) }
                // ...
            }
        )*
    };
}
impl_signal_value! { bool => Bool, i32 => Int32, f64 => Double, String => String }
```

One table can generate the enum, its `Display`, its `FromStr` and an `ALL`
list, so adding a variant is a one-line change that can't drift out of sync.
The databroker doesn't do this. `databroker/src/types.rs` spells out the 24
`DataType` variants in the enum (line 17) and again in `Display` (line 44).
`databroker-proto/src/lib.rs` has a third hand-written list in
`impl FromStr for DataType` (line 49), and `DataValue::greater_than`
(`types.rs:137`) is hundreds of lines of near-identical match arms. That's a
reasonable trade-off: hand-written code is easier to read, grep and debug,
and it's what most contributors expect. But it's exactly the situation where
a macro would help, and a fair discussion to have in a PR.

A test-generation trick used all over the ecosystem:

```rust
macro_rules! roundtrip_tests {
    ($($name:ident: $value:expr => $ty:ty),*) => {
        $( #[test] fn $name() { /* ... uses $value and $ty ... */ } )*
    };
}
```

Each case becomes its own named test, so a failure reports
`roundtrip_i64 ... FAILED` rather than "case 3 of 7".

## What derive expands to

`#[derive(Trait)]` is a procedural macro that reads your struct and *adds*
an `impl Trait for YourStruct`. For

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Datapoint { pub path: String, pub value: i64, pub source_ts: Option<u64> }
```

the compiler generates (simplified from the real std expansion):

```rust
impl ::core::fmt::Debug for Datapoint {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("Datapoint")
            .field("path", &self.path)
            .field("value", &self.value)
            .field("source_ts", &self.source_ts)
            .finish()
    }
}
impl ::core::clone::Clone for Datapoint {
    fn clone(&self) -> Datapoint {
        Datapoint {
            path: ::core::clone::Clone::clone(&self.path),
            value: ::core::clone::Clone::clone(&self.value),
            source_ts: ::core::clone::Clone::clone(&self.source_ts),
        }
    }
}
impl ::core::cmp::PartialEq for Datapoint {
    fn eq(&self, other: &Datapoint) -> bool {
        self.path == other.path && self.value == other.value && self.source_ts == other.source_ts
    }
}
impl ::core::default::Default for Datapoint {
    fn default() -> Datapoint {
        Datapoint {
            path: ::core::default::Default::default(),
            value: ::core::default::Default::default(),
            source_ts: ::core::default::Default::default(),
        }
    }
}
```

Two details matter in practice:

- **Absolute paths** (`::core::clone::Clone`) are used so the expansion works
  even if the user has their own `Clone` or `Debug` in scope. Write your own
  macros the same way (`::std::vec::Vec`, not `Vec`).
- **Derive bounds every type parameter.** `#[derive(Clone)] struct Shared<T> { inner: Arc<T> }`
  becomes `impl<T: Clone> Clone for Shared<T>`, even though cloning an
  `Arc<T>` never clones `T`. When that bound is wrong, write the impl by hand
  (exercise c18_e05). Tokio, tower and many other libraries write
  `Clone` by hand for exactly this reason.

`#[derive(Deserialize)]` is serde's derive, which turns data formats such as JSON into
your structs (serde gets its own chapter, 28; here you only need that one-line
idea). It expands into much more: a visitor struct, a field
enum, and a `deserialize` method that drives serde's data model. You never
need to write that by hand. But when a value needs validation (such as "id must
be ≥ 0"), the idiom is a manual `impl<'de> Deserialize<'de>` that
deserializes a simpler type first and then checks it, using
`D::Error::custom(...)`.

**Seeing expansions yourself.** The `cargo-expand` tool
(`cargo install cargo-expand`; it drives the compiler's unstable
`-Zunpretty=expanded` output) prints a crate with every macro expanded:

```sh
cargo expand -p databroker broker          # expand one module
cargo expand --test c18_e05_reading_derive_output -p solutions
```

It isn't installed in this course environment, so the expansions in this
guide were written out by hand from the std and serde sources. rust-analyzer
gives you the same thing interactively: put the cursor on a macro call and
run **"Expand macro recursively"**.

## Procedural macros

A procedural macro is a Rust function the compiler calls during compilation:

```rust
#[proc_macro_derive(SignalNames, attributes(signal))]
pub fn derive_signal_names(input: TokenStream) -> TokenStream { ... }
```

Rules of the game:

- It must live in its **own crate** with `[lib] proc-macro = true` in its
  `Cargo.toml`. That crate is compiled for the *host* (the machine running the
  compiler), because it runs inside the compiler. Here that's
  `learn-rust/chapters/18-macros/exercise-macros` (your version) and `learn-rust/chapters/18-macros/solution-macros`
  (the reference). The exercises import it as `macros`.
- `TokenStream` in, `TokenStream` out. Nobody works with raw tokens directly:
  - **`syn`** parses tokens into a syntax tree (`DeriveInput`, `Data::Struct`,
    `Fields::Named`, `Attribute`, …);
  - **`quote!`** builds tokens back from Rust-looking code with
    interpolation: `#name` inserts a variable and `#(#items),*` repeats, like
    `$( ... ),*` in `macro_rules!`;
  - **`proc-macro2`** is the token type that `syn` and `quote` share (and it
    works in unit tests, unlike `proc_macro`).
- `attributes(signal)` registers `#[signal(...)]` as a **helper attribute**,
  so the compiler accepts it on the struct and fields. Only your macro reads it.
- **Errors are values.** Don't `panic!`, which gives "proc macro panicked" with
  no useful location. Return a `syn::Error` built with
  `syn::Error::new_spanned(tokens, "message")` and convert it with
  `into_compile_error()`. The user then sees your message underlined at the
  exact offending tokens.
- `input.generics.split_for_impl()` gives you `impl<...>`, `Type<...>` and
  `where ...` so that generic structs work.

The shape of almost every derive macro:

```rust
#[proc_macro_derive(SignalNames, attributes(signal))]
pub fn derive_signal_names(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    // 1. read attributes   2. walk fields   3. collect what you need
    let name = &input.ident;
    let (impl_g, ty_g, where_c) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_g #name #ty_g #where_c {
            pub const SIGNAL_PATHS: &'static [&'static str] = &[#(#paths),*];
        }
    })
}
```

Attribute macros (`#[tokio::main]`, `#[async_trait]`) have the signature
`fn(attr: TokenStream, item: TokenStream) -> TokenStream`, and the output
*replaces* the item. `#[tokio::main]` turns `async fn main()` into a normal
`fn main()` that builds a runtime and `block_on`s your body.
`#[async_trait]` rewrites every `async fn` in a trait into a method that
returns `Pin<Box<dyn Future<Output = T> + Send + 'a>>` (chapter 23 explains why
that's needed for `dyn` traits).

Costs to keep in mind: proc-macro crates add compile time (`syn` with
`full` features is a big dependency), their errors can be confusing if written
carelessly, and generated code is invisible in the source. That's why the
guideline is "function first, `macro_rules!` second, proc macro last".

## Mental model

| Idea | Analogy |
|---|---|
| `macro_rules!` | A mail-merge template: the pattern finds the blanks, the template pastes them in |
| Fragment specifier | The *shape* of the blank ("a name goes here", "an expression goes here") |
| Hygiene | Ink that only the macro can see: its local variables can't smudge yours |
| tt munching | Eating a sandwich one bite at a time, keeping a plate of what you've processed so far |
| Derive macro | A clerk who reads your struct's blueprint and files the paperwork (impls) for it |
| Attribute macro | An editor who rewrites the whole item before the compiler sees it |

**Guarantee:** whatever a macro produces is type-checked and borrow-checked
exactly like hand-written code. Macros can't bypass Rust's safety rules. They
only save typing.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Macro arguments are evaluated like function arguments." | They're pasted as tokens. A fragment mentioned twice runs twice (c18_e01). |
| "I can call a macro before its definition, like a function." | `macro_rules!` is textually scoped: "cannot find macro `x` in this scope". |
| "Macros can inspect types." | They run before type checking and only see tokens. Use generics and traits for type-driven logic. |
| "`#[derive(Clone)]` just clones the fields." | It also adds `T: Clone` for every type parameter, which is sometimes wrong. |
| "Proc macros are magic compiler plugins." | They're ordinary Rust functions from tokens to tokens, in their own crate. |
| "`panic!` is fine for bad macro input." | Return `syn::Error` so the error points at the user's code. |

## Smells and idioms

| Smell | Idiomatic form |
|---|---|
| A macro where a generic function would do | Write the function. Macros are harder to read, document and debug. |
| `$x * $x` with an `expr` fragment | Bind once: `{{ let v = $x; v * v }}` |
| Relative paths in generated code (`Vec`, `HashMap`) | Absolute paths: `::std::vec::Vec`, `::std::collections::HashMap` |
| An exported macro naming its own crate's items as `mycrate::Thing` | `$crate::Thing`: `$crate` always resolves to the crate that *defined* the macro, whatever the caller renamed it to (see `indexmap!`) |
| No trailing-comma support | Add `$(,)?` |
| A giant single-rule macro | Internal `@rules` with a clear accumulator |
| `panic!` / `unwrap` inside a proc macro | `syn::Error::new_spanned(..).into_compile_error()` |
| `lazy_static!` in new code | `std::sync::LazyLock` / `OnceLock` (chapter 12). The databroker still uses `lazy_static!` for `ALLOW_ALL` (`permissions.rs:21`). |
| Hand-written lists that must stay in sync across files | Consider one macro table, or at least a test that checks every variant round-trips |

## In the databroker and in the wild

In the databroker:

| Where | What to notice |
|---|---|
| `databroker-proto/src/lib.rs:23` and `:143` | `tonic::include_proto!("kuksa.val.v2")` is a one-line `macro_rules!` that expands to `include!(concat!(env!("OUT_DIR"), "/kuksa.val.v2.rs"))`. The gRPC types are generated by `build.rs` and pasted in here (chapter 30). |
| `databroker/src/permissions.rs:21` | `lazy_static! { pub static ref ALLOW_ALL: ... }`: a macro that builds a lazily initialised global. |
| `databroker/src/broker.rs:188`, `:197` | `#[async_trait::async_trait]` on `ActuationProvider` / `SignalProvider`: an attribute proc macro rewriting async methods. |
| `databroker/src/grpc/kuksa_val_v2/val.rs:250` | `#[tonic::async_trait]`: the same macro, re-exported by tonic. |
| `databroker/src/broker.rs:105`, `databroker/src/vss.rs:39` | Derives: `Debug, Clone, PartialEq` and serde's `Deserialize`. |
| `databroker/src/main.rs:233`, `:245` | `option_env!` and `concat!`: compile-time strings for `--version`. |
| `databroker/src/lib.rs:47` | `format_args!`: the zero-allocation building block under `format!`/`println!`. |
| `databroker/src/broker.rs:2531` | `#[tokio::test]`: builds a runtime around each async test. |
| `databroker/src/broker.rs:275` | `#[cfg_attr(feature="otel", tracing::instrument(...))]`: an attribute macro applied only when a feature is on. |
| `databroker-cli/src/cli.rs:36` | `#[derive(Debug, Parser, Clone)]`: clap's derive generates the whole argument parser (chapter 31). |

In the wild (sources on disk under
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`):

| File | What to read |
|---|---|
| `serde_json-1.0.151/src/macros.rs:54` (`json!`) and `:70` (`json_internal!`) | A large, well-commented tt-muncher: internal `@array`/`@object` rules and accumulators. |
| `anyhow-1.0.104/src/macros.rs:58` (`bail!`) and `:204` (`anyhow!`) | Small, practical macros with several rules for different argument shapes. |
| `indexmap-2.14.2/src/macros.rs:91` | `indexmap!{}`: the `hashmap!` from c18_e02, done properly. It counts keys in a `const` block to pre-size, and uses `$crate::IndexMap` so it works however the caller imported the crate. |
| `lazy_static-1.5.0/src/lib.rs:161` | How `lazy_static!` expands into a hidden type plus a `Deref` impl. |
| `tonic-0.14.6/src/macros.rs:31` | `include_proto!` in full: three lines. |
| `tokio-1.53.1/src/macros/select.rs:559` | `select!`: one of the most complex `macro_rules!` macros in the ecosystem. Skim the internal rules. |
| `tokio-macros-2.7.2/src/lib.rs:314` | `#[tokio::main]` as an attribute proc macro. |
| `async-trait-0.1.92/src/lib.rs:254` | The `#[async_trait]` entry point. The docs at the top of the file show the expansion. |
| `serde_derive-1.0.229/src/lib.rs:121` | `#[proc_macro_derive(Deserialize, attributes(serde))]`: same shape as your `SignalNames`. |
| `clap_derive-4.6.7/src/lib.rs:54` | `#[proc_macro_derive(Parser, attributes(clap, ...))]`. |

## Exercises

| Exercise | Goal |
|---|---|
| c18_e01_macro_rules_basics | Definition order, fragment kinds, evaluate-once, hygiene |
| c18_e02_repetition | `$(...),*`, trailing commas, nested repetition, `count!` |
| c18_e03_recursive_and_tt_munching | Recursive `max_of!`, a `glob!` tt-muncher, a small signal-declaration DSL |
| c18_e04_macros_that_generate_items | One table generates impls for many types, an enum, `Display`/`FromStr`, and tests |
| c18_e05_reading_derive_output | Write by hand what `derive(Debug, Clone, PartialEq, Default)` generates; a bound-free `Clone`; a validating `Deserialize` |
| c18_e06_write_a_derive_macro | A real `#[derive(SignalNames)]` with `syn`/`quote`, helper attributes, generics and proper errors |
| c18_e07_review_macro_hygiene | Review: fix double-evaluation of a macro argument, a repetition reusing the wrong metavariable, and an off-by-one base case |

## Further reading

- The Rust Book, chapter 20.5 "Macros": <https://doc.rust-lang.org/book/ch20-05-macros.html>
- *The Little Book of Rust Macros*: <https://veykril.github.io/tlborm/> (the definitive `macro_rules!` guide, including tt munching)
- The Rust Reference, "Macros By Example" and "Procedural Macros": <https://doc.rust-lang.org/reference/macros.html>
- `syn` docs: <https://docs.rs/syn/2> (see `DeriveInput`, `Attribute::parse_nested_meta`)
- `quote` docs: <https://docs.rs/quote/1>
- David Tolnay's `proc-macro-workshop` (hands-on exercises): <https://github.com/dtolnay/proc-macro-workshop>

## Check yourself

1. Why does `square!(next())` with template `$x * $x` call `next()` twice, and how do you fix it?
2. Which fragment specifier do you need for a struct name, a type, and "anything at all"?
3. In a tt-muncher handling both `*` and `**`, which rule must come first, and why?
4. What does `#[derive(Clone)]` generate for `struct Shared<T> { inner: Arc<T> }`, and why might you write it by hand?
5. Why must a procedural macro live in its own crate?
6. How should a derive macro report "this only works on structs"?
7. What does `tonic::include_proto!("kuksa.val.v2")` expand to?

<details><summary>Answers</summary>

1. Macro arguments are pasted as tokens, not evaluated first, so the
   expression appears twice in the expansion. Bind it once inside a block:
   `{{ let v = $x; v * v }}`.
2. `ident`, `ty`, and `tt` (a single token tree; use `$($t:tt)*` for any sequence).
3. The `* *` rule, because `**` is two `*` tokens and rules are tried top to
   bottom. The single-`*` rule would otherwise match first and `**` would
   never be recognised.
4. `impl<T: Clone> Clone for Shared<T>`. The `T: Clone` bound is unnecessary,
   because cloning an `Arc` only bumps a counter, and it prevents cloning
   `Shared<NotClone>`. A manual `impl<T> Clone` avoids it.
5. It is compiled for the host and loaded into the compiler as a plugin
   (`proc-macro = true`), so it can't be part of the crate being compiled.
6. Return a `syn::Error::new_spanned(&input.ident, "...")` and convert it with
   `into_compile_error()`, rather than panicking.
7. `include!(concat!(env!("OUT_DIR"), "/kuksa.val.v2.rs"))`, which pastes the
   Rust file that `build.rs` generated from the `.proto` files.
</details>
