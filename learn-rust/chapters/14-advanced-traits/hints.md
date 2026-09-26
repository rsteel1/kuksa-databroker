# Chapter 14 hints

## c14_e01_associated_types_vs_generics
Write `impl SignalSource for DoorSensor { type Value = bool; fn next_value(&mut self) -> Option<bool> { ... } }`.
Return `None` once `toggles_left` is 0. Otherwise decrement it, flip `open`,
and return the new state.

`drain`: `while let Some(v) = source.next_value() { out.push(v) }`. The
return type `Vec<S::Value>` needs no extra generic because each `S` has
exactly one `Value`.

The last error says `Reading` has no `ConvertFrom<bool>`. A generic trait can
be implemented many times for one type, once per `T`. Add a second impl
next to the `f32` one.

## c14_e02_supertraits_and_blanket_impls
E0277 `Gps: Named`: `Provider: Named` means a type can't be a `Provider`
until it is also `Named`. Add `impl Named for Gps`.

For the blanket impl, write `impl<T: Display> Describe for T { ... }`.
Inside, `self.to_string()` exists because std has its own blanket impl,
`impl<T: Display + ?Sized> ToString for T`. Count characters with
`.chars().count()` (not `.len()`, see chapter 06).

## c14_e03_orphan_rule_newtype
`rustc --explain E0117`. Neither `Display` nor `Vec` belongs to your crate.
Change the impl's target from `Vec<Reading>` to your wrapper `Readings`, and
iterate over `self.0` instead of `self`.

`From<Vec<Reading>> for Readings` is allowed because `Readings` is local.
Once `From` exists, `.into()` works automatically (std provides
`impl<T, U: From<T>> Into<U> for T`).

## c14_e04_operator_overloading
Each operator method returns a new value built from the inner numbers:
`Speed(self.0 + rhs.0)`. `AddAssign` mutates in place: `self.0 += rhs.0`.

`Index` must return a *reference*. `self.values.get(path)` gives
`Option<&f64>`. On `None`, `panic!("no signal named {path}")`.

`IndexMut` is what `table["x"] = 42.0` uses, and it should create the entry
if it's missing: `self.values.entry(path.to_string()).or_insert(0.0)`.

`Priority`: "lower number = more important = greater" means the comparison
is reversed: `other.0.cmp(&self.0)`. Make `partial_cmp` return
`Some(self.cmp(other))` so the two can never disagree (clippy checks this).

## c14_e05_deref_asref_borrow
Three small impls, each returning `&self.0` (a `&String`, which coerces to `&str`):

```rust
impl Deref for VssPath { type Target = str; fn deref(&self) -> &str { &self.0 } }
```

`AsRef<str>` has `fn as_ref(&self) -> &str`, and `Borrow<str>` has
`fn borrow(&self) -> &str`.

Why does `map.get("x")` need `Borrow`? `HashMap::get` is generic:
`get<Q>(&self, k: &Q) where K: Borrow<Q>, Q: Hash + Eq`. With
`K = VssPath, Q = str`, the bound is exactly `VssPath: Borrow<str>`. It is
only correct because the derived `Hash`/`Eq` on `VssPath` give the same
answers as those on `str`.

## c14_e06_any_downcast
`value.downcast_ref::<i32>()` returns `Option<&i32>`. Chain `if let`s. Note
that `"hello"` is a `&'static str`, which is a different type from `String`,
so test for both.

`find_plugin`: iterate and use `find_map`. `plugin.as_ref()` gives
`&dyn Plugin`. Because `Plugin: Any`, it coerces to `&dyn Any` (trait
upcasting, Rust 1.86+): `let any: &dyn Any = plugin.as_ref();` and then
`any.downcast_ref::<T>()`.

Careful: calling `plugin.downcast_ref` or `(&plugin as &dyn Any)` on the
`Box` itself asks whether the *Box* is a `T`, and the answer is always no.

## c14_e07_sealed_and_marker_traits
`rustc --explain E0277`. `Rc`'s reference count isn't atomic, so it is
neither `Send` nor `Sync`, and neither is anything containing it. The
thread-safe version is `Arc` (chapter 12). Change the `use`, the field type
and `new`.

For the sealed trait, write `impl private::Sealed for Sensor {}` (empty body)
and then `impl SignalKind for Sensor { const NAME: &'static str = "sensor"; const WRITABLE: bool = false; }`.
Do the same for `Actuator`. Those impls must be inside `mod kinds`, where
`private` is visible.

`publish` just needs to format the string. The `T: Validated` bound is the
whole point, and the value itself isn't used (write `let _ = value;` to say so).

## c14_e08_typestate_phantomdata
`rustc --explain E0392`. A struct must use every type parameter. Add a field
`state: PhantomData<S>`, whose value is written `PhantomData`.

`new()` fills in the defaults: an empty Vec, 0, None, `PhantomData`.

`buffer_size`/`interval_ms` live in `impl<S>`, take `mut self`, set a field,
and return `self`. Clamp with `size.min(MAX_SUBSCRIBE_BUFFER_SIZE)`.

`signal` changes the type from `SubscribeBuilder<S>` to
`SubscribeBuilder<HasSignals>`, so you can't return `self` as-is. Build a new
struct, moving each field across and setting `state: PhantomData`.

`build` only exists in `impl SubscribeBuilder<HasSignals>`. Move the three
fields into a `SubscribeRequest`.
