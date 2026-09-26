# 04 — Enums and pattern matching

**Why this matters for the databroker.** If structs are the nouns of the
databroker, enums are its vocabulary of *possibilities*: a value is one of
17 `DataValue` variants; a signal is a `Sensor`, `Attribute` or `Actuator`;
a write fails with one of eight `UpdateError`s; a provider's message is one
of seven actions. `match` is how the code handles each possibility — and
the compiler checks that none was forgotten. `Option` and (in chapter 07)
`Result` are enums too. This is the most "Rust-flavoured" chapter so far.

---

## Enums: one of several variants

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryType {
    Sensor,
    Attribute,
    Actuator,
}
```

A value of type `EntryType` is exactly one of those three. There is no
"null", no fourth value, no integer that happens to be out of range. Compare
with using strings (`"sensor"`) or integers (`2`): with an enum a typo is a
compile error, and the set of possibilities is written down in one place.

`match` does something different for each variant:

```rust
fn can_actuate(entry_type: EntryType) -> bool {
    match entry_type {
        EntryType::Actuator => true,
        EntryType::Sensor | EntryType::Attribute => false,   // `|` = "or"
    }
}
```

`match` is an expression (it produces a value), arms are tried top to
bottom, and together they **must cover every variant**.

The real enums: `types.rs:76-80` `EntryType`, `types.rs:83-87` `ChangeType`,
and the error enums at the top of `broker.rs` (`ActuationError` at 42,
`UpdateError` at 64, `ReadError` at 76, …).

## Variants that carry data

Each variant can hold its own data — different types for different variants:

```rust
enum DataValue {
    NotAvailable,
    Bool(bool),
    String(String),
    Int32(i32),
    Double(f64),
    Int32Array(Vec<i32>),
}
```

In memory an enum is a *tag* saying which variant it is, plus room for the
largest variant's data. You can only get at the data by matching, which
checks the tag first:

```rust
match value {
    DataValue::Double(v) => println!("a double: {v}"),   // v is the f64
    DataValue::Int32Array(items) => println!("{} ints", items.len()),
    DataValue::NotAvailable => println!("no value yet"),
    _ => println!("something else"),
}
```

Contrast with a C union or a dynamically-typed language: there's no way to
read a `Double`'s bits as a `String` by mistake. This is called a *sum type*
or *tagged union* and it's the single most useful type-design tool in Rust.

**Matching on a reference.** In a method taking `&self`, `match self { DataValue::Double(v) => ... }`
binds `v` as `&f64` (a reference into the value — no move out of borrowed
data). Use `*v` to get the `f64`. This convenience is called "match
ergonomics" / default binding modes.

The real `DataValue` (`types.rs:89-108`) has 17 variants. Note what's
missing: there's no `Int8` or `Uint16` *value* variant. Small integers are
stored as `Int32`/`Uint32` and range-checked against the signal's
`DataType` on write (chapter 01's `try_from` check). The `DataType` enum
(`types.rs:17-42`), which describes what a signal *should* hold, does list
all 24 VSS types.

## Exhaustive matching and the wildcard trap

If a `match` misses a variant, compilation fails:

```
error[E0004]: non-exhaustive patterns: `DataType::Uint8` not covered
```

This is a superpower during maintenance: add a variant to an enum and the
compiler lists *every* `match` in the codebase that needs a decision.

Unless someone wrote a wildcard:

```rust
fn is_array(t: DataType) -> bool {
    match t {
        DataType::StringArray | DataType::Int32Array => true,
        _ => false,          // silently swallows BoolArray, added later!
    }
}
```

`_` means "anything else". It's fine when "anything else" is genuinely
uniform and will stay so (e.g. matching on strings or integers, where you
*can't* list everything). For your own enums, prefer listing variants
explicitly when a future variant would need a conscious decision.

The databroker gets this right in `impl fmt::Display for DataType`
(`types.rs:44-73`): 24 explicit arms. It also shows the risk in
`vss.rs:513-531` `determine_change_type`, which uses
`_ => types::ChangeType::Continuous` for "everything but attributes" —
reasonable today, but a new `EntryType` would silently be `Continuous`.

Other pattern tools you'll see:
- `x @ pattern` binds the whole value while matching a pattern.
- **Guards**: `Some(end) if end <= len => ...` — the arm applies only if
  the extra condition holds.
- Ranges: `0..=15 => 1`.

## Option: a value or nothing

Rust has no `null`. "Maybe a value" is an ordinary enum from the standard
library:

```rust
enum Option<T> {
    Some(T),
    None,
}
```

`Option<String>` is either `Some(text)` or `None`, and the compiler won't
let you treat it as a `String` until you've handled `None`. The billion-
dollar mistake (null pointer exceptions) is simply not expressible.

Ways to deal with an `Option`, from most explicit to most concise:

```rust
match unit {                                    // full match
    Some(u) => println!("unit {u}"),
    None => println!("no unit"),
}

if let Some(u) = unit { println!("unit {u}"); } // only care about Some

let Some(u) = unit else { return None; };       // let-else: bind or bail out

let u = unit?;              // in a fn returning Option: None → return None early

unit.unwrap_or("")          // default value
unit.map(|u| u.len())       // transform the inside, keep None as None
unit.is_some() / is_none()
unit.expect("reason")       // panic if None — only when None is a bug
```

`?` on an `Option` is used in the databroker's lookup —
`broker.rs:1161-1164`:

```rust
pub fn get_metadata_by_path(&self, path: &str) -> Option<&Metadata> {
    let id = self.db.path_to_id.get(path)?;   // unknown path → None
    self.get_metadata_by_id(*id)
}
```

And `if let` in `Entry::diff` (`broker.rs:277`):
`if let Some(datapoint) = &update.datapoint { ... }`.

`Metadata` (`broker.rs:90-103`) uses `Option` for things a signal may not
have: `min`, `max`, `allowed`, `unit`.

## Option<Option<T>>: the tri-state update

An update struct uses `None` to mean "don't touch this field". But what if
the field itself is optional, and you want to *clear* it? A single
`Option` can't express three states:

| Update value | Meaning |
|---|---|
| `None` | leave the field as it is |
| `Some(None)` | set the field to "nothing" |
| `Some(Some(x))` | set the field to `x` |

Hence `Option<Option<T>>`. The databroker explains it in a comment right in
`EntryUpdate` (`broker.rs:256-259`):

```rust
// Actuator target is wrapped in an additional Option<> in
// order to be able to convey "update it to None" which would
// mean setting it to `Some(None)`.
pub actuator_target: Option<Option<Datapoint>>, // only for actuators
```

and handles it in `Entry::apply` (`broker.rs:758-779`) by peeling one layer:

```rust
if let Some(actuator_target) = update.actuator_target {   // part of the update?
    self.actuator_target = actuator_target;              // Option<Datapoint>, may be None
    changed.insert(Field::ActuatorTarget);
}
```

(An alternative design is a dedicated enum `enum Patch<T> { Keep, Clear, Set(T) }`
— more self-describing, but more code. The databroker's choice is common in
Rust and you'll see it in other codebases, e.g. JSON "PATCH" handling.)

## Matching two values at once

Match on a tuple to handle pairs of enums together:

```rust
match (self, other) {
    (DataValue::Bool(a), DataValue::Bool(b)) => Some(a > b),
    (DataValue::Double(a), DataValue::Double(b)) => Some(a > b),
    (DataValue::Double(a), b) => b.as_i64().map(|b| *a > b as f64),
    (a, b) => match (a.as_i64(), b.as_i64()) {
        (Some(a), Some(b)) => Some(a > b),
        _ => None,
    },
}
```

Arms are checked in order, so specific cases go first and catch-alls last.
This is exactly how `DataValue::greater_than` (`types.rs:137`) compares a
value with a signal's `min`/`max`, which may be stored in a different
variant — e.g. `(Int32(v), Int64(o)) => Ok(i64::from(*v) > *o)` at
`types.rs:140-142`. It's used by `Entry::validate_value_min_max`
(`broker.rs:497`).

The real function is ~150 lines because it spells out every numeric pair
explicitly and returns `Result<bool, CastError>`. Explicit is safer (the
compiler checks every pair), shorter is easier to read — a trade-off you'll
make yourself.

---

## Mental model

| Concept | Analogy |
|---|---|
| enum | a multiple-choice question: exactly one box is ticked |
| variant with data | each answer comes with its own attachment (a number, a string…) |
| `match` | a checklist that the compiler refuses to accept until every answer is handled |
| `_` | "…and everything else" — including answers added next year |
| `Option<T>` | a parcel that is either there or explicitly "not delivered"; you must check before opening |
| `?` on Option | "if the parcel isn't there, stop and report 'not delivered' upward" |
| `Option<Option<T>>` | a form field with "don't change" / "clear" / "new value" |

**What the compiler guarantees:** you can't read a variant's data without
checking it's that variant; every `match` handles all cases; there's no
null, so no null-pointer crashes.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Enums are named integers" | Rust enums can carry data per variant (sum types) | — |
| "`_ =>` is harmless" | It hides variants added later | (silent bug) |
| "`Option` is like nullable, I can just use it" | You must unwrap/match before using the inner value | E0308, E0599 |
| "`unwrap()` is how you get the value" | `unwrap` panics on `None`; prefer `?`, `if let`, `unwrap_or`, `map` | runtime panic |
| "`?` works on anything in any function" | `?` on `Option` needs the function to return `Option` (Result → Result) | E0277 |
| "Matching on `&self` moves the data out" | It binds references (`&T`) automatically | E0507 if you force a move |
| "Missing a variant is a warning" | It's a hard error | E0004 |

## Smells and idioms

- **Stringly-typed code** (`"sensor"`, `"actuator"` passed around) → enums.
  Parse strings into enums at the border (like `from_name`), then use enums
  inside.
- **Boolean flags that encode states** (`is_actuator: bool, is_sensor: bool`)
  → one enum; impossible combinations disappear.
- **Wildcards on your own enums** where a new variant would need a decision.
- **`if x.is_some() { x.unwrap() }`** → `if let Some(v) = x`.
- **Deeply nested `match`** → early returns with `let ... else`, `?`, or
  helper functions. The databroker's `open_provider_stream`
  (`grpc/kuksa_val_v2/val.rs:909` onward; the big `match req.action` at
  `val.rs:944`) nests `match`es eight levels deep inside a `select!` — it
  works, but it's a known readability pain point and a good refactoring
  exercise once you've finished the async chapters (20–24).
- `match x { Some(v) => Some(f(v)), None => None }` → `x.map(f)`.

## In the databroker

1. `types.rs:17-42` `DataType` and `types.rs:44-73` its `Display` impl — a
   fully explicit match.
2. `types.rs:89-108` `DataValue` — the sum type at the heart of the broker.
3. `types.rs:137-160` `greater_than` — matching on `(&self, other)` pairs.
4. `broker.rs:41-87` — error enums (`ActuationError`, `UpdateError`, …).
5. `broker.rs:250-272` `EntryUpdate` — `Option` and `Option<Option<T>>` fields.
6. `broker.rs:517-560` `validate_value` — `match self.metadata.data_type`,
   with an inner `match value` per type.
7. `broker.rs:1072-1075` `EntryReadAccess` — an enum whose variants hold
   *references*: either the entry, or its metadata plus an error
   ("you may see that it exists, but not read it"). Chapter 11 revisits it.
8. `vss.rs:513-531` `determine_change_type` — nested `match` on
   `Option<ChangeType>` with a wildcard fallback.

## Exercises

| Exercise | Goal |
|---|---|
| `c04_e01_simple_enum` | Add a variant; `match` with `\|`; parse strings into an enum |
| `c04_e02_enum_with_data` | Bind data inside variants; `as_f64` returning `Option` |
| `c04_e03_exhaustive_match` | E0004; replace a bug-hiding wildcard; map array → element type |
| `c04_e04_option` | `unwrap_or`, `?` on Option, `let ... else`, `if let` |
| `c04_e05_option_option_tristate` | Apply keep / clear / set updates like `Entry::apply` |
| `c04_e06_match_on_tuples` | Compare mixed numeric variants like `greater_than` |

## Further reading

- The Rust Book, ch. 6 "Enums and Pattern Matching" and ch. 19 "Patterns
  and Matching" (in older editions ch. 18) —
  <https://doc.rust-lang.org/book/ch06-00-enums.html>
- `std::option` module docs — the combinator table is worth bookmarking:
  <https://doc.rust-lang.org/std/option/>
- Rust by Example: "Flow of Control → match", "if let", "let-else".
- `rustc --explain E0004`.

## Check yourself

1. Why is `enum EntryType` better than `const SENSOR: u8 = 0;` etc.?
2. How does Rust stop you from reading the `f64` inside a `DataValue` that
   is actually a `String`?
3. When is a `_ =>` arm the right choice, and when is it a trap?
4. What does `let id = map.get(path)?;` do if `path` isn't in the map?
5. What are the three meanings of `Option<Option<Datapoint>>` in an update?
6. Why doesn't `DataValue` have an `Int8` variant?

<details><summary>Answers</summary>

1. Only the three valid values exist; typos are compile errors; `match`
   must handle all three; the type documents intent.
2. The data is only reachable through a pattern like `DataValue::Double(v)`,
   which checks the variant tag first.
3. Right: when the remaining cases are genuinely uniform or can't be listed
   (strings, numbers). Trap: on your own enums when a future variant would
   need its own handling.
4. The function returns `None` immediately.
5. `None` = don't change; `Some(None)` = clear it; `Some(Some(dp))` = set it.
6. Small integers are stored in `Int32`/`Uint32`; the signal's `DataType`
   says `Int8`, and writes are range-checked with `try_from`.

</details>
