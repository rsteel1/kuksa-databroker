# 33 — Cargo, the ecosystem & API design

**Why this matters.** Up to now you've *used* crates. This chapter is about
the contract between crates: what a version number promises, how features
combine when many crates share a dependency, how to design an API other
people can use without reading your source, and how to judge a crate before
you depend on it. The databroker's own manifests are the examples, but the
same rules govern every crate on crates.io. When you open an unfamiliar
project, `Cargo.toml` is the first file you read (chapter 34), so this is also
the first step of reading any codebase.

Chapter 08 introduced packages, crates, modules, workspaces and `build.rs`.
This chapter builds on that.

---

## SemVer: the contract behind every version number

Cargo assumes every crate follows [Semantic Versioning](https://semver.org):
`MAJOR.MINOR.PATCH`.

| Bump | Meaning | Example |
|---|---|---|
| PATCH | bug fixes only; nobody's code can break | 1.46.1 → 1.46.2 |
| MINOR | new things added; existing code still compiles | 1.46.1 → 1.47.0 |
| MAJOR | something existing changed or went away | 1.46.1 → 2.0.0 |

**Before 1.0 everything shifts one place right.** In `0.14.6`, a breaking
change bumps the *minor* number (`0.15.0`), and everything else bumps the
patch. That's why the repository's root `Cargo.toml:36` can say
`tokio = "1.46.1"` and silently get tokio 1.53.1, while `Cargo.toml:39` says
`tonic = "0.14.0"` and will never get tonic 0.15.

### What `version = "1.2.3"` actually means

A bare version is a **caret requirement**, `^1.2.3`:

| Requirement | Accepts |
|---|---|
| `"1.2.3"` = `^1.2.3` | `>=1.2.3, <2.0.0` |
| `"0.14"` = `^0.14.0` | `>=0.14.0, <0.15.0` |
| `"0.0.3"` | exactly `0.0.3` |
| `"~1.2.3"` | `>=1.2.3, <1.3.0` |
| `"=1.2.3"` | exactly that version |

`Cargo.lock` records which versions were actually picked. Applications
(like the databroker binary) commit their lockfile so every build is
reproducible, and `cargo update` moves it forward within the requirements.

### Which changes are breaking?

The Cargo book has a complete list ("SemVer Compatibility"). These are the
ones you'll meet most:

| Change | Why | Bump |
|---|---|---|
| Remove or rename a `pub` item | callers named it | major |
| Change a function's signature | callers passed the old types | major |
| Add a `pub` field to a struct whose fields are all `pub` | `Datapoint { ts, value }` literals and exhaustive destructuring no longer list every field | major |
| Add a field to a struct that already has a private field | nobody outside could write a literal anyway | minor |
| Add an enum variant | exhaustive `match`es stop compiling | major |
| …unless the enum is `#[non_exhaustive]` | callers were forced to write `_ =>` | minor |
| Add a trait method without a default | every implementor breaks | major |
| Add a trait method with a default | (technically can cause ambiguity; the book calls it minor) | minor |
| Remove a Cargo feature | `features = ["x"]` stops resolving | major |
| Raise the minimum Rust version | the book allows it in a minor release; many crates treat it as policy | minor |

`#[non_exhaustive]` exists precisely so a library can grow its error enums
without a major release. The databroker's `UpdateError`
(`databroker/src/broker.rs:64`) isn't marked, so adding a variant would be
breaking for anyone matching on it. That's fine for an application crate, but
something to fix before publishing a library.

Tools: `cargo semver-checks` compares your crate's public API against the last
published version and reports breaking changes automatically.

## Features: additive switches, unified per build

Features are named switches in `[features]` that turn on optional code and
optional dependencies. The databroker's (`databroker/Cargo.toml:83-89`):

```toml
[features]
default = ["tls"]
tls = ["tonic/tls-ring", "kuksa-common/tls", "kuksa/tls"]
jemalloc = ["dep:jemallocator"]
viss = ["dep:axum", "dep:chrono"]
libtest = []
otel = ["dep:chrono", "dep:opentelemetry", ...]
```

| Entry form | Meaning |
|---|---|
| `"other"` | turn on another feature of this crate |
| `"dep:axum"` | compile the optional dependency `axum` (declared with `optional = true`) |
| `"tonic/tls-ring"` | turn on feature `tls-ring` of dependency `tonic` (and enable `tonic` if optional) |
| `"serde?/std"` | *weak*: if `serde` is enabled anyway, turn on its `std` feature; never enables `serde` itself |

`default` is requested automatically unless someone writes
`default-features = false` (as `learn-rust/Cargo.toml` does for the
databroker, which is why the course builds without TLS).

**Unification.** If two crates in one build depend on `tokio` with
features `["sync"]` and `["net"]`, Cargo compiles tokio *once* with both.
You can't have two copies of the same crate version with different features.
Consequences:

- **Features must be additive.** Turning a feature on may add code, never
  remove or change existing behaviour. A feature that does
  `#[cfg(not(feature = "x"))] fn f() { A } #[cfg(feature = "x")] fn f() { B }`
  can silently change behaviour for a crate that never asked for `x`.
- `cargo build -p databroker` and `cargo build` at the workspace root can
  compile a dependency with different features, because different crates are
  in the build.
- `resolver = "2"` (repository `Cargo.toml:16`) stops dev-dependency and
  build-dependency features from leaking into normal builds. Edition 2021
  crates get it by default, but a *virtual* workspace (one with no
  `[package]`) has no edition, so it must say so explicitly.

`cargo tree -e features -i tonic` shows which crate turned on which feature.

## Workspaces, inheritance and profiles

Chapter 08 covered the basics. The details that matter in practice:

- `[workspace.dependencies]` (repository `Cargo.toml:28`) declares versions
  once; members write `tokio = { workspace = true, features = [...] }`
  (`databroker/Cargo.toml:28-29`). The features are additive on top of the
  workspace entry.
- `exclude = ["lib/*"]` (`Cargo.toml:24`) keeps the client libraries out of the
  workspace: they have their own `Cargo.lock` and are built separately.
- Profiles live only at the workspace root. The databroker's release profile
  (`Cargo.toml:43-46`) sets `lto = true`, `opt-level = "s"` and
  `codegen-units = 1`: slower builds for a smaller, faster binary on
  in-vehicle hardware. `[profile.release-with-debug]` (`Cargo.toml:50`)
  `inherits = "release"` but keeps symbols for profiling (chapter 27).
- Platform-specific dependencies: `[target.'cfg(target_os = "linux")'.dependencies]`
  (`databroker/Cargo.toml:80-81`) pulls in `sd-notify` only on Linux.
- `[[test]] ... harness = false` (`databroker/Cargo.toml:102-104`) lets the
  cucumber test binary supply its own `main`.
- `[lints]` tables (`databroker/Cargo.toml:106-107`) configure rustc and
  clippy lints in the manifest instead of `#![...]` attributes.

## Build scripts, generated code and `links`

`build.rs` runs *before* the crate compiles, on the host machine:

- `databroker/build.rs:16-30` uses `vergen-gix` to emit `cargo:rustc-env=...`
  lines, which is how `option_env!("VERGEN_GIT_SHA")` in `main.rs` gets a value.
- `databroker-proto/build.rs:17` builds a vendored `protoc` and points
  `PROTOC` at it; `:31` reads `OUT_DIR`, where the generated Rust is written.
  `databroker-proto/src/lib.rs:23` and `:143` pull it in with
  `tonic::include_proto!`. Chapter 34 reads that generated code.

Build scripts are arbitrary code that runs at build time. They're one reason
to look closely at new dependencies (next section).

## Choosing and auditing crates

Before adding a dependency, answer these:

1. **Is it maintained?** Recent releases, open-issue response, more than one
   maintainer. Check crates.io and the repository.
2. **Is it widely used?** Download counts and reverse dependencies on
   crates.io; [blessed.rs](https://blessed.rs) lists the de-facto standard
   crate for each job (serde, tokio, clap, thiserror/anyhow, tracing, ...).
3. **What does it pull in?** `cargo tree -p <crate>` before and after. A small
   helper that brings 60 transitive crates is a real cost in build time,
   supply-chain risk and audit burden.
4. **Do we already have it, maybe in another version?** `cargo tree -d` lists
   duplicates. The databroker currently builds `getrandom` 0.2 and 0.4,
   `socket2` 0.5 and 0.6, and `thiserror` 1 and 2, because different
   dependencies are on different major versions. That's normal, but each
   duplicate costs compile time and binary size.
5. **Proc-macros and build scripts?** They run code on your machine at build
   time. `cargo tree` marks proc-macros `(proc-macro)`.
6. **`unsafe` and `no_std`?** `cargo geiger` counts unsafe code;
   `#![forbid(unsafe_code)]` in a crate's `lib.rs` is a good sign for simple
   crates. For embedded targets, check whether it supports `no_std`.
7. **Licence.** The databroker is Apache-2.0 and ships a
   `scripts/licensecuration.yaml`; a GPL dependency would be a problem.

Reading `cargo tree`:

```text
databroker v0.7.2-dev.0 (../databroker)       depth 0: the root
├── async-trait v0.1.92 (proc-macro)           depth 1: direct dependency, runs at compile time
│   └── syn v3.0.4                             depth 2: its dependency
├── kuksa v0.7.2-dev.0 (../lib/kuksa)          local path dependency
│   └── tonic v0.14.6 (*)                      (*) = subtree already printed above
```

| Command | Answers |
|---|---|
| `cargo tree -p x` | what x depends on |
| `cargo tree -i y` | who depends on y (inverted) |
| `cargo tree -d` | crates present in more than one version |
| `cargo tree -e features` | which features are on, and who enabled them |
| `cargo tree -e normal` | ignore dev/build dependencies |
| `cargo deny check` | licences, bans, duplicates and advisories, driven by `deny.toml` |
| `cargo audit` | known vulnerabilities (RustSec advisory database) |
| `cargo outdated` / `cargo update --dry-run` | newer versions available |

## Designing APIs other people enjoy

The [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) are the
community's checklist. The rules with the biggest effect:

**Naming.**

| Prefix | Cost | Ownership | Example |
|---|---|---|---|
| `as_` | free | borrowed → borrowed | `str::as_bytes`, `as_map(&self) -> &HashMap` |
| `to_` | allocates or computes | borrowed → owned | `str::to_string`, `to_sorted_lines(&self)` |
| `into_` | usually free | owned → owned, consumes `self` | `String::into_bytes`, `into_inner` |

Getters don't use `get_`: `fn value(&self)`, not `fn get_value(&self)`. The
databroker breaks this rule throughout `AuthorizedAccess`
(`get_datapoint`, `broker.rs:1431`; `get_metadata`, `:1452`). Renaming
them now would touch hundreds of call sites for little gain, which is a
reasonable trade-off in an application. In a new library, get it right
from the start.

**Types over sentinels and flags.** Return `Option<f64>`, not `-1.0` for
"missing". Return `Result<T, E>` with an error enum, not `bool` plus a
`last_error` field: a `bool` can be ignored silently, while a `Result`
triggers the `unused_must_use` warning.

**Flexible parameters.** Take `&str` (not `&String`), `impl Into<String>` when
you'll store the value, `impl AsRef<Path>` for paths, and
`impl IntoIterator<Item = T>` for collections.

**Don't allocate on the caller's behalf.** Return `impl Iterator<Item = &str>`
instead of a fresh `Vec<String>`. The caller can `collect()` if they need to.

**Builders for many options.** `add_entry` has nine positional parameters
and needs `#[allow(clippy::too_many_arguments)]` (`broker.rs:1379-1380`).
A builder (or a struct of options with `Default`) makes each call
self-describing, and adding an option later isn't a breaking change.
Put `#[must_use]` on the builder so a forgotten `.build()` warns.

**Implement the common traits.** Every public type should implement `Debug`.
Add `Clone`, `PartialEq`, `Eq`, `Hash`, `Default` and `Display` where they make
sense, and `Send + Sync` should hold unless you have a reason. Error types
implement `std::error::Error + Send + Sync + 'static` (chapter 17).

**Leave room to grow.** Keep fields private and add accessors;
`#[non_exhaustive]` on enums and structs you expect to extend; sealed traits
(chapter 14) for traits users may call but not implement.

**Document.** `///` on every public item, with an `# Examples` section that
doubles as a doc test (chapter 32), plus `# Errors` and `# Panics` sections.

## Writing documentation with rustdoc

Documentation is part of a crate's API: docs.rs builds it for every
published version, and `cargo doc --open` builds it locally, including every
dependency.

* `///` documents the item that follows; `//!` documents the enclosing
  module or crate (the top of `lib.rs`). Both are Markdown.
* The first sentence is the summary shown in lists, so make it stand alone.
* Conventional headings: `# Examples` (compiled and run as doc tests, see
  chapter 32), `# Errors` (when it returns `Err`), `# Panics`, and `# Safety`
  (required on every `unsafe fn`, see chapter 26).
* **Intra-doc links** such as ``[`Permissions::can_read`]`` or ``[`crate::broker`]``
  are resolved by the compiler, and `cargo doc` warns when one breaks.
* Hide setup lines in examples with a leading `# ` (they still compile).
* `#![warn(missing_docs)]` makes undocumented public items a warning;
  `#![deny(rustdoc::broken_intra_doc_links)]` keeps links honest.
* `#[doc(hidden)]` removes an item from the docs without making it private
  (macro support code). `#[cfg_attr(docsrs, doc(cfg(feature = "tls")))]`
  shows feature requirements on docs.rs.

The databroker's API docs are sparse: `broker.rs` has a few dozen `///`
lines in about 5,700, and `permissions.rs` and `types.rs` have none. Its
behaviour is described in `doc/*.md` and in the `.proto` comments instead. Documenting its public
`broker.rs` API (`DataBroker`, `AuthorizedAccess`) with examples would make a
good first contribution.

## Editions and MSRV

An **edition** is a set of source-level rules chosen per crate
(`edition = "2021"` at `databroker/Cargo.toml:18`). Crates of different
editions link together freely. Editions let Rust fix syntax mistakes
without breaking old code.

| Edition | Changes you'll notice when reading code |
|---|---|
| 2015 | `extern crate foo;` everywhere; `Box<Trait>` without `dyn`; `try!()` |
| 2018 | `async`/`await` keywords; paths start with `crate::`; no `extern crate`; `dyn Trait` encouraged |
| 2021 | bare trait objects are errors; closures capture disjoint fields; `array.into_iter()` yields values; `TryFrom`/`TryInto`/`FromIterator` in the prelude; resolver 2 by default |
| 2024 | `-> impl Trait` captures all in-scope lifetimes (use `use<..>` to narrow); `gen` reserved; `unsafe extern` blocks; `if let ... && ...` let chains; `Box<[T]>` into_iter by value; `expr` fragment matches `const {}`/`_` |

`cargo fix --edition` migrates a crate automatically; read the Edition Guide
chapter for each edition before doing it.

The **MSRV** (minimum supported Rust version) is the oldest compiler that can
build a crate: the newest language or library feature it uses. Declare it
with `rust-version = "1.80"` in `[package]` so older compilers fail with a
clear message instead of a confusing error. The databroker doesn't declare
one. `cargo msrv` finds the real value by bisecting.

## `no_std` and publishing (brief)

- `#![no_std]` crates use only `core` (plus `alloc` if there's an allocator).
  They're essential for microcontrollers, and increasingly relevant in
  vehicles. Libraries often offer a `std` feature enabled by default, so
  `default-features = false` gives a `no_std` build.
- Publishing: `cargo publish --dry-run` checks the package. Crates.io
  versions are immutable (you can `yank` but never overwrite), which is why
  getting SemVer right matters. docs.rs builds your documentation
  automatically; `[package.metadata.docs.rs]` controls which features it uses.

## Mental model

| Idea | Analogy |
|---|---|
| SemVer | A contract printed on the box: "patch = same parts, minor = extra parts, major = different shape" |
| Caret requirement | "Any box from the same product line, this model or newer" |
| Cargo.lock | The receipt listing the exact boxes you bought |
| Feature unification | One shared toolbox per build: everyone's requested tools go into the same box |
| `#[non_exhaustive]` | "More options may be added" printed on a form |
| Edition | Spelling rules for a file, not a different language |

What the tools guarantee: Cargo guarantees a single, reproducible resolution
per build (given `Cargo.lock`), and the compiler guarantees crates of
different editions interoperate. Nothing guarantees authors follow SemVer.
That's a social contract, backed by tools like `cargo semver-checks`.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`tokio = "1.46.1"` pins that version." | It means `>=1.46.1, <2.0.0`; `Cargo.lock` is what pins. |
| "0.x crates follow the same rules as 1.x." | Before 1.0 the minor digit is the breaking one: `^0.14` excludes 0.15. |
| "Adding a field is never breaking." | It breaks struct literals and exhaustive patterns unless a private field or `#[non_exhaustive]` already prevents those. |
| "Features are per dependent crate." | They're unified per build: one compilation with the union of everyone's features. |
| "A feature may switch an implementation." | Features must be additive, because someone else's feature can switch your code. |
| "Editions split the ecosystem." | Crates of all editions link together; editions only change how one crate's source is read. |
| "Bare `Box<Trait>` is invalid Rust." | It's valid 2015/2018 code; since 2021 it's E0782 (write `dyn`). |

## Smells and idioms

| Smell | Better |
|---|---|
| `fn get_value(&self)` | `fn value(&self)` |
| `-1`, `""` or `u64::MAX` meaning "none" | `Option<T>` |
| `fn set(...) -> bool` plus a `last_error` field | `Result<T, ErrorEnum>` |
| `fn f(path: &String)` | `fn f(path: &str)` or `impl AsRef<str>` |
| returning `Vec<String>` of your own keys | `impl Iterator<Item = &str> + '_` |
| nine positional arguments | builder or options struct with `Default` |
| public error enum without `#[non_exhaustive]` in a library | add it before 1.0 |
| `version = "*"` | a real caret requirement |
| a feature that removes behaviour | an additive feature, or a runtime option |
| no `Cargo.lock` committed for a binary | commit it (since 2023 Cargo's guidance says libraries may commit theirs too) |

## In the databroker and in the wild

- `Cargo.toml:16-46` (repository root): `resolver = "2"`, members, `exclude`,
  `[workspace.dependencies]`, and the size-optimised release profile.
- `databroker/Cargo.toml:83-89`: the feature table. `c33_e02` reads this
  actual file.
- `databroker/Cargo.toml:80-81`: a target-specific dependency.
- `databroker/build.rs:16-30` and `databroker-proto/build.rs:17-31`: the two
  build scripts.
- `databroker/src/broker.rs:1379-1380`: `add_entry` and its
  `too_many_arguments` allowance; `:1431`/`:1452`: `get_` getters.
- `databroker/src/broker.rs:64`: `UpdateError`, a public enum without
  `#[non_exhaustive]`.
- In the wild: tokio's `Cargo.toml` (under
  `~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/Cargo.toml`) shows a
  big, carefully additive feature table (`full = [...]`, `sync = []`).
  serde's `std`/`alloc` features show the `no_std` pattern.

## Exercises

| Exercise | Goal |
|---|---|
| `c33_e01_semver_and_breaking_changes` | parse versions, classify changes, compute the next version, implement caret matching |
| `c33_e02_feature_flags_design` | write Cargo's feature resolver (`dep:`, `crate/feat`, weak `?/`) and run it on the databroker's manifest |
| `c33_e03_api_guidelines` | refactor an awkward API into an idiomatic one the tests describe |
| `c33_e04_editions_and_msrv` | fix 2015-style code, use 2021 semantics, answer an edition/MSRV quiz |
| `c33_e05_choosing_crates` | parse real `cargo tree` output and answer dependency questions in code |
| `c33_e06_review_semver_and_api_naming` | Review: 3 planted bugs — a breaking change classified as Minor, an `as i32` sign bug, a formatter dropping the patch number |

## Further reading

- The Cargo Book: "Specifying Dependencies", "Features", "Workspaces",
  "Build Scripts", and "SemVer Compatibility":
  <https://doc.rust-lang.org/cargo/>
- Rust API Guidelines, especially the checklist:
  <https://rust-lang.github.io/api-guidelines/checklist.html>
- The Edition Guide: <https://doc.rust-lang.org/edition-guide/>
- blessed.rs crate recommendations: <https://blessed.rs>
- `cargo-semver-checks`, `cargo-deny`, `cargo-audit` on docs.rs/crates.io

## Check yourself

1. Which versions does `tonic = "0.14"` accept? And `tokio = "1.46.1"`?
2. You add `pub unit: String` to a struct whose fields are all public. What bump is needed, and why?
3. Why must features be additive?
4. What does `"serde?/std"` do that `"serde/std"` doesn't?
5. Why does the repository's root `Cargo.toml` spell out `resolver = "2"`?
6. What's the difference between `to_sorted_lines(&self)` and `into_sorted_pairs(self)`, as the names promise?
7. `cargo tree -d` lists `thiserror` twice. What does that tell you, and is it a bug?
8. What is an MSRV, and where do you declare it?

<details><summary>Answers</summary>

1. `tonic = "0.14"`: `>=0.14.0, <0.15.0`. `tokio = "1.46.1"`: `>=1.46.1, <2.0.0`.
2. Major: struct literals like `Datapoint { ts, value }` and exhaustive
   destructuring stop compiling, because they no longer name every field.
3. Features are unified: every crate in the build gets the union. If turning
   a feature on changed or removed behaviour, one crate's choice would break
   another crate that never asked for it.
4. The weak form only turns on `std` if `serde` is enabled some other way;
   `"serde/std"` also enables the optional `serde` dependency itself.
5. It's a virtual workspace (no `[package]`, so no edition to infer the
   resolver from). Resolver 2 stops dev/build-dependency features leaking
   into normal builds.
6. `to_` borrows `self` and builds something new (it costs an allocation, and
   `self` stays usable). `into_` consumes `self`, so it can move data out
   without cloning.
7. Two dependencies require incompatible major versions (1.x and 2.x), so
   both are compiled. It's not a bug, but it costs build time and binary
   size; updating the older dependent would remove it.
8. The oldest Rust compiler that can build the crate. Declare it with
   `rust-version = "1.xx"` in `[package]`.
</details>
