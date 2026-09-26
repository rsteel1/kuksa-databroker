# 14 — Advanced traits

**Why this matters for the databroker.** Chapter 09 taught you to define and
implement traits. Real codebases lean on the *advanced* features:
associated types (every `Iterator`, every `FromStr`), supertraits
(`SignalProvider: Send + Sync + 'static`), blanket impls (why every
`Display` type has `.to_string()`), the orphan rule (why the databroker's
conversions live where they do), and `Deref`/`AsRef`/`Borrow` (why a
`HashMap<String, i32>` can be queried with a `&str`). Most library
signatures you'll read (tower, tonic, serde, tokio) are built from exactly
these pieces. This chapter lets you read those signatures instead of
guessing.

## Associated types vs generic parameters

Both let a trait talk about "some other type". The difference is **how many
implementations a type can have**:

```rust
trait Iterator {            // associated type: ONE Item per iterator type
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}

trait From<T> {             // generic parameter: MANY impls per type
    fn from(value: T) -> Self;
}
// String: From<&str>, From<char>, From<Box<str>>, ... all at once
```

The rule of thumb is to ask: *for one implementing type, can there be more
than one answer?*

- A `Vec<u8>` iterator yields `u8`, and only `u8`. So it's an **associated
  type**. Callers write `I::Item` and the compiler knows exactly what that is,
  with no extra annotations.
- A `Reading` can be built from an `f32` *and* from a `bool`. So it's a
  **generic parameter**, and each `T` gets its own impl.

Associated types also keep signatures readable:
`fn drain<S: SignalSource>(s: &mut S) -> Vec<S::Value>` needs no second type
parameter.

Associated **constants** work the same way (`const NAME: &'static str;`)
and are resolved at compile time. You'll use them in `c14_e07`.

## Supertraits and blanket impls

**Supertrait:** `trait Provider: Named { ... }` means "to implement
`Provider`, a type must also implement `Named`". Inside `Provider`'s
default methods you may call `self.name()`. A `dyn Provider` can call
`Named` methods too.

The databroker's most important trait uses supertraits as *requirements*:

```rust
pub trait SignalProvider: Send + Sync + 'static { ... }   // broker.rs:198
```

Any provider must be thread-safe (`Send + Sync`) and own its data
(`'static`), because the broker stores it in an `Arc` and calls it from
whichever task handles a request.

**Blanket impl:** implement a trait for *every* type meeting a bound:

```rust
impl<T: Display> Describe for T {
    fn describe_line(&self) -> String { /* uses self.to_string() */ }
}
```

std's own `impl<T: fmt::Display + ?Sized> ToString for T` is why
`42.to_string()` works. Blanket impls are powerful but *exclusive*: once
`impl<T: Display> Describe for T` exists, you can't also write
`impl Describe for MyType` if `MyType: Display`, because the two would
overlap (E0119, conflicting implementations).

## The orphan rule and the newtype pattern

You may write `impl Trait for Type` only if **the trait or the type is
defined in your crate** ("coherence"). Otherwise two unrelated crates could
both write `impl Display for Vec<Reading>`, and the program would have two
answers for the same question. So `impl Display for Vec<Reading>` in your
crate gives E0117.

**The fix is a newtype.** `struct Readings(Vec<Reading>);` is *your* type,
so any trait can be implemented for it. It has zero runtime cost: same
layout, same speed. Add `From<Vec<Reading>>` so conversions are one
`.into()` away.

**In the databroker:** `grpc/kuksa_val_v2/conversions.rs` is full of
`impl From<&proto::Datapoint> for broker::Datapoint`. That's legal because
`broker::Datapoint` is local to the `databroker` crate, even though the
proto type comes from `databroker-proto`. A conversion between *two* proto
types would have to live in `databroker-proto` itself, or behind a newtype.

## Operator overloading

Every operator is a trait in `std::ops` or `std::cmp`:

| Syntax | Trait | Method |
|---|---|---|
| `a + b` | `Add<Rhs = Self>` | `add(self, rhs) -> Self::Output` |
| `a += b` | `AddAssign` | `add_assign(&mut self, rhs)` |
| `-a` | `Neg` | `neg(self)` |
| `a * 2.0` | `Mul<f64>` | `mul(self, rhs: f64)` |
| `t["x"]` (read) | `Index<&str>` | `index(&self, idx) -> &Output` |
| `t["x"] = v` | `IndexMut<&str>` | `index_mut(&mut self, idx) -> &mut Output` |
| `==` | `PartialEq` / `Eq` | `eq` |
| `<` `>` `sort()` | `PartialOrd` / `Ord` | `partial_cmp` / `cmp` |

Guidelines:

- Overload only when the meaning is **obvious** (adding speeds, indexing a
  table). `+` that sends a network packet is a trap.
- `Index` returns a *reference* and must panic on a missing key (like
  `HashMap`'s `map["x"]`). Offer a `get` returning `Option` alongside it.
- **`Ord` and `PartialOrd` must agree.** Implement `Ord::cmp`, then write
  `partial_cmp` as `Some(self.cmp(other))`. Clippy flags the alternative
  (`non_canonical_partial_ord_impl`). `sort`, `max`, `BTreeMap` and binary
  search all assume the ordering is consistent.
- `PartialOrd` exists for types where some values aren't comparable
  (`f64::NAN`). `partial_cmp` returns `None` there.

**Design note from the databroker:** `DataValue` compares with methods
(`greater_than`, `less_than`, `types.rs:137`) returning
`Result<bool, CastError>`, not `PartialOrd`. Comparing a `String` with an
`Int32` is an *error* to report, not just "not comparable". A
`partial_cmp` returning `None` could express "not comparable", but it can't
carry a reason. Both choices are defensible, and seeing why is the point.

## Deref, AsRef and Borrow

Three traits that all mean "view me as a `&T`", with different contracts:

| Trait | Used by | Promise |
|---|---|---|
| `Deref<Target = T>` | the compiler: `*x`, auto-deref for method calls, deref *coercion* `&VssPath → &str` | "I am a smart pointer or wrapper around a `T`" |
| `AsRef<T>` | generic APIs: `fn f(p: impl AsRef<str>)` | "a cheap reference conversion exists" |
| `Borrow<T>` | `HashMap`, `HashSet`, `BTreeMap` lookups | "`Hash`, `Eq` and `Ord` on the borrowed form give **the same answers** as on me" |

Why does `HashMap<String, i32>::get("x")` work with a `&str` and no
allocation? The signature is:

```rust
pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
where K: Borrow<Q>, Q: Hash + Eq
```

With `K = String` and `Q = str` it needs `String: Borrow<str>`, which std
provides. The *promise* matters: the map hashed the `String` when
inserting and now hashes the `&str` when looking up, so the two hashes must
match. `AsRef` makes no such promise, which is why maps use `Borrow`.

**When should you implement `Deref`?** When your type *is* a wrapper that
should behave like its inside: `Box`, `Arc`, `String → str`, a validated
newtype like `VssPath`. **Don't** use `Deref` to fake inheritance
("`Car` derefs to `Vehicle` so it gets its methods"). That's a known
anti-pattern: methods appear from nowhere, and trait impls don't follow.

## Any and downcasting

`std::any::Any` is implemented for every `'static` type. A `&dyn Any` has
forgotten its concrete type but remembers a `TypeId`, so you can ask:

```rust
if let Some(n) = value.downcast_ref::<i32>() { /* n: &i32 */ }
```

Use it for **type-erased storage**: plugin registries, request "extension"
maps, test doubles. `tonic::Request::extensions()` (how the databroker
passes `Permissions` from the interceptor to the handler, chapter 30) is a
map keyed by `TypeId` built on `Any`.

**Trait upcasting** (stable since Rust 1.86): if `trait Plugin: Any`, a
`&dyn Plugin` coerces to `&dyn Any`, so you can downcast plugins
directly. Older code adds `fn as_any(&self) -> &dyn Any` to every trait for
the same effect. You'll meet both.

Pitfall: downcasting `&Box<dyn Plugin>` asks "is this a *Box*?". Get to the
`dyn Plugin` first (`plugin.as_ref()` or `&**plugin`).

Reach for enums or traits before `Any`. Downcasting is a runtime check the
compiler can't help with.

**A small databroker finding:** `databroker-proto/src/lib.rs:136` prints
`dt.type_id()` in an error message. `type_id` there is `Any::type_id`
(`Any` is imported on line 26), which prints an opaque `TypeId(0x…)`, not
the `DataType` variant name the message seems to intend (`{dt:?}` would
be). Knowing what `Any` really gives you is how you spot it.

## Marker traits, auto traits and sealed traits

**Marker traits** have no methods. They *label* a type so bounds can
require the label: `Copy`, `Eq`, your own `trait Validated {}`.

**Auto traits** are markers the compiler implements *for you*, based on a
type's fields. The important ones are `Send` ("can be moved to another
thread") and `Sync` ("`&T` can be shared between threads"). A struct is
`Send` if all its fields are. One `Rc` inside makes the whole thing
non-`Send`, and the error appears where you *require* `Send` (a thread spawn,
a `SignalProvider` bound), which can be far from the `Rc`. `Sized` and
`Unpin` are auto traits too (chapters 15 and 24).

**Sealed traits** are public traits that outside crates can *use* but not
*implement*:

```rust
mod private { pub trait Sealed {} }          // not reachable from outside
pub trait SignalKind: private::Sealed { ... }
impl private::Sealed for Sensor {}
impl SignalKind for Sensor { ... }
```

Why? It lets a library add methods to the trait later without breaking
anyone, since no external impls can exist to break. tokio seals
`ToSocketAddrs` this way, and std seals many traits internally. When a
library trait "can't be implemented", look for a private supertrait.

## Object-oriented patterns: the state pattern three ways

Rust has no inheritance. The Book's chapter on OOP rebuilds the classic
*state pattern* (a blog post moves Draft → PendingReview → Published) to
show what replaces it. You now know all three Rust answers, and real
codebases use each:

1. **Trait objects**, closest to classic OOP. `state: Box<dyn State>`, where
   each state struct implements `fn request_review(self: Box<Self>) -> Box<dyn State>`.
   It's open for extension (new states in other crates) and costs dynamic
   dispatch. Invalid transitions are still possible at runtime; they just do nothing.
2. **An enum plus `match`**, the most common in practice. `enum PostState { Draft,
   Pending, Published }` with methods that `match` on `self`. The set of
   states is closed, the compiler checks that every transition is handled,
   and adding a state means fixing every `match`. That's a feature: nothing
   gets forgotten. The databroker models `EntryType`, `ChangeType` and
   `ReadError` this way.
3. **Typestate** (the section below): `Post<Draft>` has `request_review()`
   that returns `Post<Pending>`, and only `Post<Published>` has `content()`.
   Invalid transitions **don't compile**. Use this when misuse is costly
   and the state is known statically. Builders are the usual example.

Heuristic: choose an enum when you own every state, typestate when misuse
must be impossible at compile time, and trait objects when others must add
states. "Composition over inheritance" in Rust means holding values that
implement traits (`Arc<dyn SignalProvider>` inside a subscription), not
deriving from a base class.

## Typestate and PhantomData

**Typestate** encodes *which steps have happened* in the type, so misuse is
a **compile** error instead of a runtime check:

```rust
struct SubscribeBuilder<S> { signals: Vec<String>, /* ... */ state: PhantomData<S> }
struct NoSignals; struct HasSignals;          // labels only

impl SubscribeBuilder<NoSignals> { fn new() -> Self { ... } }
impl<S> SubscribeBuilder<S> { fn signal(self, p: &str) -> SubscribeBuilder<HasSignals> { ... } }
impl SubscribeBuilder<HasSignals> { fn build(self) -> SubscribeRequest { ... } }

SubscribeBuilder::new().build();        // error[E0599]: no method named `build`
```

The real broker checks "empty subscription" at runtime
(`SubscriptionError::InvalidInput`, `broker.rs:1608`). With typestate that
error variant can't happen, because the type makes it impossible to ask.

**`PhantomData<S>`** is a zero-sized field that tells the compiler "this
struct logically involves an `S`". It's required because every type
parameter must be used (E0392). It also affects variance and auto traits,
which chapter 16 covers. Cost at runtime: nothing
(`size_of::<PhantomData<T>>() == 0`).

Use typestate when a mistake would be expensive and the states are few.
With many states it becomes hard to read, and a runtime check with a clear
error is often the better trade-off. The databroker's `PermissionBuilder`
(`permissions.rs:47`) is a plain consuming builder whose `build()` returns a
`Result` (`permissions.rs:151`). That fits, because its validation (glob
syntax) can only happen at runtime anyway.

## Mental model

| Idea | Analogy |
|---|---|
| Associated type | A vending machine sells exactly one product line; you ask "what does *this* machine sell?" |
| Generic trait param | A universal charger with a separate adapter for each plug type |
| Supertrait | "To be a licensed electrician you must first be a registered tradesperson" |
| Blanket impl | "Every licensed driver may also rent a car": one rule covers everyone who qualifies |
| Orphan rule | You may only sign contracts where you're one of the parties |
| Newtype | Putting a foreign product in your own branded box so you may sell it |
| `Borrow` | A promise that the photocopy is identical for filing purposes |
| `Any` | A sealed parcel with a type label you can check before opening |
| Sealed trait | A club with a secret membership card: anyone may see the club, only insiders can join |
| Typestate | A form whose "Submit" button only appears once the required fields are filled |

**What the compiler guarantees:** trait resolution is unambiguous
(coherence: at most one impl applies), auto traits are exactly as strong as
the fields allow (no data races through a forgotten `Rc`), and typestate
misuse is rejected at compile time with zero runtime cost.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Associated types and generics are interchangeable." | Associated type = one impl per type; generic = many. Choose by that. |
| "I can implement any trait for any type." | Orphan rule (E0117). One side must be local; use a newtype. |
| "A supertrait is inheritance." | It's a *requirement*, not data or method inheritance. The subtype must implement both. |
| "`Deref` is how you do inheritance in Rust." | That's an anti-pattern. Use composition and traits. |
| "`AsRef` and `Borrow` are the same." | `Borrow` also promises identical `Hash`/`Eq`/`Ord`, which maps rely on. |
| "`Send`/`Sync` are implemented by writing `impl Send`." | They're auto traits, derived from fields. Manual impls are `unsafe` (chapter 26). |
| "`PhantomData` stores a value." | It's zero-sized. It only informs type checking. |
| "`downcast_ref` on a `Box<dyn Trait>` checks the inner type." | It checks the Box. Deref to the trait object first. |

## Smells and idioms

- **Don't overload operators with surprising meaning.** `+` should be
  arithmetic-like, and `Index` should be a cheap lookup that panics only on
  a missing key.
- **Implement `Ord`, derive `PartialOrd` from it.** Never hand-write two
  comparison functions that can drift apart.
- **Blanket impls in libraries are forever.** Adding one later is a breaking
  change, because it can conflict with users' impls. Think before you add one.
- **Prefer enums or traits over `Any`.** Use `Any` only for genuinely open,
  type-erased registries.
- **Seal traits you might extend** in a public library, and document the
  seal.
- **Typestate for few, critical states.** For many states or data-dependent
  validation, prefer a runtime check with a precise error type.
- **The databroker's choices, honestly:** `SignalProvider` uses supertraits
  exactly as intended. `DataValue`'s method-based comparisons trade
  `sort()` compatibility for error reporting. The `dt.type_id()` message in
  `databroker-proto` is a small bug of the "didn't know what `Any` gives
  you" kind. Its `lazy_static` usage (chapter 12) and nine-argument
  `add_entry` are places a builder or typestate could help.

## In the databroker and in the wild

Databroker:

- `databroker/src/broker.rs:198`, `pub trait SignalProvider: Send + Sync + 'static`:
  supertraits as thread-safety requirements.
- `databroker/src/broker.rs:1119-1120`,
  `impl<'a> Iterator for EntryReadIterator<'a, '_> { type Item = EntryReadAccess<'a>; ... }`:
  an associated type in a real impl.
- `databroker-proto/src/lib.rs:49-50`, `impl FromStr for DataType { type Err = ParsingError; ... }`.
- `databroker/src/grpc/kuksa_val_v2/conversions.rs:24`, `:394` and `:436`:
  `From` implemented many times across the same pair of domains, which the
  orphan rule allows because one side is always local.
- `databroker/src/broker.rs:130`, `path_to_id: HashMap<String, i32>`,
  queried with `&str` thanks to `String: Borrow<str>`. At `:1148`,
  `get_entry_by_path(&self, path: impl AsRef<str>)`.
- `databroker/src/types.rs:137`, `DataValue::greater_than`, the
  method-instead-of-`PartialOrd` design.
- `databroker-proto/src/lib.rs:26` (imports `Any`) and `:136`
  (`dt.type_id()` in an error message).
- `databroker/src/broker.rs:1608`, the runtime `SubscriptionError::InvalidInput`
  that typestate would make impossible. `databroker/src/permissions.rs:47`
  and `:151` show the consuming `PermissionBuilder`.

In the wild. These are paths on disk. std sources come from the
`rust-src` component under
`~/.rustup/toolchains/stable-<target>/lib/rustlib/src/rust/library/`, and
crate sources are under `~/.cargo/registry/src/index.crates.io-*/`. The line
numbers are for Rust 1.96 and the crate versions in `Cargo.lock`; if they've
moved, search for the item name.

- `core/src/iter/traits/iterator.rs:42` defines `trait Iterator`, with
  `type Item;` at `:46`. (In std's own source you'll see `pub const trait`,
  which is internal nightly syntax. Read it as `pub trait`.)
- `alloc/src/string.rs:2891`, `impl<T: fmt::Display + ?Sized> ToString for T`:
  the blanket impl behind `.to_string()`.
- `alloc/src/string.rs:2818`, `impl ops::Deref for String` (`Target = str`),
  and `alloc/src/str.rs:189`, `impl Borrow<str> for String`.
- `std/src/collections/hash/map.rs:1030`, `pub fn get<Q: ?Sized>(&self, k: &Q)`,
  whose `K: Borrow<Q>` bound makes `&str` lookups work.
- `core/src/time.rs:1176`, `impl Add for Duration`: real operator overloading.
- `core/src/marker.rs:92` (`Send`), `:657` (`Sync`), `:811` (`PhantomData`).
- `core/src/any.rs:116` (`trait Any: 'static`) and `:228` (`downcast_ref`).
- `tower-service-0.3.3/src/lib.rs:311`, `pub trait Service<Request>` with
  **three associated types** (`Response`, `Error`, `Future`, `:313-319`) and
  a generic `Request`. That's the associated-vs-generic rule applied: one
  service handles many request types, but each has one response type. tonic
  and axum are built on it (chapter 24).
- `anyhow-1.0.104/src/error.rs:674`, `impl<E> From<E> for Error` (with
  bounds): the blanket impl that makes `?` convert any error into
  `anyhow::Error` (chapter 17).
- `serde_core-1.0.229/src/ser/impls.rs:460`, the `deref_impl!` macro, which
  implements `Serialize` for `&T`, `Box<T>` and friends by forwarding to
  `T`, generated by a macro (chapter 18).
- `tokio-1.53.1/src/net/addr.rs:19`,
  `pub trait ToSocketAddrs: sealed::ToSocketAddrsPriv {}`, with the private
  `mod sealed` at `:252`: a real sealed trait.

## Exercises

| Exercise | Goal |
|---|---|
| `c14_e01_associated_types_vs_generics` | A `SignalSource` with `type Value` next to a generic `ConvertFrom<T>` |
| `c14_e02_supertraits_and_blanket_impls` | `Provider: Named`, and one blanket impl for every `Display` type |
| `c14_e03_orphan_rule_newtype` | Hit E0117, fix it with a newtype + `From` |
| `c14_e04_operator_overloading` | `Add`/`Sub`/`Neg`/`Mul`/`AddAssign`, `Index`/`IndexMut`, a reversed `Ord` |
| `c14_e05_deref_asref_borrow` | Make a path newtype behave like `str` and be a zero-allocation map key |
| `c14_e06_any_downcast` | Downcast values and find plugins by concrete type (trait upcasting) |
| `c14_e07_sealed_and_marker_traits` | `Rc` vs `Arc` for auto traits, a sealed trait, a marker bound |
| `c14_e08_typestate_phantomdata` | A subscribe builder where `build()` exists only after a signal is added |

## Further reading

- The Book, ch. 20.2 "Advanced Traits" (associated types, operator
  overloading, supertraits, newtype):
  <https://doc.rust-lang.org/book/ch20-02-advanced-traits.html>
- The Rust Reference, "Trait and lifetime bounds" and "Implementations →
  Orphan rules": <https://doc.rust-lang.org/reference/items/implementations.html>
- Rust API Guidelines, "Future proofing → Sealed traits" (C-SEALED):
  <https://rust-lang.github.io/api-guidelines/future-proofing.html>
- `std::borrow::Borrow` docs (the Hash/Eq contract):
  <https://doc.rust-lang.org/std/borrow/trait.Borrow.html>
- `std::any` module docs: <https://doc.rust-lang.org/std/any/>
- *Rust for Rustaceans*, ch. 2 "Types" and ch. 3 "Designing Interfaces".

## Check yourself

1. Why is `Iterator::Item` an associated type while `From<T>` is generic?
2. What does `trait SignalProvider: Send + Sync + 'static` require of an implementor, and why does the broker need it?
3. Why is `impl Display for Vec<Reading>` rejected, and what's the zero-cost fix?
4. `HashMap<VssPath, i32>` should be queryable with `&str`. Which trait do you implement, and what promise must it keep?
5. When is implementing `Deref` a smell?
6. Why implement `Ord` and derive `partial_cmp` from it rather than the other way round?
7. How does a sealed trait prevent outside implementations, and why would a library want that?
8. What does `PhantomData<S>` cost at runtime, and why is it needed?

<details><summary>Answers</summary>

1. An iterator yields one item type (one answer per implementing type). A
   type can be built from many source types, which needs one impl per `T`.
2. The implementor must be `Send` (movable across threads), `Sync` (shareable
   by reference) and own its data (`'static`). The broker keeps providers in
   `Arc<dyn SignalProvider>` and calls them from arbitrary Tokio tasks.
3. The orphan rule: neither `Display` nor `Vec` is local (E0117). Wrap it in
   a local newtype, `struct Readings(Vec<Reading>)`.
4. `Borrow<str>`. `Hash`, `Eq` (and `Ord`) on the `&str` must give the same
   results as on the `VssPath`, which deriving from the inner `String`
   ensures.
5. When it's used to fake inheritance, making one type pretend to be
   another unrelated type to borrow its methods.
6. `Ord` is the total, authoritative ordering. Defining `partial_cmp` as
   `Some(self.cmp(other))` guarantees they agree, which sorting and ordered
   collections assume.
7. The trait has a supertrait in a private module that outsiders can't name,
   so they can't implement it. The library can then add methods without a
   breaking change.
8. Nothing (it's zero-sized). Every type parameter must be used by some
   field (E0392), and `PhantomData` also tells the compiler about variance
   and auto traits.
</details>
