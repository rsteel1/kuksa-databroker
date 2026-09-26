# 05 — Collections

**Why this matters for the databroker.** At its heart the databroker is a
handful of collections behind locks: a map from signal path to id, a map from
id to entry, a map of subscriptions, a list of actuation providers. Almost
every request is "look something up in a map, compare some sets, build a
result list". Knowing which collection to use — and what each one guarantees
about *order* and *cost* — is how you read `broker.rs` without getting lost.

All of these types live in the standard library (`std::collections`) except
`IndexMap`, which comes from the `indexmap` crate.

| Type | What it is | Order | Lookup |
|---|---|---|---|
| `Vec<T>` | growable array | insertion order | by index, O(1) |
| `HashMap<K, V>` | hash table | **none** (random) | by key, O(1) average |
| `HashSet<T>` | `HashMap<T, ()>` | **none** | membership, O(1) |
| `BTreeMap<K, V>` | balanced tree | **sorted by key** | by key, O(log n) |
| `IndexMap<K, V>` | hash table + vec | **insertion order** | by key, O(1) |

## Vec: the workhorse

```rust
let mut readings: Vec<f32> = Vec::with_capacity(8); // one allocation up front
readings.push(12.5);
readings.push(13.0);
let last: Option<&f32> = readings.last();   // Option: the Vec may be empty
let tail: &[f32] = &readings[1..];          // a slice: a *view*, no copy
readings.drain(..1);                        // remove a range in one pass
```

Key ideas:

- A `Vec<T>` **owns** its elements. It lives in three words on the stack
  (pointer, length, capacity) and its elements on the heap.
- `&[T]` (a *slice*) is a borrowed view of some contiguous elements. Functions
  that only read should take `&[T]`, not `&Vec<T>` — then they work with
  arrays and sub-ranges too.
- Indexing with `v[i]` **panics** when out of bounds. `v.get(i)` returns an
  `Option<&T>` instead. Prefer `get`, `first`, `last` when absence is normal.
- `remove(0)` shifts every remaining element left: O(n). Removing the front
  repeatedly is O(n²). Use `drain(..k)` once, or a `VecDeque` if you need a
  real queue.
- `saturating_sub` avoids integer underflow: `2usize.saturating_sub(5) == 0`.
  (`2usize - 5` panics in debug builds.)

## HashMap and the entry API

```rust
use std::collections::HashMap;

let mut path_to_id: HashMap<String, i32> = HashMap::new();
path_to_id.insert("Vehicle.Speed".to_string(), 0);

// `get` takes a &str even though keys are String — thanks to the Borrow trait.
let id: Option<&i32> = path_to_id.get("Vehicle.Speed");
let id: Option<i32> = path_to_id.get("Vehicle.Speed").copied();
```

The **entry API** is how you do "insert if missing, otherwise update" in one
lookup:

```rust
let mut counts: HashMap<String, usize> = HashMap::new();
*counts.entry("Vehicle".to_string()).or_insert(0) += 1;
```

`entry(k)` returns an `Entry` that is either `Occupied` or `Vacant`;
`or_insert(v)` gives you a `&mut V` either way. There's also
`or_insert_with(|| expensive())` and `or_default()`.

Things to know:

- Keys must implement `Eq + Hash`. That's why the databroker's `Field` enum
  derives `PartialEq, Eq, Hash` — so it can be put into a `HashSet<Field>`.
- **Iteration order is random** and can change between runs. Never rely on it
  in tests or in anything a client sees.
- `insert` returns the old value (`Option<V>`) if the key was already there.

## HashSet and set algebra

A `HashSet<T>` answers "is x in here?" in O(1), and gives you set operations:

```rust
a.is_disjoint(&b)     // no common element?
a.intersection(&b)    // iterator over common elements (&T)
a.union(&b)
a.difference(&b)      // in a, not in b
a.is_subset(&b)       // every element of a is in b
```

These methods read like the specification. Compare:

```rust
// hand-written: what is this checking?
let mut hit = false;
for f in fields { for c in changed { if f == c { hit = true; } } }

// set algebra: obvious
let hit = !fields.is_disjoint(changed);
```

`intersection` yields references; add `.copied()` (for `Copy` types) or
`.cloned()` before `.collect()` if you need owned values.

## BTreeMap: sorted keys

A `BTreeMap` keeps its keys sorted at all times. That gives you:

- deterministic iteration order (alphabetical for strings),
- `first_key_value()` / `last_key_value()`,
- **range queries**: `map.range(start..)` jumps to the first key `>= start`.

Because VSS paths are dotted strings, every child of `Vehicle.Cabin` sorts
right after the string `"Vehicle.Cabin."` — a range query plus `take_while`
finds a whole branch without scanning the map. Watch the trailing dot:
`"Vehicle.CabinX"` starts with `"Vehicle.Cabin"` but is *not* a child.

## IndexMap: when order is part of the contract

`IndexMap` (crate `indexmap`) behaves like a `HashMap` for lookups, but
iterates in **insertion order**. Use it when a result must come back in a
specific order *and* you need keyed access while building it. The
databroker's `GetValues` contract says values are returned in the order the
client requested them; `get_values_broker` gathers values from providers and
the database (keyed by `SignalId`), then rebuilds an `IndexMap` in request order.

Note: `IndexMap::remove` is deprecated in favour of `swap_remove` (fast, but
moves the last element into the hole) or `shift_remove` (keeps order, O(n)).

## Removing while iterating: retain

```rust
for (id, sub) in &self.subscriptions {
    if sub.receiver_count == 0 {
        self.subscriptions.remove(id); // E0502!
    }
}
```

The `for` loop holds a shared borrow (`&`) of the map for its whole duration;
`remove` needs an exclusive borrow (`&mut`). Rust refuses (E0502). In Java
this compiles and throws `ConcurrentModificationException` at runtime; in
C++ it compiles and corrupts your iterator. Rust turns the bug into a compile
error.

The idiomatic fix is `retain`, which walks the collection once and keeps the
elements for which your closure returns `true`:

```rust
let mut removed = Vec::new();
self.subscriptions.retain(|id, sub| {
    let keep = sub.receiver_count > 0;
    if !keep { removed.push(*id); }
    keep
});
```

`Vec::retain` keeps the survivors in their original order.

## Mental model

| Collection | Analogy |
|---|---|
| `Vec<T>` | a numbered row of lockers; you own every locker |
| `&[T]` | permission to look at lockers 3..7 |
| `HashMap<K,V>` | a coat check: hand in a ticket (key), get your coat; the rack has no order |
| `BTreeMap<K,V>` | a dictionary: always alphabetical, so you can open it "at C" |
| `IndexMap<K,V>` | a guest list: look anyone up by name, but read it top to bottom in arrival order |
| `retain` | a bouncer walking the line once, asking each person "are you staying?" |

**Guarantee the compiler gives you:** you cannot mutate a collection while
something is iterating over (borrowing) it. No iterator invalidation, ever.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "HashMap iterates in insertion order" | Order is unspecified and changes run to run; use `IndexMap` or `BTreeMap` | flaky tests |
| "I can remove items inside a `for` over the map" | The loop borrows the map; use `retain`, or collect keys first | E0502 |
| "`v[i]` returns None when out of range" | It panics; `v.get(i)` returns `Option` | panic |
| "`map.get(k)` gives me the value" | It gives `Option<&V>`; you may need `.copied()`/`.cloned()` | E0308 |
| "Any type can be a HashMap key" | Keys need `Eq + Hash` (derive them) | E0277 |
| "`intersection` returns a new set" | It returns a lazy iterator of `&T` | E0308 |

## Smells and idioms

- **Taking `&Vec<T>` or `&String` as a parameter.** Take `&[T]` / `&str`.
- **Index loops** (`for i in 0..v.len() { v[i] ... }`). Iterate directly:
  `for x in &v`, or with `.iter().enumerate()` if you need the index.
- **Double lookup**: `if map.contains_key(k) { map.get(k).unwrap() }`. Use
  `if let Some(v) = map.get(k)` or the entry API.
- **Sorting a HashMap by collecting to a Vec every time** — if you always
  need order, choose `BTreeMap`/`IndexMap` instead.
- **`remove(0)` in a loop** — O(n²); use `drain` or `VecDeque`.
- **In the databroker:** `valid_provider_publish_signals` does
  `.get(&provider_uuid).as_ref().unwrap()` — a missing uuid would panic the
  task instead of returning an error. It's "safe" only because callers always
  pass a registered uuid. That kind of invariant deserves an `expect("...")`
  message at least, or a proper `Option` return.
- **In the databroker:** `register_signals` computes the "already registered"
  set with `.filter(|id| registered.contains(id))` rather than
  `HashSet::intersection` — equivalent, because `registered_vss_ids` is a set.

## In the databroker

- `databroker/src/broker.rs:128` — `struct Database { next_id, path_to_id:
  HashMap<String, i32>, entries: HashMap<i32, Entry> }`. Two maps: path → id,
  id → entry. Lookups by path are one extra hash away.
- `databroker/src/broker.rs:1281` and `:1329` — inside `DatabaseWriteAccess::add`:
  if the path already exists, return the existing id (idempotent registration);
  otherwise `path_to_id.insert(name, id)`.
- `databroker/src/broker.rs:135` — `struct Subscriptions` holds a `Vec` of
  actuation subscriptions and two `HashMap<Uuid, _>`.
- `databroker/src/broker.rs:160` — `EntryUpdates { updates: Vec<ChangeNotification> }`,
  the batch sent to a subscriber.
- `databroker/src/broker.rs:939` — `if !fields.is_disjoint(changed_fields)`:
  the check "does this change concern this subscription?".
- `databroker/src/broker.rs:857`, `:872`, `:892` — the three `cleanup*`
  functions, all built on `retain`, removing subscriptions whose client went
  away (`receiver_count() == 0`) or whose token expired.
- `databroker/src/broker.rs:2086` — rejecting a provider that asks for signals
  another provider already owns.
- `databroker/src/broker.rs:2197` — `signal_request.is_subset(key_set)`: a
  provider may only publish what it registered.
- `databroker/src/broker.rs:2218` — `provider.vss_ids.intersection(&requested_signals)`:
  which requested signals can this provider serve?
- `databroker/src/broker.rs:2321`–`2329` — rebuild the answer as an `IndexMap`
  in request order.
- `databroker/src/vss.rs:400` — `flatten_vss_tree` produces a
  `BTreeMap<String, DataEntry>`; `databroker/src/main.rs:163` iterates it, so
  signals get ids in alphabetical order on every start.

## Exercises

| Exercise | Goal |
|---|---|
| `c05_e01_vec` | bounded reading history with `push`, `drain`, slices |
| `c05_e02_hashmap_path_to_id` | path ⇄ id registry; entry API |
| `c05_e03_hashset_ops` | subscription matching with `is_disjoint`, `intersection`, `is_subset` |
| `c05_e04_btreemap_sorted` | sorted VSS paths; range queries for a branch |
| `c05_e05_indexmap_order` | GetValues with provider fallback, in request order |
| `c05_e06_retain_cleanup` | E0502, then subscription cleanup with `retain` |

## Further reading

- The Rust Book, ch. 8 "Common Collections" — https://doc.rust-lang.org/book/ch08-00-common-collections.html
- `std::collections` module docs ("When should you use which collection?") — https://doc.rust-lang.org/std/collections/index.html
- `HashMap::entry` — https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.entry
- `indexmap` crate docs — https://docs.rs/indexmap

## Check yourself

1. Why does `databroker` keep both `path_to_id` and `entries` instead of one
   `HashMap<String, Entry>`?
2. Your test compares `map.keys().collect::<Vec<_>>()` to a fixed list and
   passes locally but fails in CI. What's wrong?
3. What does `HashMap::get` return, and why not just `V`?
4. Why is `for (k, v) in &map { map.remove(k); }` rejected? What do you use instead?
5. When would you choose `BTreeMap` over `HashMap`? `IndexMap` over both?
6. What's the difference between `&Vec<f32>` and `&[f32]` as a parameter type?

<details><summary>Answers</summary>

1. Ids are small, cheap to copy and hash, and are what the wire protocol and
   subscriptions use internally; the string is resolved once at the edge.
   Also, `entries` can be keyed by id while metadata still carries the path.
2. HashMap iteration order is unspecified; sort first, or use an ordered map.
3. `Option<&V>`: the key may be absent, and the value stays owned by the map
   (you get a borrow, not the value).
4. The loop holds `&map` while `remove` needs `&mut map` (E0502). Use `retain`,
   or collect the keys to remove first and remove them after the loop.
5. `BTreeMap` for sorted/deterministic iteration and range queries;
   `IndexMap` when you need keyed access *and* insertion order (e.g. "return
   in request order").
6. `&[f32]` is more general: it accepts a `&Vec<f32>` (auto-deref), an array,
   or a sub-slice. `&Vec` adds nothing for a reader.
</details>
