# Chapter 02 hints

## c02_e01_move
E0382 "borrow of moved value: `speed`". Run `rustc --explain E0382`.

`append_unit(speed, ...)` *moves* the String into the function. After that
line the variable `speed` is an empty husk the compiler won't let you touch.

You don't need a clone: you only need the *length* of `speed`. Compute
`speed.len()` (a plain `usize`, which is Copy) on the line *before* the move.

## c02_e02_clone_vs_move
Two separate errors.

`register`: `registry.paths.push(path)` moves `path` into the Vec, so the
next line can't read it. `format!("registered {path}")` only *reads* `path`
and builds a new String — so build the log line first, then move `path` in.

`snapshot`: you only have `&Registry` — a loan. You may look at
`registry.paths` but you can't take it away from its owner (E0507: "cannot
move out of ... which is behind a shared reference"). The caller wants its
own list: `registry.paths.clone()`.

## c02_e03_borrow
`average(readings)` moves the Vec into `average`; the next call has nothing
left to pass.

Change the parameters to `readings: &[u32]` and call with `&readings`.
Inside `maximum`, iterating a `&[u32]` yields `&u32` items; the pattern
`for &reading in readings` copies each number out of its reference.

## c02_e04_mut_borrow
E0596: you can't take `&mut readings` if `readings` itself isn't `mut`.
`let mut readings = Vec::new();`

E0502: `let first = &readings[0];` keeps a shared loan into the Vec alive
until `first` is last used. `push` may reallocate the Vec's buffer — `first`
would then point at freed memory, so Rust forbids it. `i32` is Copy: take
the value, not a reference: `let first = readings[0];`

`clamp_all`: `for reading in readings.iter_mut() { *reading = (*reading).clamp(min, max); }`
`*reading` means "the i32 this `&mut i32` points to".

## c02_e05_string_vs_str
E0308: the tests pass `&str` / `&String`, the function wants an owned
`String`. The function only reads — change the parameter to `&str`.

`path.rsplit_once('.')` returns `Option<(&str, &str)>`: `Some((before, after))`
splitting at the LAST dot, or `None` if there is no dot. Use `match`:
```rust
match path.rsplit_once('.') {
    Some((_, leaf)) => leaf,
    None => path,
}
```
`join_path` creates new text, so it returns a `String`:
`format!("{branch}.{leaf}")`, or `leaf.to_string()` if `branch` is empty.

## c02_e06_slices_and_returning_refs
E0106: the compiler can't tell whether the returned `&str` borrows from `a`
or from `b`. Declare a lifetime and tie them together:
`fn longer_path<'a>(a: &'a str, b: &'a str) -> &'a str`.

`default_branch` has no input to borrow from — the only candidate is the
local `branch`, which is dropped when the function returns. Return the
owned value: `fn default_branch() -> String { String::from("Vehicle") }`.

`window`: `start + len` can overflow for huge inputs — use
`start.checked_add(len)`, which gives `None` on overflow. Then:
```rust
match start.checked_add(len) {
    Some(end) if end <= readings.len() => &readings[start..end],
    _ => &[],
}
```
