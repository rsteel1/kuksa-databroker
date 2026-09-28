# Chapter 04 hints

## c04_e01_simple_enum
E0599 "no variant named `Actuator`": add `Actuator,` to the enum.

`match` must cover every variant. Several variants can share an arm with `|`:
```rust
match entry_type {
    EntryType::Actuator => true,
    EntryType::Sensor | EntryType::Attribute => false,
}
```
You *can* match on string slices too: `match name { "sensor" => ..., _ => None }`.
For strings a `_` arm is required — there are infinitely many strings.

## c04_e02_enum_with_data
A pattern can bind the data inside a variant: `DataValue::Double(v) => ...`.
Because you match on `&self`, `v` is a *reference* (`&f64`) — use `*v`.

If you don't need the data, write `DataValue::Bool(_)`.

`as_f64`: `f64::from(*v)` converts `i32` and `u32` losslessly.

`len`: `DataValue::Int32Array(values) => values.len()`, `NotAvailable => 0`,
and `_ => 1` for the rest is fine here (the wildcard is deliberate: "every
other variant is a single value").

## c04_e03_exhaustive_match
E0004 names the missing variant. Add the arm for `DataType::Uint8`.

`is_array`: the test for `BoolArray` fails because `_ => false` swallowed it.
Replace the wildcard by listing every scalar variant explicitly:
`DataType::String | DataType::Bool | ... => false`. Now, if someone adds
`DataType::DoubleArray`, this `match` stops compiling until they decide.

`element_type`: one arm per array type returning `Some(scalar type)`, and
the scalars listed explicitly returning `None`.

## c04_e04_option
`find_id`: `for &(registered, id) in registry { if registered == path { return Some(id); } }`
then `None` after the loop. (Chapter 10 shows `iter().find_map(...)`.)

`display_unit`: `unit.unwrap_or("")`.

`leaf_len`: inside a function returning `Option`, `expr?` means "if `expr` is
`None`, return `None` right now; otherwise give me the value":
`let (_, leaf) = path.rsplit_once('.')?; Some(leaf.len())`.

`parse_interval_ms`: `text.strip_suffix("ms")` returns `Option<&str>`, so
`let number = text.strip_suffix("ms")?;` works. `number.parse::<u32>()`
returns a `Result`, which `?` can't convert to `None` here. `let ... else`
destructures a pattern or bails out:
```rust
let Ok(ms) = number.parse::<u32>() else { return None; };
Some(ms)
```
(`number.parse().ok()` is an equally good one-liner.) `u32` parsing rejects
"-5" by itself.

`describe_min`: `if let Some(value) = min { format!("min={value}") } else { ... }`.

## c04_e05_option_option_tristate
Peel one layer at a time:
```rust
if let Some(new_value) = update.value {   // field is part of the update
    // new_value: Option<i32> — maybe Some(20), maybe None ("clear it")
    if new_value != entry.value { ... }
}
```
Assign `entry.value = new_value;` and push `"value"` onto the result only
when it actually differs. Do the same for `unit` (you can move `new_unit`
into `entry.unit` because `update` is owned).

## c04_e06_match_on_tuples
Match on a tuple of references: `match (self, other) { ... }`. Arms are
tried top to bottom, so put the specific ones first:
```rust
(DataValue::Bool(a), DataValue::Bool(b)) => Some(a > b),
(DataValue::String(a), DataValue::String(b)) => Some(a > b),
(DataValue::Double(a), DataValue::Double(b)) => Some(a > b),
(DataValue::Double(a), b) => b.as_i64().map(|b| *a > b as f64),
(a, DataValue::Double(b)) => a.as_i64().map(|a| a as f64 > *b),
(a, b) => /* both integers? compare as i64, else None */
```
`opt.map(|x| ...)` transforms the value inside a `Some` and leaves `None` alone.

`less_than_or_equal`: "not greater than": `self.greater_than(other).map(|g| !g)`.

## c04_e07_build_datavalue_ops
No bug to fix — the tests are the spec, and you write every method.

`type_name`: one match, one `&'static str` literal per variant.

`as_f64`: `Int(v) => Some(*v as f64)`, `Float(v) => Some(*v)`, anything
else `None`. (`Bool(_) | Text(_) => None` matches both in one arm.)

`greater_than`: match the *pair* `(self.as_f64(), other.as_f64())`; only
`(Some(a), Some(b))` gives an answer, `a > b`.

`clamp_numeric` takes `self` by value, so you can move out of it: match
`self`, and for `Int(v)`/`Float(v)` build a new variant from the clamped
number (`(v as f64).clamp(min, max) as i64` for `Int`, `v.clamp(min, max)`
for `Float`); anything else, return it unchanged (`other => other`).

## c04_e08_review_entry_apply
Compare each function's doc comment against what it actually does.

Bug 1 (`apply`): the doc comment lists three distinct cases, but the
match only spells out two of them (`None` and `Some(Some(v))`) and lumps
the third, `Some(None)`, into the trailing `_`. That arm returns
`NoChange` and never touches `current` — a clear request is silently
ignored. Add an explicit `Some(None) => { *current = None; ChangeType::Cleared }`
arm.

Bug 2 (`latest_value`): `0..changes.len().saturating_sub(1)` never visits
the last index, so the very last change is always ignored. Loop over
`changes` directly (`for change in changes`) instead of by index.

Bug 3 (`describe_changes`): inside the `Updated` arm,
`let updates = updates + 1;` declares a brand new local that shadows the
outer counter and vanishes at the end of the arm — the outer `updates`
never moves off `0`. Use `updates += 1;` instead.
