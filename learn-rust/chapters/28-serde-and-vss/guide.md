# 28 — Serde & VSS JSON

> **Builds on:** chapters 04 (enums), 07 and 17 (errors), 09 (traits and derives), 12 (Box for recursive trees).

**Why this matters for the databroker.** Before the databroker can serve a
single request it has to know *which signals exist*: `Vehicle.Speed` is a
`float` sensor in km/h, `Vehicle.Cabin.Door.Row1.DriverSide.IsOpen` is a
`boolean` actuator, and so on. That catalogue is the **Vehicle Signal
Specification (VSS)**, delivered as a JSON file and loaded at startup with
`--vss vss.json`. All of that parsing lives in one file,
`databroker/src/vss.rs`, and it is built on **serde** — Rust's
serialization framework. After this chapter you can read `vss.rs` top to
bottom and know why every line is there.

---

## What serde is (and isn't)

serde splits the problem in two:

| Part | Who writes it | Example |
|---|---|---|
| **Data model side** — "what does my type look like?" | You, via `#[derive(Deserialize)]` | `struct Entry { description: String, ... }` |
| **Format side** — "how do I read JSON / YAML / TOML?" | A format crate | `serde_json`, `serde_yaml`, `toml` |

The derive macro generates an implementation of the `Deserialize` trait for
your type. That implementation doesn't know anything about JSON; it just
describes "I'm a struct with these fields". `serde_json::from_str::<T>(s)`
then drives it. That's why the same struct could be read from YAML by
swapping one function call.

Everything happens at **compile time**: no reflection, no runtime schema.
The generated code is roughly what you'd write by hand, and just as fast.

## Deriving Deserialize

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SignalInfo {
    description: String,       // required: missing key -> error
    datatype: String,
    unit: Option<String>,      // optional: missing key -> None
}

let info: SignalInfo = serde_json::from_str(r#"{"description":"Speed","datatype":"float"}"#)?;
```

Rules you get for free:

* A **missing required field** is an error: `missing field `description``.
* An `Option<T>` field may be missing (→ `None`) or `null` (→ `None`).
* **Unknown keys are ignored** by default. VSS files contain many keys the
  databroker doesn't care about (`uuid`, `comment`, `deprecation`...) — this
  default is what lets the databroker's `Entry` list only the fields it uses.
  Add `#[serde(deny_unknown_fields)]` on the struct if you want strictness.
* Nested types just need to implement `Deserialize` themselves:
  `Option<HashMap<String, Entry>>` works because `HashMap`, `String` and
  `Entry` all do.

If you forget the derive, the compiler says:

```text
error[E0277]: the trait bound `SignalInfo: serde::Deserialize<'de>` is not satisfied
```

Read it as: "`from_str::<SignalInfo>` has a `T: Deserialize` bound and your
type doesn't meet it". This E0277 shape — *a generic function's trait bound
isn't satisfied* — is one of the most common errors in all of Rust.

> **Aside — `'de`**: `Deserialize<'de>` has a lifetime so that a type can
> *borrow* from the input (`struct Borrowed<'a> { name: &'a str }`) instead
> of allocating. `serde_json::from_reader` can't lend you data from a file
> stream, so types used with it must own everything (`String`, not `&str`).
> That's why the databroker's `Entry` uses `String` everywhere. When you need
> "a type that deserializes from any input" you write the bound
> `T: for<'de> Deserialize<'de>` (a *higher-ranked* bound) or the shortcut
> `T: serde::de::DeserializeOwned`.

## Renaming fields and defaults

JSON keys don't have to be valid Rust identifiers; field attributes bridge
the gap:

```rust
#[derive(Debug, Deserialize)]
pub struct Entry {
    #[serde(rename = "type")]               // `type` is a Rust keyword
    entry_type: EntryType,
    #[serde(rename = "datatype")]           // Rust style is snake_case
    data_type: Option<DataType>,
    #[serde(rename = "x-kuksa-changetype")] // dashes aren't allowed in identifiers
    change_type: Option<ChangeType>,
    #[serde(default)]                       // missing key -> Default::default()
    deprecated: bool,
}
```

Useful attributes to know:

| Attribute | Where | Effect |
|---|---|---|
| `rename = "k"` | field / variant | use key `k` |
| `rename_all = "lowercase"` / `"camelCase"` / `"snake_case"` | struct / enum | rename every field / variant |
| `default` | field | missing → `Default::default()` |
| `default = "path::to_fn"` | field | missing → call a function |
| `alias = "k"` | field | also accept `k` |
| `skip` | field | never read (uses `Default`) |
| `deny_unknown_fields` | struct | unknown keys become errors |
| `flatten` | field | inline a nested struct's fields |

`Option<T>` does **not** need `default` — serde already treats missing
`Option` fields as `None`.

## Enums in serde

By default a unit variant is represented by its exact name: `"Sensor"`.
VSS writes `"sensor"`, `"onchange"`, `"string[]"`. Two ways to fix it:

```rust
// The databroker's way: one rename per variant (databroker/src/vss.rs:78)
#[derive(Debug, Deserialize)]
pub enum EntryType {
    #[serde(rename = "actuator")] Actuator,
    #[serde(rename = "attribute")] Attribute,
    #[serde(rename = "branch")] Branch,
    #[serde(rename = "sensor")] Sensor,
}

// Shorter: a container attribute, with per-variant overrides where needed
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DataType {
    Boolean,                          // "boolean"
    Int8,                             // "int8"
    #[serde(rename = "string[]")]     // rename_all would give "stringarray"
    StringArray,
}
```

An unknown string gives a helpful error:
``unknown variant `int128`, expected one of `boolean`, `int8`, ...``.

Enums *with data* have four representations — worth knowing because you'll
meet them in other JSON APIs (VISS in `databroker/src/viss/v2/types.rs`
uses several):

| Representation | Attribute | JSON for `Sensor { unit: "km/h" }` |
|---|---|---|
| externally tagged (default) | — | `{"Sensor": {"unit": "km/h"}}` |
| internally tagged | `#[serde(tag = "type")]` | `{"type": "Sensor", "unit": "km/h"}` |
| adjacently tagged | `#[serde(tag = "t", content = "c")]` | `{"t": "Sensor", "c": {"unit": "km/h"}}` |
| untagged | `#[serde(untagged)]` | `{"unit": "km/h"}` — first variant that fits wins |

**Untagged order matters.** With `enum Scalar { Float(f64), Int(i64) }`, the
JSON `3` becomes `Float(3.0)` because `Float` is tried first and succeeds.
Put the most specific variant first.

### Why vss.rs doesn't use a tagged enum

VSS entries look *exactly* like an internally tagged enum: `"type": "branch"`
decides which other fields exist. The natural model would be
`#[serde(tag = "type")] enum Entry { Branch(..), Sensor(..), ... }`. The
comment at `databroker/src/vss.rs:24` explains why they didn't: tagged and
untagged enums make serde buffer the input, which **loses line/column
numbers in error messages** — terrible when a user's 5000-line VSS file has a
typo. So the databroker uses one flat struct with the superset of all fields
(`Option` for the ones that only some entry types have) and checks
"sensor needs datatype" by hand. A deliberate trade-off: less type safety,
better error messages.

## serde_json::Value: typing data late

Some VSS fields change type depending on a sibling field: `"default": false`
for a boolean, `"allowed": ["LEFT", "RIGHT"]` for a string, `"max": 250` for
a uint8. serde reads fields in one pass and can't say "parse `max` as the
type named in `datatype`". Solution: read them as the dynamically-typed
`serde_json::Value`, and convert later:

```rust
min: Option<serde_json::Value>,        // vss.rs:54
allowed: Option<Vec<serde_json::Value>>,

// later, when the datatype is known (vss.rs:275):
types::DataType::Int8 => serde_json::from_value::<i8>(value)
    .map(|value| Some(types::DataValue::Int32(value.into())))
    .map_err(|err| err.into()),
```

Two lessons hide in that snippet:

1. **Parse with the declared width, store wider.** The broker only has
   `DataValue::Int32`, but parsing with `i8` means `"max": 300` on an `int8`
   is rejected. `.into()` then widens losslessly (`From<i8> for i32`).
2. **`From` for errors** — `impl From<serde_json::Error> for Error`
   (`vss.rs:167`) lets `.map_err(|err| err.into())` — or just `?` — convert
   serde's error into the module's own error type.

`Value` is also handy for exploring: `value["Vehicle"]["children"]["Speed"]`
indexes without a struct, returning `Value::Null` for missing keys.

## From a tree to flat paths

VSS is a tree; the broker wants a flat map keyed by dotted path. The recursion
in `add_entry` (`vss.rs:409`) is short:

```rust
fn add_entry(entries: &mut BTreeMap<String, DataEntry>, path: String, entry: Entry)
    -> Result<(), Error>
{
    match entry.entry_type {
        EntryType::Branch => match entry.children {
            Some(children) => {
                for (name, child) in children {
                    add_entry(entries, format!("{path}.{name}"), child)?;   // recurse
                }
                Ok(())
            }
            None => Err(Error::ParseError("children required for type branch".to_owned())),
        },
        EntryType::Actuator => { /* build a DataEntry, insert it */ }
        ...
    }
}
```

Things to notice:

* **The recursive type.** `struct Entry { children: Option<HashMap<String, Entry>> }`
  contains itself. That compiles because `HashMap` puts its values on the
  heap — the `Entry` struct itself only holds a pointer-sized map header. A
  struct holding `Entry` *directly* would be infinitely large (error E0072)
  and would need a `Box<Entry>`.
* **`?` inside recursion** unwinds the whole tree walk on the first error.
* **`&mut BTreeMap` is threaded through** instead of each call returning its
  own map and merging — no intermediate allocations.
* **`BTreeMap`, not `HashMap`**: keys come out sorted, so signals get
  registered in a stable, alphabetical order → deterministic ids across runs.
* `RootEntry(HashMap<String, Entry>)` (`vss.rs:22`) is a **tuple struct
  newtype** around the top-level map; `root.0` reaches inside.

The result is then fed to the broker in `read_metadata_file`
(`databroker/src/main.rs:153`), which calls `add_entry` on the broker for
every path and publishes the attribute's `default` value if present
(`main.rs:181`).

## Mental model

| Concept | Analogy |
|---|---|
| `#[derive(Deserialize)]` | A form template: "this document has these boxes" |
| format crate (`serde_json`) | The clerk who reads a specific language and fills in the form |
| `Option<T>` field | An optional box on the form |
| `rename` | "The box labelled `type` goes into my `entry_type` slot" |
| untagged enum | Trying each form template until one fits |
| `serde_json::Value` | Photocopy the page now, decide what it means later |

**Guarantee:** a successfully deserialized value is *well-typed* — a `u8`
field really holds 0..=255, a required field really exists. Anything that
doesn't fit is an `Err`, never a garbage value or a crash.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "serde uses reflection at runtime" | Code is generated at compile time by the derive macro. |
| "Unknown JSON keys cause errors" | Ignored unless `deny_unknown_fields`. |
| "`Option<T>` needs `#[serde(default)]`" | Missing `Option` fields are `None` automatically. |
| "Untagged picks the best match" | It picks the **first** match, in declaration order. |
| "E0277 on `from_str` is a serde bug" | Your type is missing `Deserialize`; the generic bound isn't met. |
| "I can deserialize `&str` fields from a file" | Readers can't lend data; use owned `String` (`DeserializeOwned`). |

## Smells and idioms

* **One rename per variant when `rename_all` would do.** `vss.rs` spells out
  every rename (`vss.rs:78`, `:103`). Explicit and greppable, but verbose;
  `#[serde(rename_all = "lowercase")]` plus a few overrides is the idiom.
* **Giant parallel `match`es.** `try_from_json_value` (`vss.rs:263`) has one
  near-identical arm per type. It's clear, but a small generic helper
  (`fn parse<T: DeserializeOwned>(v) -> Result<T, Error>`) would shrink it.
* **Copy-paste error strings.** The sensor and attribute branches of
  `add_entry` both say `"data_type required for actuator"` (`vss.rs:460`,
  `vss.rs:489`) — a real (harmless) bug. Include the *path* in messages, as
  you did in `c28_e05`: `"datatype required for Vehicle.Speed"` is far more
  useful to a user.
* **`let _ = entries.insert(...)`** — `insert` returns the *previous* value.
  Ignoring it explicitly is fine; but note a duplicate path silently
  overwrites. If duplicates must be errors, check `insert(..).is_some()`.
* Prefer `?` over `.map_err(|err| err.into())` when you're already in a
  function returning `Result` — they do the same thing.

## In the databroker

| Where | What to notice |
|---|---|
| `databroker/src/vss.rs:22` | `RootEntry` newtype around the top-level map |
| `databroker/src/vss.rs:24` | Why a tagged enum was *not* used (line numbers in errors) |
| `databroker/src/vss.rs:40` | `Entry`: superset struct, `rename`s, `serde_json::Value` fields |
| `databroker/src/vss.rs:64` | `DataEntry`: the *typed* output, fields are broker `types::*` |
| `databroker/src/vss.rs:103` | `DataType` renames incl. `"string[]"` (line 128) |
| `databroker/src/vss.rs:167` | `impl From<serde_json::Error> for Error` → enables `?` |
| `databroker/src/vss.rs:175` | `impl From<vss::DataType> for types::DataType` — parsing types vs domain types |
| `databroker/src/vss.rs:210` | `try_from_json_array`: `allowed` is an array of the base type |
| `databroker/src/vss.rs:263` | `try_from_json_value`: late typing with `from_value::<T>` |
| `databroker/src/vss.rs:368` | `try_from_json_single_value`: `min`/`max` of an array type are scalars |
| `databroker/src/vss.rs:400` / `:409` | `flatten_vss_tree` + recursive `add_entry` |
| `databroker/src/vss.rs:513` | `determine_change_type`: defaults when `x-kuksa-changetype` is absent |
| `databroker/src/vss.rs:533` | `parse_vss_from_reader<R: Read>` — generic over any reader |
| `databroker/src/vss.rs:549` | `test_parse_vss` — a full example document |
| `databroker/src/main.rs:153` | `read_metadata_file`: file → `BufReader` → parse → register |

Try it: `data/vss-core/` contains real VSS files. Open one and find
`Vehicle.Speed` — then trace which `Entry` fields it fills.

## Exercises

| Exercise | Goal |
|---|---|
| `c28_e01_derive_deserialize` | Fix E0277 by deriving `Deserialize`; optional fields |
| `c28_e02_rename_default_optional` | `rename` for `type` / `datatype` / `x-kuksa-changetype`; `default` |
| `c28_e03_enum_representations` | `rename_all`, per-variant renames, untagged order |
| `c28_e04_serde_json_value` | Late typing with `from_value::<T>`, range checks, `From` for errors |
| `c28_e05_flatten_vss_tree` | Recursive tree → sorted `BTreeMap` of dotted paths |
| `c28_e06_review_entry_attributes` | Review: 3 planted bugs — a wrong `rename` key, a wrong `default` value, and a wrong-case enum rename |

## Further reading

* serde book — <https://serde.rs/> ("Attributes", "Enum representations",
  "Deserializer lifetimes").
* `serde_json` docs — `Value`, `from_value`, `from_reader` (docs.rs/serde_json).
* Rust Book ch. 15.1 "Using `Box<T>` to Point to Data on the Heap" — for
  recursive types.
* VSS documentation — <https://covesa.github.io/vehicle_signal_specification/>.

## Check yourself

1. Why can `Entry` contain `Option<HashMap<String, Entry>>` but not `Option<Entry>`?
2. What happens to a JSON key your struct doesn't declare?
3. In an untagged `enum { Float(f64), Int(i64) }`, what does `3` become, and why?
4. Why does `vss.rs` parse an `int8` value with `from_value::<i8>` and then convert to `i32`?
5. What did the databroker authors give up by *not* using a tagged enum for `Entry`, and what did they gain?
6. Why is the result a `BTreeMap` rather than a `HashMap`?

<details><summary>Answers</summary>

1. `HashMap` stores values on the heap, so `Entry`'s size is finite; a direct
   `Option<Entry>` would need infinite size (E0072) — use `Option<Box<Entry>>`.
2. It's ignored (unless the struct has `deny_unknown_fields`).
3. `Float(3.0)` — untagged tries variants in order and `f64` accepts `3`.
4. To enforce the declared range (−128..=127) while parsing; the broker only
   stores `Int32`, and `i8 → i32` is lossless.
5. Gave up: compile-time guarantees that e.g. a sensor has a datatype (now a
   runtime check). Gained: line/column numbers in parse errors.
6. Sorted, deterministic iteration → signals are registered in a stable order.

</details>
