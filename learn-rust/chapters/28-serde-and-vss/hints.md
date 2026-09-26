# Chapter 28 hints

## c28_e01_derive_deserialize
The error E0277 says serde_json doesn't know how to build a `SignalInfo`.
`serde_json::from_str::<T>` requires `T: Deserialize`. You already imported
the `Deserialize` derive macro — add it to the `#[derive(...)]` list on the
struct: `#[derive(Debug, Deserialize)]`.

## c28_e02_rename_default_optional
Field attributes go directly above the field:
`#[serde(rename = "type")]`. Do it for `entry_type`, `data_type` and
`change_type`. For `deprecated`, `#[serde(default)]` means "use
`Default::default()` when the key is missing" — `false` for a `bool`.
`Option<T>` fields don't need `default`; serde already treats a missing key
as `None`.

## c28_e03_enum_representations
Container attributes go above the enum: `#[serde(rename_all = "lowercase")]`
turns `OnChange` into `"onchange"`. `rename_all` can't produce `"string[]"`,
so those two variants also need `#[serde(rename = "string[]")]` /
`#[serde(rename = "int32[]")]` (a per-variant rename wins over `rename_all`).
For `Scalar`, add `#[serde(untagged)]` and reorder the variants so the most
specific ones come first: `Bool`, `Int`, `Float`, `Text`. JSON `3` fits both
`i64` and `f64`; untagged picks the first one that succeeds.

## c28_e04_serde_json_value
`serde_json::from_value::<i8>(value)?` both parses and range-checks. Then
widen with `.into()` (`i32::from(i8)` exists): `DataValue::Int32(x.into())`.
Because `ParseError: From<serde_json::Error>`, `?` converts the error for you.
For `allowed_for_type`: decide the array type with a `match`, return an
error for other types, then call
`value_for_type(Value::Array(values), &array_type)`.

## c28_e05_flatten_vss_tree
Structure of `add_entry`:
```rust
match entry.entry_type {
    EntryType::Branch => { /* children.ok_or_else(...)?; for (name, child) in children { add_entry(entries, format!("{path}.{name}"), child)?; } */ }
    other => { /* let Some(data_type) = entry.data_type else { return Err(...) }; entries.insert(path, DataEntry { ... }); */ }
}
Ok(())
```
`parse_vss_from_str`: `let root: RootEntry = serde_json::from_str(json)?;`
then loop over `root.0` calling `add_entry` — `root.0` is how you reach the
inner value of a tuple struct.
