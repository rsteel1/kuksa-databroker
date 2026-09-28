# 08 — Modules, crates & Cargo

**Why this matters for the databroker.** Before you can read a line of
`broker.rs` you need to know *where things are* and *why the build does
what it does*: why `DataValue` is reachable as both `databroker::types::DataValue`
and `databroker::broker::DataValue`; why the gRPC code lives in private
modules yet works; why `cargo build` runs a C++ compiler; where the
`proto::val_server::Val` trait even comes from (there's no `.rs` file
defining it!); and why some code only exists when you pass `--features viss`.

## Crates, packages and targets

- A **crate** is one compilation unit: a library (`lib.rs`) or a binary
  (`main.rs`). The crate is the unit of `pub(crate)` visibility and of
  parallel compilation.
- A **package** is a directory with a `Cargo.toml`. It can contain one
  library crate and any number of binaries, examples, tests and benches
  (**targets**). `databroker/` has a library (`src/lib.rs`, declared under
  `[lib]` in `databroker/Cargo.toml:21`) *and* a binary (`src/main.rs`,
  discovered automatically). `main.rs` uses the library like any external
  crate: `use databroker::broker;`.
- Every file in `tests/` is its own crate that links the library — exactly
  how this course's exercises are built.

## Modules and privacy

```rust
// lib.rs
pub mod broker;   // contents in broker.rs (or broker/mod.rs)
pub mod glob;
mod helpers { ... } // inline module
```

`mod x;` *declares* a module and tells the compiler to load `x.rs` or
`x/mod.rs`. Files are not modules until some parent declares them — there's
no implicit "every file is compiled".

**Everything is private by default.** An item is visible in the module
where it's defined and all of that module's descendants. `pub` makes it
visible to whoever can see the parent. Rules worth memorising:

- Struct fields are private individually, even in a `pub struct`. Outside the
  module nobody can write `DataBroker { database: ..., ... }` — they must use
  `DataBroker::new`. This is how a type guarantees its invariants.
- Enum variants are public if the enum is public.
- Trait **impls** have no visibility: `impl Val for DataBroker` inside a
  private module still applies everywhere the trait and type are visible.
  That's why `databroker/src/grpc/mod.rs` can declare `mod kuksa_val_v2;`
  privately: nobody needs to *name* that module, only the impl matters.

## Paths, use and re-exports

- `crate::a::b` — absolute path from the crate root.
- `super::b` — from the parent module. `self::b` — from the current module.
- A `use` path is resolved relative to the current module (2018+ editions),
  so inside `mod grpc`, `use broker::X` looks for `grpc::broker` — usually
  not what you want. Error **E0432** ("unresolved import").
- External crates are in scope by name: `use tokio::sync::RwLock;`.

`use` just creates a local shortcut. `pub use` **re-exports**: the item
becomes reachable under a second path. `databroker/src/broker.rs:16-18`:

```rust
pub use crate::types;
pub use crate::types::{ChangeType, DataType, DataValue, EntryType, SignalId, TimeInterval};
```

So users (and the gRPC layer) write `broker::DataValue`. The library shapes
its public API independently of its file layout. A re-export can't be more
visible than the item itself (**E0365**).

## Restricted visibility

| Syntax | Visible to |
|---|---|
| (nothing) | this module and its children |
| `pub(super)` | the parent module (and its children) |
| `pub(in crate::a)` | everything inside `crate::a` |
| `pub(crate)` | the whole crate, but not other crates |
| `pub` | anyone who can see the parent |

Choose the narrowest one that works; it tells readers "this is internal".
(The databroker mostly uses plain `pub`, because `main.rs` and the tests are
separate crates that need access — `pub(crate)` would hide things from them.)

## Workspaces

The repo root `Cargo.toml` is a **virtual workspace**: no package of its own,
just a list of members that share one `Cargo.lock` and one `target/` dir:

```toml
[workspace]
members = ["databroker", "databroker-proto", "databroker-cli"]
exclude = ["lib/*"]

[workspace.dependencies]
tokio = "1.46.1"
tonic = { version = "0.14.0", default-features = false }
...
```

- Members inherit versions with `tokio = { workspace = true, features = [...] }`
  (see `databroker/Cargo.toml:28`–`31`). One place to bump a version.
- `lib/*` is **excluded**: `lib/Cargo.toml` is a *separate* workspace with the
  client libraries (`kuksa`, `kuksa-common`, `kuksa_val_v2`,
  `databroker-examples`). The databroker still depends on two of them by path
  (`kuksa-common = { path = "../lib/common" }`), but they're versioned and
  released separately.
- This course's `learn-rust/` is yet another independent workspace (its own
  `[workspace]` table), so it doesn't disturb the real build.

## Conditional compilation and features

`#[cfg(predicate)]` removes an item at compile time when the predicate is
false. The removed code isn't type-checked — it simply doesn't exist.
`cfg!(predicate)` is instead a `bool` expression; both branches must compile.
`#[cfg_attr(predicate, attr)]` applies `attr` only when the predicate holds.

**Cargo features** are named, additive compile-time switches defined in
`Cargo.toml` (`databroker/Cargo.toml:83`):

```toml
[features]
default = ["tls"]
tls = ["tonic/tls-ring", "kuksa-common/tls", "kuksa/tls"]
jemalloc = ["dep:jemallocator"]
viss = ["dep:axum", "dep:chrono"]
otel = ["dep:chrono", "dep:opentelemetry", ...]
```

- A feature can enable features of dependencies (`tonic/tls-ring`) and
  optional dependencies (`dep:axum` — `axum` is declared `optional = true`
  at line 69 and is only compiled when `viss` is on).
- Build variants: `cargo build --features viss,otel`, or
  `cargo build --no-default-features` (drops TLS). This course depends on
  `databroker` with `default-features = false`.
- Features must be **additive**: turning one on must never break code that
  worked without it, because Cargo unifies features across the dependency
  graph.

How the databroker uses them:

```rust
// databroker/src/lib.rs:24
#[cfg(feature = "viss")]
pub mod viss;               // the whole VISS WebSocket server

// databroker/src/lib.rs:40 and :59 — two versions of the same function
#[cfg(not(feature = "otel"))]
pub fn init_logging() { /* plain tracing-subscriber */ }
#[cfg(feature = "otel")]
pub fn init_logging() { /* + OpenTelemetry exporter */ }

// databroker/src/grpc/server.rs:109 — a *parameter* that only exists with TLS
#[cfg(feature = "tls")] server_tls: ServerTLS,

// databroker/src/main.rs:14 — swap the global allocator
#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// databroker/src/main.rs:613 — platform, not feature
#[cfg(target_os = "linux")]
// ... sd_notify::notify(false, &[sd_notify::NotifyState::Ready])
```

And the pattern you'll see on dozens of methods in `broker.rs`:

```rust
#[cfg_attr(feature = "otel", tracing::instrument(name = "subscriptions_notify", skip(self, changed, db)))]
pub async fn notify(...)
```

"Only when `otel` is on, wrap this function in a tracing span."

Other predicates: `test` (set by `cargo test`), `debug_assertions` (off in
`--release`), `unix`, `windows`, `target_arch = "aarch64"`.

## build.rs, OUT_DIR and generated code

A package may have a `build.rs`: a small program Cargo compiles and runs
*before* compiling the crate. It can generate code, compile C, or emit
instructions to Cargo (`cargo:rustc-env=...`, `cargo:rerun-if-changed=...`).
Its dependencies go under `[build-dependencies]`.

The databroker has two:

1. **`databroker/build.rs`** uses `vergen-gix` to emit env vars like
   `VERGEN_GIT_SHA` at build time. `main.rs:235` reads them with
   `option_env!("VERGEN_GIT_SHA")` — a macro evaluated *at compile time*.
   That's how `databroker --version` knows its commit.

2. **`databroker-proto/build.rs`** generates all the gRPC code:

   ```rust
   std::env::set_var("PROTOC", protobuf_src::protoc()); // build protoc from C++ source
   tonic_prost_build::configure()
       .compile_protos(&["proto/kuksa/val/v2/val.proto", ...], &["proto"])?;
   ```

   `protobuf-src` compiles Google's `protoc` from source (that's the C++ you
   see during the first build — and why a broken C++ toolchain breaks the
   Rust build). `tonic-prost-build` runs `protoc` over the `.proto` files and
   writes Rust files into **`OUT_DIR`** — a per-crate scratch directory under
   `target/` that Cargo passes to build scripts as an env var
   (`databroker-proto/build.rs:31`).

   Then `databroker-proto/src/lib.rs:143` pulls the generated file in:

   ```rust
   pub mod kuksa { pub mod val { pub mod v2 {
       tonic::include_proto!("kuksa.val.v2");   // = include!(concat!(env!("OUT_DIR"), "/kuksa.val.v2.rs"))
   } } }
   ```

   So `databroker_proto::kuksa::val::v2::val_server::Val` — the trait the
   databroker implements — is *generated*. To read it, open
   `target/debug/build/databroker-proto-*/out/kuksa.val.v2.rs`, or let
   rust-analyzer jump there.

## The proto submodule

The `.proto` files aren't in this repo. `.gitmodules` points `proto/` at
`https://github.com/eclipse-kuksa/kuksa-proto.git`, pinned to a commit, and
`databroker-proto/proto` is a symlink to `../proto/`. After cloning you must
run:

```sh
git submodule update --init
```

Otherwise the build fails with `protoc failed: Could not make proto path
relative: proto/kuksa/val/v1/val.proto: No such file or directory`.
Updating the API = bumping the submodule commit, rebuilding, and fixing
whatever no longer compiles.

## Mental model

| Concept | Analogy |
|---|---|
| crate | a book (compiled as a whole) |
| module | a chapter; private items are the author's notes in the margin |
| `pub` | printing it in the table of contents of the enclosing chapter |
| `pub use` | a cross-reference: "see also DataValue, p. 12" |
| workspace | a book series sharing one printer (`target/`) and one parts list (`Cargo.lock`) |
| feature | optional appendices you choose when ordering the book |
| `#[cfg]` | pages physically removed before printing |
| `cfg!()` | a sentence that says "if you're reading the Linux edition, ..." |
| build.rs | the typesetter that runs before printing and can add generated pages |

**Guarantee:** nothing outside a module can touch its private items, so a
type's invariants can only be broken by code in the same module.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Every `.rs` file is compiled" | Only files declared with `mod` from the crate root | "file not included in module tree" |
| "`use foo::X` searches from the crate root" | Relative to current module; use `crate::`/`super::` | E0432 / E0433 |
| "`pub struct` makes fields public" | Fields need their own `pub` | E0616 / E0451 |
| "private module ⇒ its trait impls are hidden" | Impls are global | — |
| "`#[cfg(feature = \"x\")]` code is still type-checked" | It's removed entirely; bugs can hide in rarely-built features — CI should build feature combinations | — |
| "features can remove functionality" | They must be additive | — |

## Smells and idioms

- `pub` on everything "just in case" — expose the minimum.
- Deep `use super::super::super::x` — use `crate::` paths instead.
- Glob imports (`use foo::*`) outside tests and preludes hide where names come from.
- Feature-gated code nobody builds in CI rots. The databroker's CI runs
  clippy with `--features viss` and coverage tests with `--all-features`
  (`.github/workflows/kuksa_databroker_build.yml:74` and `:156`); locally,
  try `cargo check --features viss,otel` before sending a PR that touches `broker.rs`.
- `build.rs` doing network access or slow work on every build — use
  `cargo:rerun-if-changed`.
- **In the databroker:** `lib.rs` exposes every module `pub` (so `main.rs`
  and the integration tests can use them). It's an app with a library
  inside, not a published library, so that's a reasonable trade-off.

## In the databroker

- `Cargo.toml:14` onwards — the workspace: members, `exclude = ["lib/*"]`,
  `[workspace.dependencies]`, release profile (`lto`, `opt-level = "s"`,
  `strip`) tuned for small binaries in cars.
- `databroker/Cargo.toml:83`–`89` — features; `:62`–`77` optional deps;
  `:91` build deps; `:95` dev-deps (test-only, e.g. `cucumber`); `:102`–`104`
  a custom integration test with `harness = false` (cucumber brings its own `main`).
- `databroker/src/lib.rs:14`–`25` — the module tree; `viss` is feature-gated.
- `databroker/src/lib.rs:40`/`:59` — `init_logging` compiled one of two ways.
- `databroker/src/broker.rs:16`–`18` — re-exports.
- `databroker/src/grpc/mod.rs` — `pub mod server; mod kuksa_val_v1; mod kuksa_val_v2;`.
- `databroker/src/main.rs:14`, `:36`, `:613` — `jemalloc`, `viss`, linux-only systemd notify.
- `databroker-proto/build.rs:16` and `:31` — vendored `protoc`, `OUT_DIR`.
- `databroker-proto/src/lib.rs:23` and `:143` — `tonic::include_proto!`.
- `.gitmodules` — the `proto` submodule.

## Exercises

| Exercise | Goal |
|---|---|
| `c08_e01_inline_modules_and_pub` | make exactly the right things `pub` |
| `c08_e02_use_paths_and_reexports` | fix `use` paths with `crate::`/`super::`; add `pub use` |
| `c08_e03_visibility_pub_crate` | `pub(super)` vs `pub(crate)` |
| `c08_e04_cfg_attributes` | `#[cfg]`, `cfg!`, `#[cfg_attr]`, test-only code |
| `c08_e05_build_signal_store_module` | Build: a module with `pub use` re-exports and a small store, from a spec |
| `c08_e06_review_module_boundary` | Review: find 3 planted bugs (off-by-one, swallowed `Result` at a module boundary, shadowing) |

## Further reading

- The Rust Book, ch. 7 "Managing Growing Projects with Packages, Crates, and Modules" — https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html
- The Rust Book, ch. 14.3 "Cargo Workspaces" — https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
- The Cargo Book: "Features" and "Build Scripts" — https://doc.rust-lang.org/cargo/reference/features.html , https://doc.rust-lang.org/cargo/reference/build-scripts.html
- The Rust Reference, "Conditional compilation" — https://doc.rust-lang.org/reference/conditional-compilation.html
- `tonic-prost-build` — https://docs.rs/tonic-prost-build

## Check yourself

1. Why can `grpc/mod.rs` declare `mod kuksa_val_v2;` without `pub`, yet
   clients can still call the v2 API?
2. What's the difference between `#[cfg(feature = "otel")]` and `cfg!(feature = "otel")`?
3. Where does the Rust source for `val_server::Val` come from, and where on
   disk can you read it?
4. You cloned the repo and the build says it can't find `val.proto`. Why?
5. Why is `lib/` excluded from the root workspace?
6. Why must features be additive?

<details><summary>Answers</summary>

1. The module only contains trait impls (`impl Val for DataBroker`) plus
   helpers; impls apply globally regardless of module privacy. `server.rs`
   builds the tonic service from `DataBroker` directly.
2. `#[cfg]` removes the item at compile time (not type-checked when off);
   `cfg!` is a boolean expression and both branches must compile.
3. `databroker-proto/build.rs` runs `protoc` + `tonic-prost-build`, writing
   into `OUT_DIR` (`target/debug/build/databroker-proto-*/out/`); `include_proto!`
   includes it.
4. The `proto` git submodule isn't checked out: `git submodule update --init`.
5. It's a separate workspace for client libraries with their own lock file
   and release cycle.
6. Cargo unifies features: if any crate in the graph enables a feature, every
   user gets it. A feature that removed something would break other users.
</details>

## Where this goes next

Chapter 18 shows how `include_proto!` and derives generate code. **Chapter 33** goes deeper on Cargo: semver, feature unification in practice, editions and MSRV, and API design. Chapter 34 uses `Cargo.toml` as the entry point for reading any codebase.
