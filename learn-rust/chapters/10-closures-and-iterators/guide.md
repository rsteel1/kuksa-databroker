# 10 — Closures & iterators

**Why this matters for the databroker.** Almost every read path in the
databroker is an iterator pipeline over the entries of the database, and
almost every "do something while holding the lock" helper takes a closure.
`AuthorizedAccess::map_entries`, `filter_map_entries`, `for_each_entry`,
`with_read_lock`, `Subscriptions::cleanup` with `retain`,
`get_values_broker` with `intersection().copied().collect()` — once you can
read closures and iterator chains fluently, big parts of `broker.rs` become
short and obvious instead of dense and scary.

---

## Closures and what they capture

A closure is an anonymous function written `|args| body`:

```rust
let limit = 100;
let too_fast = |speed: u32| speed > limit;   // uses `limit` from the scope
assert!(too_fast(120));
```

Unlike a plain `fn`, a closure can *use variables from where it was created*.
The compiler decides **how** it captures each variable from how the body uses
it — always the least powerful way that works:

| Body does…                      | Captured as | Closure implements          |
|---------------------------------|-------------|-----------------------------|
| only reads `x`                  | `&x`        | `Fn` (+ `FnMut` + `FnOnce`) |
| mutates `x` (`x.push(..)`)      | `&mut x`    | `FnMut` (+ `FnOnce`)        |
| moves `x` out (`drop(x)`, returns it) | `x` by value | `FnOnce` only          |

The `move` keyword overrides the *capture mode*: everything used is moved
(or copied, for `Copy` types like `u32`) into the closure. It does **not**
change which traits the closure implements — that still depends on what the
body does.

Why you need `move`:

```rust
fn above_limit(limit: u32) -> impl Fn(u32) -> bool {
    |speed| speed > limit        // E0373: borrows `limit`…
}                                 // …which dies here, but the closure lives on
```

The returned closure would hold a reference to a dead local. `move |speed|
speed > limit` copies `limit` into the closure instead. You'll write
`move` constantly in async code: `tokio::spawn(async move { ... })` — the
task may run long after the spawning function returned, so it must own its
data.

A closure that mutates its captures has *state* that changes when called, so
calling it needs `&mut closure`:

```rust
let mut seen = Vec::new();
let mut record = |r| seen.push(r);   // `let mut` — E0596 without it
record(1);
record(2);
// `seen` is mutably borrowed by `record` until `record`'s last use
```

## Fn, FnMut and FnOnce

These are the three traits a function parameter can require:

```text
            FnOnce   "can be called at least once"
              ▲
            FnMut    "can be called repeatedly, may mutate captures"
              ▲
             Fn      "can be called repeatedly, only reads captures"
```

Every `Fn` is also `FnMut` and `FnOnce`. So as a *caller of `f`*, ask for the
**weakest** trait your code needs — that accepts the most closures:

- You call `f` exactly once → `impl FnOnce(..)`. The databroker's
  `with_read_lock(&self, f: impl FnOnce(&DatabaseReadAccess) -> T) -> T`
  does exactly this: take the read lock, call `f` once, release.
- You call `f` in a loop and it may accumulate → `impl FnMut(..)`, and bind
  the parameter as `mut f`. See `for_each_entry(&self, f: impl FnMut(EntryReadAccess))`.
- You call `f` many times, possibly concurrently, and it must not mutate →
  `impl Fn(..)`.

`impl Trait` in argument position is shorthand for a generic:
`fn with_read<T>(&self, f: impl FnOnce(&Db) -> T)` ≡
`fn with_read<T, F: FnOnce(&Db) -> T>(&self, f: F)`. The compiler generates a
specialised copy for each closure type (monomorphisation), so there's no
runtime cost compared to writing the loop by hand.

## Iterator adaptors and collect

An `Iterator` is anything with `fn next(&mut self) -> Option<Self::Item>`.
You rarely call `next` yourself; you chain **adaptors** (which return a new
iterator) and finish with a **consumer** (which pulls the items):

```rust
let actuators: Vec<&str> = entries
    .iter()                                        // yields &Metadata
    .filter(|m| m.entry_type == EntryType::Actuator)
    .map(|m| m.path.as_str())                      // yields &str
    .collect();                                     // consumer
```

Key points:

- **Lazy.** `filter`/`map` do nothing until a consumer runs. A chain with no
  consumer is a warning (`unused must_use`).
- **Three ways to iterate a collection:** `iter()` (yields `&T`),
  `iter_mut()` (`&mut T`), `into_iter()` (yields `T`, consuming the
  collection). `for x in &v` is `v.iter()`; `for x in v` is `v.into_iter()`.
- **`collect` is driven by the target type.** The same chain can build a
  `Vec`, `HashSet`, `String`, or — from `(K, V)` tuples — a `HashMap`:

  ```rust
  let ids: HashMap<String, i32> = entries.iter().map(|m| (m.path.clone(), m.id)).collect();
  let set: HashSet<SignalId>    = vss_signals.iter().copied().collect();   // broker.rs
  ```

  Tell `collect` what to build with a type annotation on the `let` or the
  turbofish `collect::<Vec<_>>()`.
- **`filter_map`** = `map` to an `Option`, keep only the `Some`s. Perfect
  for "parse, skip failures": `raw.iter().filter_map(|s| s.parse().ok())`.
- **`copied()` / `cloned()`** turn an iterator of `&T` into `T` (for `Copy`
  types / via `Clone`).

## Collecting Results

`Result<Vec<T>, E>` implements `FromIterator<Result<T, E>>`. Collecting stops
at the first `Err` — **fail fast**:

```rust
let ids: Result<Vec<i32>, _> = raw.iter().map(|s| s.parse::<i32>()).collect();
```

The same works for `Option` (`None` stops) and for `sum`/`product`.

Sometimes fail-fast is wrong. `AuthorizedAccess::update_entries`
(`databroker/src/broker.rs:1529`) applies *every* update it can and reports a
`Vec<(i32, UpdateError)>` of *all* failures, so a client publishing 10 signals
learns about all 3 bad ones at once. With iterators, `partition` splits a
stream in two:

```rust
let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
```

## Writing your own iterator

To make your own type iterable, implement one method:

```rust
impl<'a> Iterator for EntryReadIterator<'a, '_> {
    type Item = EntryReadAccess<'a>;            // what `next` yields
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| EntryReadAccess::new(entry, self.permissions))
    }
}
```

That is the real code (`databroker/src/broker.rs:1119`). It **wraps** the
standard `hash_map::Values` iterator and converts each `&Entry` into a
permission-checked `EntryReadAccess`. Once `next` exists, every adaptor
(`map`, `filter`, `count`, `collect`…) works on it for free — they're default
methods of the `Iterator` trait.

`size_hint` is optional; forwarding the inner iterator's hint lets
`collect` pre-allocate the right capacity.

## Consumers: any, all, fold, find

| Consumer | Returns | Short-circuits |
|----------|---------|----------------|
| `count()` | `usize` | no |
| `any(pred)` | `bool` — true if some item matches | yes, at first `true` |
| `all(pred)` | `bool` — true if every item matches (true for empty) | yes, at first `false` |
| `find(pred)` | `Option<Item>` — first match | yes |
| `fold(init, f)` | accumulated value | no |
| `sum()` / `max()` / `min()` | … | no |

These replace the "`let mut found = false; for … { if … { found = true;
break; } }`" pattern. Compare the databroker's matching code in
`ChangeSubscription::notify` (`databroker/src/broker.rs:936`):

```rust
let mut matches = false;
for (id, changed_fields) in changed {
    if let Some(fields) = self.entries.get(id) {
        if !fields.is_disjoint(changed_fields) {
            matches = true;
            break;
        }
    }
}
```

with the iterator version you write in `c10_e06`:

```rust
changed.iter().any(|(id, changed_fields)| {
    subscribed.get(id).is_some_and(|wanted| !wanted.is_disjoint(changed_fields))
})
```

Both compile to essentially the same machine code. The second has no mutable
flag to get wrong.

`chain` glues two iterators end to end: `primary.iter().chain(fallback)`.

## Mental model

| Concept | Analogy |
|---------|---------|
| Closure | A function with a backpack: it carries (references to, or copies of) the variables it uses |
| `move` | Pack the actual items into the backpack instead of notes saying where they are |
| `Fn` / `FnMut` / `FnOnce` | Read-only guest / guest who may rearrange the furniture / guest who may take things home |
| Iterator chain | An assembly line that only starts when someone at the end asks for an item |
| `collect` | The box at the end of the line — its shape (Vec, Set, Map) decides how items are packed |

**What the compiler guarantees:** a closure can never outlive the data it
borrows (E0373), never mutate through a shared capture, and an `FnOnce`
closure can never be called twice. Iterator chains borrow their source, so
you can't modify a collection while iterating over it (E0502).

## Common misconceptions

| Wrong model | Correct model | Error you'll see |
|-------------|---------------|------------------|
| "Closures always copy what they use" | They borrow by default; `move` moves/copies | E0373 |
| "`move` makes a closure `FnOnce`" | `move` changes capture mode; the traits depend on what the body *does* | — |
| "A closure that pushes to a Vec is just a function" | It has mutable state, so the binding needs `mut` | E0596 |
| "`Fn` is the most flexible bound" | `FnOnce` accepts the most closures; `Fn` accepts the fewest | E0525 |
| "`map` runs immediately" | Adaptors are lazy; nothing happens without a consumer | `unused_must_use` warning |
| "`collect()` always makes a Vec" | It builds whatever the target type says | E0282 (type annotations needed) |

## Smells and idioms

| Smell | Idiomatic |
|-------|-----------|
| `for i in 0..v.len() { v[i] … }` | `for x in &v` / `v.iter().enumerate()` |
| `let mut found = false; for … { … break }` | `any` / `find` / `position` |
| `let mut out = Vec::new(); for x in … { out.push(f(x)) }` | `.map(f).collect()` |
| `.collect::<Vec<_>>()` then iterate again | Keep chaining; collect once at the end |
| `.map(|x| x.clone())` | `.cloned()` (or `.copied()` for `Copy` types) |
| `.filter(..).next()` | `.find(..)` |
| `impl Fn` for a callback called once | `impl FnOnce` |

The databroker isn't always idiomatic here: `notify` uses the flag-and-break
loop above, and repeats the "does this subscription care?" check twice.
That's fine code — it's clear and fast — but when you add new code, prefer
the iterator form.

## In the databroker

- `databroker/src/broker.rs:1411` — `with_read_lock<T>(&self, f: impl FnOnce(&DatabaseReadAccess) -> T) -> T`.
  Lock, call once, unlock. Callers use it to run several reads under one lock.
- `databroker/src/broker.rs:1493`, `:1503`, `:1514` — `for_each_entry`
  (`FnMut`), `map_entries`, `filter_map_entries`: each takes the read lock,
  calls `iter_entries()`, and hands your closure to the matching std adaptor.
  They are thin wrappers — read them, they're 8 lines each.
- `databroker/src/broker.rs:1114`–`1133` — `EntryReadIterator` and its
  `Iterator` impl (with `size_hint`). Modelled in `c10_e05`.
- `databroker/src/broker.rs:1167` — `iter_entries` builds the iterator from
  `self.db.entries.values()`.
- `databroker/src/broker.rs:857`–`908` — `cleanup`, `cleanup_signal_providers_subscriptions`,
  `cleanup_change_subscriptions`: `Vec::retain` / `HashMap::retain` with a
  closure that *also* mutates an outer `Vec` (so it's `FnMut`).
- `databroker/src/broker.rs:936`–`944` — the matching loop discussed above.
- `databroker/src/broker.rs:2210`–`2231` — `get_values_broker` snapshots
  providers with `.values().filter_map(|provider| …).collect()`, using
  `intersection(..).copied().collect()` inside.
- `databroker/src/broker.rs:1529` — `update_entries(&self, updates: impl IntoIterator<Item = (i32, EntryUpdate)>)`
  accepts a Vec, an array, a `map` chain… anything iterable.

## Exercises

| Exercise | Goal |
|----------|------|
| `c10_e01_closure_capture` | Fix E0373/E0596: `move` and `let mut` closures |
| `c10_e02_fn_fnmut_fnonce` | Choose the weakest closure bound; implement `with_read`-style helpers |
| `c10_e03_iterator_adaptors` | `map`/`filter`/`filter_map`/`collect` into Vec, HashMap, HashSet |
| `c10_e04_collect_results` | Fail-fast collect vs. gather-all-errors like `update_entries` |
| `c10_e05_custom_iterator` | Implement `Iterator` for an `EntryReadIterator` look-alike |
| `c10_e06_subscription_matching` | `any`/`all`/`fold`/`chain`/`find`, rewriting the notify matcher |

## Further reading

- The Rust Book, ch. 13 "Functional Language Features: Iterators and Closures"
  — https://doc.rust-lang.org/book/ch13-00-functional-features.html
- Rust by Example, "Closures" and "Iterator::any / find" —
  https://doc.rust-lang.org/rust-by-example/fn/closures.html
- `std::iter::Iterator` docs (skim the list of provided methods) —
  https://doc.rust-lang.org/std/iter/trait.Iterator.html

## Check yourself

1. Why does returning `|x| x > limit` from a function fail, and why does
   `move` fix it?
2. A function calls its callback once. Which bound should it use, and why is
   that *more* flexible for callers than `Fn`?
3. What does `v.iter().map(f);` (no consumer) do?
4. How do you collect an iterator of `(String, i32)` into a map?
5. `update_entries` returns `Result<(), Vec<(i32, UpdateError)>>`. Why not
   stop at the first error?
6. What is the minimum you must implement to make a type an `Iterator`?

<details><summary>Answers</summary>

1. The closure borrows the parameter `limit`, which is dropped when the
   function returns while the closure lives on (E0373). `move` copies
   `limit` into the closure, so it owns what it needs.
2. `FnOnce`. Every closure implements `FnOnce`; only non-mutating,
   non-consuming closures implement `Fn`. The weaker the requirement, the
   more closures qualify.
3. Nothing — adaptors are lazy. The compiler warns that the result is unused.
4. `let m: HashMap<String, i32> = iter.collect();` — `HashMap` implements
   `FromIterator<(K, V)>`.
5. A publisher sending many values should learn about *every* rejected
   signal in one round trip, and valid updates should still be applied.
6. `type Item` and `fn next(&mut self) -> Option<Self::Item>`. Everything
   else has default implementations.

</details>

## Where this goes next

Streams (chapter 22) are the async version of iterators, with the same adaptor style. Chapter 15 covers function pointers and `Box<dyn Fn>`, and chapter 27 shows why iterator chains are zero-cost.
