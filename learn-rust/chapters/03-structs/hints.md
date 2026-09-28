# Chapter 03 hints

## c03_e01_define_struct
E0422: nothing named `Datapoint` exists yet. The shape is:
```rust
struct Name {
    field_name: FieldType,
    other: OtherType,
}
```
In `make_speed`, `value,` is *field init shorthand* for `value: value,`.

`newer_of`: compare `a.timestamp_ms` and `b.timestamp_ms` and return one of
the two (moving it out). Return `b` only if it's strictly newer.

## c03_e02_impl_methods
Inside a method, `self.count` reads a field; with `&mut self` you can also
assign: `self.count += 1;`.

`i64` has `min`/`max` methods: `self.min = self.min.min(value);`.

`average`: guard `count == 0` first (division by zero on floats gives NaN,
which isn't what the test wants). Convert: `self.sum as f64 / f64::from(self.count)`.

`into_report` takes `self`; use `format!("n={} min={} max={} avg={:.2}", ...)`.
`{:.2}` prints two decimals.

## c03_e03_associated_fns_new_default
E0599 "no function or associated item named `new`": write an impl block:
```rust
impl ServerSettings {
    fn new(address: &str, port: u16) -> Self {
        Self { address: address.to_string(), port, enable_unix_socket: false }
    }
}
```
`Default` is a trait with one method:
```rust
impl Default for ServerSettings {
    fn default() -> Self { Self::new("127.0.0.1", 55555) }
}
```
For `Counters`, every field type already implements `Default`, so add
`Default` to its `#[derive(...)]` line.

## c03_e04_derives
Read each error: E0369 `==` needs `PartialEq`; `assert_eq!` also needs
`Debug` to print values; `.clone()` needs `Clone`; `HashSet` needs
`Eq + Hash`; `let b = a;` then using `a` needs `Copy` (and `Copy` requires
`Clone`).

`SignalKey` (just an `i32`): `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`.

`Reading` has an `f64` (not `Eq`, not `Hash`, because of NaN) and a `String`
(not `Copy`). What's left that the tests need? `Debug, Clone, PartialEq`.

## c03_e05_struct_update_syntax
E0063 lists the missing fields. You don't want to type `None` five times:
end the struct literal with `..Default::default()` (no trailing comma after it).
It works because `EntryUpdate` derives `Default`.

`metadata_update`: `description: Some(description.to_string())`, same for
unit, then `..Default::default()`.

`describe_update`: `Option::is_some()` tells you whether a field is set.
Count them.

## c03_e06_validated_newtype
Tuple structs access their field by position: `self.0`. Construct with
`SignalId(id)`.

`parse`: reject empty input; then `input.split('.')` gives each segment.
Reject an empty segment, and reject any char that isn't
`c.is_ascii_alphanumeric() || c == '_'`. Finally `Some(VssPath(input.to_string()))`.

`as_str`: `&self.0` (a `&String` coerces to `&str`).

`leaf`: `self.0.split('.').next_back()` gives the last segment as an
`Option<&str>`; it can't be `None` for a valid path, so
`.expect("valid paths are non-empty")` is appropriate — the invariant is
guaranteed by `parse`.

## c03_e07_build_entry_stats
No bug to fix — design the struct and write every method from the tests.

Fields: something like `path: String, count: u32, sum: f64, min: f64,
max: f64`.

`new`: `count: 0` and zero for the rest; `min`/`max` don't matter yet
because `record` will set them on the first call.

`record`: on the very first reading (`self.count == 0`) set `min` and
`max` to `value` directly; otherwise `self.min = self.min.min(value)` and
`self.max = self.max.max(value)`. Always add to `sum` and increment
`count`.

`range`/`average`: guard `count == 0` and return `0.0`; otherwise
`max - min`, and `sum / f64::from(count)`.

`into_summary` takes `self` (it consumes the tracker): build the string
with `format!("{}: n={} min={} max={} avg={:.2}", ...)`.

## c03_e08_review_signal_registry
Compare each method's doc comment against what it actually does.

Bug 1 (`update`): `self.entries.iter().find(...).cloned()` makes an
independent copy of the matching `Entry`. Mutating `entry.value` on that
copy never touches `self.entries`. Use `iter_mut()` and find a `&mut
Entry` to mutate directly.

Bug 2 (`nth_most_recent`): for `n = 0` (the most recent entry),
`self.entries.len() - n` is `len()`, one past the last valid index — the
last element is at `len() - 1`. In general the `n`th-from-the-end index
is `len() - 1 - n`.

Bug 3 (`register`): `let next_id = id + 1;` declares a brand new local
that shadows nothing useful and is thrown away at the end of the
function; `self.next_id` is never touched, so it stays `0` forever and
every registration gets id `0`. Write `self.next_id = id + 1;` instead.
