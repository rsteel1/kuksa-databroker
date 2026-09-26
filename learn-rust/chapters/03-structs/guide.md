# 03 — Structs, methods and newtypes

**Why this matters for the databroker.** The databroker's data model is a
handful of structs: `Metadata` (what a signal *is*), `Datapoint` (a value
plus timestamps), `Entry` (metadata + current value + actuator target),
`EntryUpdate` (a partial change), `Database`, `DataBroker`. Reading
`broker.rs` means reading struct definitions and their `impl` blocks. This
chapter teaches you to read them — and to design your own so that invalid
data can't even be constructed.

---

## Defining and building structs

```rust
struct Datapoint {
    path: String,
    value: i64,
    timestamp_ms: u64,
}

let dp = Datapoint { path: String::from("Vehicle.Speed"), value: 120, timestamp_ms: 1_000 };
println!("{}", dp.value);
```

- A struct owns its fields. Dropping the struct drops every field.
- Build one by naming **every** field (order doesn't matter).
- Field init shorthand: if a variable has the field's name, `value` means
  `value: value`.
- Mutability belongs to the *binding*, not to individual fields:
  `let mut dp = ...; dp.value = 42;` — all fields become assignable.
- `1_000` — underscores in number literals are just for readability.

The real thing, `broker.rs:106-110`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Datapoint {
    pub ts: SystemTime,
    pub source_ts: Option<SystemTime>,
    pub value: DataValue,
}
```

`pub` makes the struct and each field visible outside its module
(chapter 08). `Option<SystemTime>` means "there may or may not be a source
timestamp" (chapter 04).

Other shapes you'll meet:

```rust
struct SignalId(i32);   // tuple struct: fields by position, `.0`
struct Marker;          // unit struct: no data at all
```

## Methods and the three selves

Behaviour lives in `impl` blocks, separate from the data:

```rust
impl SignalStats {
    fn empty() -> SignalStats { ... }        // associated fn: no self
    fn average(&self) -> f64 { ... }         // reads
    fn record(&mut self, value: i64) { ... } // modifies
    fn into_report(self) -> String { ... }   // consumes
}
```

The first parameter — the *receiver* — is the most informative part of any
method signature. It's the ownership rules of chapter 02 applied to
methods:

| Receiver | Meaning | Caller afterwards |
|---|---|---|
| `&self` | "I only look" | still owns it, others may look too |
| `&mut self` | "I change you in place" | still owns it; needed exclusive access during the call |
| `self` | "I take you over" | can't use it any more (unless `Copy`) |
| (none) | associated function, e.g. a constructor | called as `Type::name(..)` |

Naming conventions reinforce this: `into_*` consumes (`into_report`),
`as_*` borrows cheaply (`as_str`), `to_*` makes a new owned value
(`to_string`). Getters are named after the field — `id()`, **not**
`get_id()`.

Calling: `stats.record(5)` — Rust automatically takes `&mut stats` for you
(auto-ref). `SignalStats::empty()` — associated functions use `::`.

In `broker.rs` look at `impl Entry` (starts at `broker.rs:274`):
`diff(&self, ..)`, `validate(&self, ..)`, `apply(&mut self, ..)`. You can
tell which methods can change an entry without reading their bodies.

## Constructors: new and Default

Rust has no constructor keyword. Conventions:

```rust
impl ServerSettings {
    fn new(address: &str, port: u16) -> Self {
        Self { address: address.to_string(), port, enable_unix_socket: false }
    }
}
```

`Self` inside an `impl` means "the type this impl is for".

If a type has an obvious standard value, implement the `Default` trait:

```rust
impl Default for ServerSettings {
    fn default() -> Self { Self::new("127.0.0.1", 55555) }
}
```

or derive it when "every field at its own default" is right:

```rust
#[derive(Default)]
struct Counters { published: u64, dropped: u64, last_errors: Vec<String> }
// Counters::default() == Counters { published: 0, dropped: 0, last_errors: vec![] }
```

Why bother? `Default` plugs into the rest of the language:
`..Default::default()` (below), `Option::unwrap_or_default()`,
`#[derive(Default)]` on structs that *contain* your type, `mem::take`.

The databroker does both:
- `broker.rs:127-132` — `#[derive(Default)] pub struct Database` (empty maps, id counter at 0).
- `broker.rs:2338-2349` — `DataBroker::new(version, commit_sha)`.
- `broker.rs:2517-2521` — `impl Default for DataBroker { fn default() -> Self { Self::new("", "") } }`
  — the default reuses `new`, so the two can never drift apart.

## Deriving common traits

A new struct can't even be printed or compared. You ask for those
abilities with `#[derive(...)]`, which generates the obvious code:

| Derive | Gives you | Requirement |
|---|---|---|
| `Debug` | `{:?}` formatting; needed by `assert_eq!` | all fields `Debug` |
| `Clone` | `.clone()` | all fields `Clone` |
| `Copy` | implicit copy on assignment | all fields `Copy`; also derive `Clone` |
| `PartialEq` | `==`, `!=` | all fields `PartialEq` |
| `Eq` | promise that `==` is reflexive (x == x always) | all fields `Eq` — **no `f32`/`f64`** (NaN != NaN) |
| `Hash` | usable as `HashMap` key / `HashSet` member | all fields `Hash`; should be consistent with `Eq` |
| `PartialOrd`, `Ord` | `<`, sorting, `BTreeMap` keys | field-by-field comparison |
| `Default` | `Type::default()` | all fields `Default` |

Real examples:
- `broker.rs:120-125` — `#[derive(Debug, PartialEq, Eq, Hash, Clone)] pub enum Field`:
  `Hash + Eq` because the broker keeps `HashSet<Field>` ("which fields of
  this entry changed?").
- `types.rs:490-493` — `SignalId` derives `Copy` (it's an `i32`) and `Hash`
  (it's a map key).
- `types.rs:504-507` — `TimeInterval` also derives `PartialOrd, Ord`, so the
  filter manager can pick the *lowest* interval among subscribers.
- `broker.rs:105` — `Datapoint` is `PartialEq` but not `Eq`: its
  `DataValue` may contain floats.

## Struct update syntax

```rust
let update = EntryUpdate {
    value: Some(80),
    ..Default::default()
};
```

`..expr` fills every field you didn't name from `expr`. With
`Default::default()` that's "everything else at its default" — for
`Option` fields, `None`. It's how the databroker builds partial updates:
`EntryUpdate` (`broker.rs:250-272`) has twelve `Option` fields and derives
`Default`. `None` in an update means "don't touch this field". Chapter 04
explains the `Option<Option<T>>` fields you'll notice in it.

(The expression after `..` must come last and has no trailing comma.)

## Newtypes and validated construction

A **newtype** wraps a single value to give it a distinct type:

```rust
pub struct SignalId(i32);
```

At runtime it *is* an `i32` (zero cost). At compile time it's a different
type, so you can't pass a port number where a signal id is expected, or add
two ids together by accident. The databroker's `SignalId`
(`types.rs:490-502`) uses a named private field (`{ id: i32 }`) and exposes
`new` and `id()`.

The pattern becomes powerful combined with **privacy** and a validating
constructor:

```rust
mod vss {
    pub struct VssPath(String);          // field is private to `mod vss`

    impl VssPath {
        pub fn parse(input: &str) -> Option<VssPath> { /* validate */ }
        pub fn as_str(&self) -> &str { &self.0 }
    }
}
```

Code outside `mod vss` cannot write `VssPath(anything)` — the only way to
get a `VssPath` is through `parse`. Therefore **every `VssPath` that exists
is valid**: functions that take a `VssPath` never need to re-check it.
This is "make invalid states unrepresentable" / "parse, don't validate":
check once at the border of your system, carry the proof in the type.

The databroker validates paths with a function instead
(`glob.rs:202` `is_valid_path(input: &str) -> bool`) and passes plain
`String`s around. That works, but every caller must remember to call it.
It's a good example of where a newtype would make the code more robust —
a nice first-contribution-sized idea to keep in mind.

`Permissions` (`permissions.rs:39-45`) shows the privacy half: all its
fields are private, so the only way to create one is the builder
(`Permissions::builder()...build()`) — chapter 29.

---

## Mental model

| Concept | Analogy |
|---|---|
| struct | a form with named boxes; the form owns what's in the boxes |
| `impl` block | the instruction manual stapled to the form |
| `&self` / `&mut self` / `self` | read the form / fill it in / hand it in |
| `derive` | ticking "yes, the standard behaviour" instead of writing it out |
| `Default` | the blank form |
| `..Default::default()` | "and all other boxes blank" |
| newtype | a coloured folder: same paper inside, but you can't mix it up with other folders |
| private field + validating constructor | a notarised document: if you hold one, it's been checked |

**What the compiler guarantees:** every struct is fully initialised; you
can't build a type whose fields you can't see; a method's receiver tells
you exactly what it may do to the value.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Fields can be individually `mut`" | Mutability is per binding | E0594 |
| "Structs compare/print automatically" | Derive `PartialEq`, `Debug`, … | E0369, E0277 |
| "`Copy` is just a faster `Clone`" | `Copy` changes semantics (no moves) and is only possible for heap-free types | E0204 |
| "`f64` fields can be `Eq`/`Hash`" | NaN breaks equivalence; only `PartialEq` | E0277 |
| "A constructor is special syntax" | It's a plain associated function, by convention `new` | E0599 |
| "Private means per-struct" | Privacy is per *module* | E0603, E0451 |

## Smells and idioms

- **`get_` prefixes.** Rust style is `fn id(&self)`, not `fn get_id(&self)`.
  The databroker has `get_version()`, `get_datapoint()` etc. — understandable
  (some predate the convention, some read like "fetch"), but don't copy it
  in new APIs.
- **Public fields on types with invariants.** If a field must stay valid,
  make it private and provide constructors/methods. The databroker makes
  most data-model fields `pub` for convenience; `Permissions` is the
  counter-example that guards its invariants.
- **Constructors with long positional argument lists.** `add_entry`
  (`broker.rs:1380`) takes nine arguments and needs
  `#[allow(clippy::too_many_arguments)]`. A struct parameter or a builder
  would be clearer — another honest trade-off in real code.
- **Primitive obsession.** `i32` for ids, `String` for paths, `u32` for
  milliseconds. The databroker introduced `SignalId` and `TimeInterval`
  newtypes to fix exactly this in newer code.
- **Deriving `Default` when the default is meaningless.** Only implement
  `Default` when a sensible default exists.

## In the databroker

1. `broker.rs:89-118` — `Metadata`, `Datapoint`, `Entry`. Note `Entry`
   contains `Metadata` and `Datapoint` by value: an entry *owns* its parts.
2. `broker.rs:120-125` — `Field` and its derives.
3. `broker.rs:127-132` — `Database` with `#[derive(Default)]`.
4. `broker.rs:178-186` — `DataBroker`, `#[derive(Clone)]` — cloning it is
   cheap because its fields are `Arc`s (chapter 12).
5. `broker.rs:250-272` — `EntryUpdate`: many `Option`s + `Default`.
6. `broker.rs:274` onward — `impl Entry`: look at every receiver.
7. `types.rs:490-516` — `SignalId`, `TimeInterval`: newtypes with `new` and
   getter.
8. `broker.rs:2337-2521` — `impl DataBroker` (`new` at 2338) and
   `impl Default for DataBroker` (2517).

## Exercises

| Exercise | Goal |
|---|---|
| `c03_e01_define_struct` | Declare a struct, build it, pick the newer of two |
| `c03_e02_impl_methods` | `&self`, `&mut self`, `self` methods on a stats struct |
| `c03_e03_associated_fns_new_default` | `new`, hand-written and derived `Default` |
| `c03_e04_derives` | Choose derives; understand why `f64` blocks `Eq`/`Hash` |
| `c03_e05_struct_update_syntax` | Partial updates with `..Default::default()` |
| `c03_e06_validated_newtype` | `SignalId` newtype; a `VssPath` that can only be valid |

## Further reading

- The Rust Book, ch. 5 "Using Structs to Structure Related Data" —
  <https://doc.rust-lang.org/book/ch05-00-structs.html>; Appendix C
  "Derivable Traits".
- Rust API Guidelines, "Naming" (C-GETTER, C-CONV) —
  <https://rust-lang.github.io/api-guidelines/naming.html>
- "Parse, don't validate" (Alexis King) — the newtype idea in depth.
- `std::default::Default` docs.

## Check yourself

1. What can a method with `&self` do that one with no receiver can't, and
   what can `&mut self` do that `&self` can't?
2. Why does `Field` derive `Hash` and `Eq`?
3. Why can't `Datapoint` derive `Eq`?
4. What does `EntryUpdate { datapoint: Some(dp), ..Default::default() }`
   set `description` to?
5. How does a private field plus `parse` constructor guarantee validity?
6. Why is `impl Default for DataBroker` written as `Self::new("", "")`?

<details><summary>Answers</summary>

1. `&self` can read the instance's fields; `&mut self` can also modify them.
2. So `Field` values can be stored in a `HashSet<Field>` (set of changed fields).
3. It contains a `DataValue` that may hold `f32`/`f64`, which aren't `Eq` (NaN).
4. `None` — "don't change the description".
5. Outside the module nobody can construct the type directly, so every
   instance came from `parse`, which rejected invalid input.
6. So there is one place that knows how to build a `DataBroker`; the default
   can't get out of sync with `new`.

</details>
