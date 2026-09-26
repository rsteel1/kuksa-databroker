# 09 — Traits & generics

**Why this matters for the databroker.** Traits are how the databroker is
put together. The broker core doesn't know anything about gRPC: it talks to
providers through the `ActuationProvider` and `SignalProvider` traits, and
the gRPC layer supplies an implementation. The gRPC API itself is a trait
(`val_server::Val`) generated from the `.proto` file, which `DataBroker`
implements. Conversions between wire types and broker types are `From`
impls. Once you can read traits, generics and `dyn`, the architecture of the
codebase becomes visible.

## Traits: shared behaviour

A trait is a set of method signatures a type promises to provide:

```rust
pub trait ActuationProvider {
    fn actuate(&self, changes: Vec<ActuationChange>) -> Result<(), (ActuationError, String)>;
    fn is_available(&self) -> bool;
}

impl ActuationProvider for GrpcProvider {
    fn actuate(&self, changes: Vec<ActuationChange>) -> Result<(), (ActuationError, String)> { ... }
    fn is_available(&self) -> bool { !self.sender.is_closed() }
}
```

(The real one, `databroker/src/broker.rs:189`, is `async` — chapter 23 explains that.)

- Traits are **nominal**: implementing the methods isn't enough, you must
  write `impl Trait for Type`.
- The **orphan rule**: you may implement a trait for a type only if the trait
  *or* the type is defined in your crate. You can `impl Display for MyType`
  and `impl MyTrait for Vec<i32>`, but not `impl Display for Vec<i32>`.
  This is why `conversions.rs` can write `impl From<proto::Value> for broker::DataValue`
  — `DataValue` is local to the databroker crate.
- To call a trait method, the trait must be **in scope** (`use std::io::Write;`).
  That's why you sometimes see `use std::error::Error as _;`: import the trait
  for its methods without taking the name.

## Default methods

Trait methods may have bodies. Implementors inherit them and may override:

```rust
trait Provider {
    fn name(&self) -> String;                    // required
    fn is_available(&self) -> bool { true }      // default
    fn status_line(&self) -> String {            // default using the others
        format!("{}: {}", self.name(), if self.is_available() { "up" } else { "gone" })
    }
}
```

Defaults can only use what the trait itself provides (other trait methods,
and bounds on `Self`) — they don't know the concrete type's fields.

## Implementing standard traits

Many "language features" are traits you implement:

| Trait | Gives you | Derivable? |
|---|---|---|
| `Debug` | `{:?}` | yes |
| `Display` | `{}`, `.to_string()` | no — write it |
| `Clone` / `Copy` | `.clone()` / implicit copies | yes |
| `PartialEq` / `Eq` | `==` | yes |
| `PartialOrd` / `Ord` | `<`, `>`, sorting | yes |
| `Hash` | use as HashMap/HashSet key | yes |
| `Default` | `T::default()`, `..Default::default()` | yes |
| `From<T>` / `TryFrom<T>` | conversions, `.into()`, `?` error conversion | no |
| `Iterator` | `for` loops, adaptors | no |
| `Drop` | cleanup code | no |

`#[derive(...)]` generates the obvious implementation field by field.
`Display` isn't derivable because only you know how a value should look to
a human; `databroker/src/types.rs:44` and `:112` write it for `DataType` and
`DataValue`.

## Conversions: From, Into, TryFrom

- `impl From<A> for B` — infallible conversion A → B. You get `Into` for free:
  `let b: B = a.into();`. **Always implement `From`, never `Into` directly.**
- `impl TryFrom<A> for B { type Error = E; fn try_from(a: A) -> Result<B, E> }` —
  fallible conversion; `TryInto` comes for free.
- `From` also powers `?` error conversion (chapter 07).
- `TryFrom`/`TryInto` are in the 2021 prelude; no `use` needed.

The gRPC layer is a wall of `From` impls:
`databroker/src/grpc/kuksa_val_v2/conversions.rs:394` (`proto::Value → DataValue`),
`:436` (`DataValue → proto::Value`), `:24`, `:48`, `:216`, `:314`, ...
And `databroker/src/authorization/jwt/decoder.rs:134` has
`impl TryFrom<Claims> for Permissions` — a JWT's claims may be invalid.

**`From` must not fail.** Look at `conversions.rs:431`:

```rust
impl From<proto::Value> for broker::DataValue {
    fn from(value: proto::Value) -> Self {
        match &value.typed_value {
            ...
            None => todo!(),   // <- panics!
        }
    }
}
```

A client can send an `ActuateRequest` whose `Value` has no `typed_value`
(the field is optional on the wire). That reaches `DataValue::from(value)`
at `databroker/src/grpc/kuksa_val_v2/val.rs:677` and panics the request
handler. The type signature lied: this conversion *can* fail, so it should be
`TryFrom` returning an INVALID_ARGUMENT error. (A good first contribution!)
Exercise `c09_e04` makes you write the honest version.

## Generics and trait bounds

```rust
fn max_of<T: PartialOrd + Copy>(values: &[T]) -> Option<T> { ... }

impl<T> Range<T>
where
    T: PartialOrd,
{
    fn contains(&self, value: &T) -> bool { ... }
}
```

- Inside a generic function you can only use what the bounds promise. A bare
  `T` can't be compared, printed, cloned or even copied out of a slice.
  E0369 ("binary operation `>` cannot be applied to type `T`") and E0277
  ("the trait bound `T: X` is not satisfied") mean "add a bound".
- `where` clauses are the same bounds in a more readable place.
- Bounds can be put on an `impl` block: methods then only exist for `T`s
  that satisfy them.
- **Monomorphization**: the compiler generates a separate copy of generic
  code for every concrete type used. It's as fast as hand-written code
  ("zero-cost abstraction"), at the price of compile time and binary size.

`types.rs` implements `greater_than`, `less_than`, ... over every pair of
numeric `DataValue` variants by hand (`databroker/src/types.rs:137` onwards,
~350 lines). Because `DataValue` is an *enum* (one type, many runtime
shapes), generics don't directly remove that match — you'd still need to
dispatch on the variant — but a generic helper per arm could shrink it.

## impl Trait: flexible arguments, opaque returns

In **argument** position `impl Trait` is shorthand for a generic:

```rust
pub fn new(version: impl Into<String>, commit_sha: impl Into<String>) -> Self   // broker.rs:2338
pub fn get_entry_by_path(&self, path: impl AsRef<str>) -> Result<&Entry, ReadError> // broker.rs:1148
pub async fn update_entries(&self, updates: impl IntoIterator<Item = (i32, EntryUpdate)>) // broker.rs:1529
```

- `impl Into<String>`: accepts `&str` and `String`; converts once, inside.
  (If you're given a `String` it's moved, no copy.)
- `impl AsRef<str>`: accepts anything you can view as `&str` — read-only.
- `impl IntoIterator<Item = X>`: accepts a Vec, array, HashMap, or lazy iterator.

In **return** position it means "some concrete type implementing the trait,
which I won't name":

```rust
fn paths(&self) -> impl Iterator<Item = &str> + '_ { ... }
pub async fn subscribe(...) -> Result<impl Stream<Item = Option<EntryUpdates>>, SubscriptionError> // broker.rs:1606
```

Iterator and stream adaptor types are often unnameable (they contain closures),
so this is the only practical way to return them without boxing. The caller
sees only the trait. (Edition 2024 changes which lifetimes such a return type
captures by default; in 2021 you write `+ '_` to say it borrows from `self`.)

## Static vs dynamic dispatch

Two ways to write "works with any provider":

```rust
// static dispatch: one compiled copy per concrete P
fn call<P: ActuationProvider>(p: &P) { p.is_available(); }

// dynamic dispatch: one compiled copy; the method is looked up at runtime
fn call(p: &dyn ActuationProvider) { p.is_available(); }
```

A `dyn Trait` value is **unsized** — the compiler doesn't know which concrete
type is behind it — so it always lives behind a pointer: `&dyn T`,
`Box<dyn T>`, `Arc<dyn T>`. That pointer is *fat*: (data pointer, vtable
pointer). The vtable is a per-type table of function pointers.

| | Generics (`T: Trait`) | Trait objects (`dyn Trait`) |
|---|---|---|
| Resolved | at compile time | at run time (vtable) |
| Cost per call | direct call, inlinable | indirect call, not inlined |
| Collections | `Vec<T>` holds ONE type | `Vec<Box<dyn T>>` mixes types |
| Code size | one copy per type | one copy |
| Needs | nothing special | trait must be dyn compatible |

The databroker needs **mixing**: the list of signal providers holds the gRPC
`Provider` in production and `HangingSignalProvider`, `CountingSignalProvider`,
... in tests, all in the same `HashMap`. So it stores trait objects:

```rust
// broker.rs:218 and :235
actuation_provider: Arc<dyn ActuationProvider + Send + Sync + 'static>,
signal_provider: Arc<dyn SignalProvider>,
```

`+ Send + Sync + 'static` are extra bounds on the hidden type: it can move
between threads, be shared between threads, and holds no short-lived
borrows (chapters 13 and 16). Registration takes a `Box<dyn ...>` (`broker.rs:2070`)
and converts it with `Arc::from(...)` (`:2100`) so many tasks can share it.

In edition 2021, writing a trait as a type without `dyn` (`Box<Provider>`) is
error **E0782**.

## Object safety: which traits can be dyn

A trait can only be used as `dyn Trait` if every method can be put in a
vtable — the trait must be **dyn compatible** (formerly "object safe").
Roughly, a method breaks this if it:

- has **generic type parameters** (`fn f<S: Into<String>>(&self, s: S)`) —
  there'd be infinitely many versions, which one goes in the vtable?
- returns `Self` or takes `self` by value without `Self: Sized`,
- is an `async fn` or returns `impl Trait` (the future/return type is a
  different, unnameable type for each implementation).

Violations give **E0038**. The escape hatch: add `where Self: Sized` to that
method. It's then left out of the vtable — still callable on concrete types,
just not through `dyn`.

**Preview — why `#[async_trait]`.** Rust supports `async fn` in traits
natively (since 1.75), but such traits are *not* dyn compatible: each
implementation's `async fn` returns its own anonymous future type. The
databroker needs `Arc<dyn SignalProvider>`, so it uses the `async-trait`
crate (`#[async_trait::async_trait]` on `broker.rs:188`, `:197`), which
rewrites each `async fn` to return `Pin<Box<dyn Future + Send + '_>>` — one
concrete, boxed type the vtable can hold. Chapter 23 digs in.

## Mental model

| Concept | Analogy |
|---|---|
| trait | a job description ("must be able to actuate, must report availability") |
| `impl Trait for Type` | a signed contract: this type fulfils that job |
| default method | a clause the contract fills in unless you write your own |
| generic `T: Trait` | a form letter printed separately for each recipient (fast, many copies) |
| `dyn Trait` | a switchboard: one letter, the operator (vtable) routes each call |
| fat pointer | a phone number *plus* the switchboard's extension list |
| `where Self: Sized` | "this clause doesn't apply when you're reached through the switchboard" |
| `From` | a lossless translation; if translation can fail, that's `TryFrom` |

**Guarantee:** a generic function can only do what its bounds allow, so
every call site is checked at compile time — no "method not found" at runtime,
even through `dyn`.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Having the right methods implements the trait" | You must write `impl Trait for Type` | E0277 |
| "`T` supports `>`, `+`, `clone` by default" | Only what bounds promise | E0369 / E0599 / E0277 |
| "`impl Trait` in return position is dynamic dispatch" | It's one concrete (hidden) type, statically dispatched | — |
| "Any trait can be `dyn`" | Generic methods, `-> Self`, `async fn` break dyn compatibility | E0038 |
| "`Box<Trait>` is fine" | 2021 requires `Box<dyn Trait>` | E0782 |
| "I should implement `Into`" | Implement `From`; `Into` is automatic | — |
| "`dyn` is slow, avoid it" | One indirect call; negligible next to I/O. Use it when you need heterogeneity | — |

## Smells and idioms

- **`From` that panics** (see `conversions.rs:431`). Fallible → `TryFrom`.
- **Taking `String` when you only read** → `&str` or `impl AsRef<str>`.
  **Taking `&str` then immediately `.to_string()`** → `impl Into<String>`.
- **Giant traits.** Prefer small traits with one responsibility.
- **Generic everything.** Generics add compile time and complexity; if you
  need heterogeneous collections or plugin-style extension, `dyn` is simpler.
- **Repeated identical impls in tests.** Each test double in `broker.rs`
  writes `fn is_available(&self) -> bool { true }` (e.g. `broker.rs:5025`,
  `:5051`, `:5078`). A default method on the trait would remove that noise —
  but would also make it easy for a *real* provider to forget to implement
  it. Trade-offs!
- **`get_` prefixes** (`get_version()`, `get_commit_sha()`) are un-idiomatic
  for getters in Rust API guidelines (`version()`); they're harmless but you'll
  see reviewers mention it.

## In the databroker

- `databroker/src/broker.rs:188`–`208` — the two provider traits. Note the
  supertrait bounds on `SignalProvider: Send + Sync + 'static` versus bounds
  added at the use site for `ActuationProvider` (`:218`).
- `databroker/src/grpc/kuksa_val_v2/val.rs:66`–`107` — `impl ActuationProvider for Provider`;
  `:109`–`248` — `impl SignalProvider for Provider`; `:250` —
  `impl proto::val_server::Val for broker::DataBroker` (the generated gRPC trait).
- `databroker/src/broker.rs:5012` onwards — test doubles implementing the
  same traits (hanging, counting, failing providers).
- `databroker/src/types.rs:44`, `:112` — `Display` impls.
- `databroker/src/grpc/kuksa_val_v2/conversions.rs` — `From` impls both ways;
  `:431` the panicking arm.
- `databroker/src/authorization/jwt/decoder.rs:134` — `TryFrom<Claims> for Permissions`.
- `databroker/src/broker.rs:2338`, `:1148`, `:1529` — `impl Into<String>`,
  `impl AsRef<str>`, `impl IntoIterator`.
- `databroker/src/broker.rs:1606` — `-> Result<impl Stream<...>, SubscriptionError>`.
- `databroker/src/broker.rs:2517` — `impl Default for DataBroker` (delegates to `new`).

## Exercises

| Exercise | Goal |
|---|---|
| `c09_e01_define_and_impl_trait` | implement a trait for two types |
| `c09_e02_default_methods` | default bodies; overriding |
| `c09_e03_display_impl` | `Display` for `DataType`/`DataValue` |
| `c09_e04_from_into_tryfrom` | wire ⇄ broker conversions, honest `TryFrom` |
| `c09_e05_generic_bounds_and_where` | bounds, `where`, bounds on impl blocks |
| `c09_e06_impl_trait_args` | `impl Into`/`AsRef`/`IntoIterator` args, `-> impl Iterator` |
| `c09_e07_trait_objects_dyn` | `dyn`, `Arc<dyn>`, E0038 and `where Self: Sized` |

## Further reading

- The Rust Book, ch. 10 "Generic Types, Traits, and Lifetimes" (10.1, 10.2) — https://doc.rust-lang.org/book/ch10-00-generics.html
- The Rust Book, ch. 18.2 "Using Trait Objects That Allow for Values of Different Types" — https://doc.rust-lang.org/book/ch18-02-trait-objects.html
- The Rust Reference, "Dyn compatibility" — https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
- Rust API Guidelines, "Naming" (C-GETTER) and "Interoperability" (C-CONV-TRAITS) — https://rust-lang.github.io/api-guidelines/
- `async-trait` README ("dyn traits") — https://docs.rs/async-trait

## Check yourself

1. Why does the broker store `Arc<dyn SignalProvider>` rather than being
   generic over `P: SignalProvider`?
2. What's inside a `&dyn Trait` that isn't inside a `&T`?
3. Name two things that make a trait not dyn compatible, and the usual fix.
4. Why is `impl From<proto::Value> for DataValue` with `None => todo!()` a problem?
5. What does `fn new(version: impl Into<String>)` let callers do that
   `fn new(version: String)` doesn't?
6. Why can't async traits be used with `dyn` without help, and what does
   `#[async_trait]` do about it?

<details><summary>Answers</summary>

1. It needs to hold *different* provider types in one collection (real gRPC
   providers and test doubles, possibly several kinds at once), and generic
   code would make `DataBroker` itself generic over the provider type.
2. A pointer to the vtable (method table) of the concrete type — it's a fat pointer.
3. Generic methods; methods returning `Self` / taking `self` by value; `async fn`.
   Fix: `where Self: Sized` on the offending method (or redesign / `#[async_trait]`).
4. `From` promises infallibility; a client can send an empty `Value`, which
   panics the handler. It should be `TryFrom` returning an error status.
5. Pass `&str` directly (`DataBroker::new("0.7", "abc")`) as well as `String`.
6. Each impl's `async fn` returns a different anonymous future type, which
   can't go in a vtable. `#[async_trait]` rewrites them to return
   `Pin<Box<dyn Future + Send>>`, a single type.
</details>

## Where this goes next

**Chapter 14 (Advanced traits)** continues from here: associated types, blanket impls, the orphan rule, operator overloading, `Deref`/`Borrow`, `Any`, sealed traits and typestate. Chapter 23 explains why the databroker's async provider traits need `#[async_trait]` to be used as `dyn`.
