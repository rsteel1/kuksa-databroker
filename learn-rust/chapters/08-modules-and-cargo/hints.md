# Chapter 08 hints

## c08_e01_inline_modules_and_pub
Follow the errors one by one. The test module is outside `broker`, so it can
only see what `broker` marks `pub`: the `types` module, the `DataType` enum,
the `DataBroker` struct, and the methods `new`, `version`, `commit_sha`,
`type_of`. You do NOT need `pub` on the enum variants (a `pub enum` makes
its variants public) nor on the struct fields (the tests only use methods).
`sanitize` is only used inside `broker`, so it stays private.

## c08_e02_use_paths_and_reexports
Inside `mod grpc`, the path `broker::type_of` is looked up relative to
`grpc` itself — and there is no `grpc::broker`. Write either
`use crate::broker::type_of;` (from the crate root) or
`use super::broker::type_of;` (from the parent). Same for `types::DataValue`.
In `mod broker`, add
```rust
pub use crate::types;
pub use crate::types::{DataType, DataValue};
```
The first makes `broker::types::...` work; the second makes
`broker::DataType` work. It's exactly what databroker/src/broker.rs:16-18 does.

## c08_e03_visibility_pub_crate
`raw_insert` lives in `broker::database` and is called from `broker` — its
parent. `pub(super) fn raw_insert(...)` is exactly "visible to my parent".
The field `next_id` is read from `grpc`, which is not inside `broker`, so
`pub(super)` isn't enough: use `pub(crate) next_id: i32`.
(`pub(in crate::broker)` would express the first one too.)

## c08_e04_cfg_attributes
Two functions with the same name can coexist in source only if cfg removes
one of them: `#[cfg(debug_assertions)]` above the "debug" one and
`#[cfg(not(debug_assertions))]` above the "release" one.
For `Snapshot`: keep `#[derive(Debug, Clone)]` and add a second attribute
`#[cfg_attr(test, derive(PartialEq))]` — "if `test` is set, apply this attribute".
`readiness_mechanism`: `if cfg!(target_os = "linux") { "sd_notify" } else { "none" }`.
Finally put `#[cfg(test)]` above `fake_snapshot`. Try `cargo build -p exercises`
afterwards (not test): the function simply isn't compiled there.

## c08_e05_build_signal_store_module
No bug to fix — `mod signals` and the `pub use` re-exports are already
wired up; you write the method/function bodies.

`record`: check `path.starts_with("Vehicle.")` first and return an `Err`
if it fails; otherwise push a `Reading { path: path.to_string(), value }`
and return `Ok(())`.

`snapshot`: `&self.entries` — a `Vec<Reading>` coerces to `&[Reading]`.

`latest`: you want the *last* match, so search from the end:
`self.entries.iter().rev().find(|r| r.path == path)`.

`debug_dump`: collect `store.snapshot()` into a `Vec<&Reading>`,
`.sort_by(|a, b| a.path.cmp(&b.path))`, then build the string with one
`format!("{}={}\n", ...)` push per reading (or `write!` into a
pre-allocated `String`, as in chapter 6).

## c08_e06_review_module_boundary
Compare each function's doc comment against what it actually does.

Bug 1 (`segment_count`): `split('.')` already yields one item per
segment (a string with no dots is still one segment), so subtracting 1
undercounts every path by one. Drop the `- 1`.

Bug 2 (`register_valid`): `let _ = super::pathing::validate(path);` runs
the check across the module boundary and then throws the answer away —
the very next line pushes `path` unconditionally. Only push inside
`if super::pathing::validate(path).is_ok() { ... }`.

Bug 3 (`rolling_average`): inside the inner loop, `let sum = sum + value;`
declares a new local that shadows the outer accumulator and disappears at
the end of that iteration — the outer `sum` never moves off `0.0`. Use
`sum += value;` instead.
