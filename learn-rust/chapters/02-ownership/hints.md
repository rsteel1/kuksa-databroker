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

## c02_e07_build_path_buffer
No bug to fix — write each function from the spec in the tests.

`push_unique`: `paths.iter().any(|p| p == path)` tells you whether it's
already there; if not, `paths.push(path.to_string())` and return `true`.

`common_prefix`: split both strings on `'.'` and `.zip()` the two
iterators; walk it while segments are equal, accumulating the byte length
of each matched segment plus one (for the `.` that follows). At the end,
slice `a` up to that length minus one (to drop the trailing separator you
counted after the last match). Because you only ever borrow from `a`, you
can return `&'a str` with no allocation.

`dedup_paths`: build a fresh `Vec::new()` and call `push_unique` on it for
every path in the input, in order.

`take_first`: clamp `n` with `n.min(paths.len())` before slicing, so an
`n` larger than the buffer can't panic.

## c02_e08_review_reading_buffer
Compare each function's doc comment against what it actually does.

Bug 1 (`set_reading`): `let mut working = entries.clone();` makes a
*second*, independent `Vec`. Every mutation after that line changes
`working`, never `entries` — the caller's buffer is untouched. Drop the
clone and mutate `entries` directly; `find_index(entries, path)` already
works on a borrow of it.

Bug 2 (`append_batch`): the function reads `entries.len()` into
`count_before` *before* the loop that appends the batch, then returns that
stale count. Move the read to the end: `entries.len()` after the loop.

Bug 3 (`average_of_last`): `entries[entries.len() - take_n..entries.len() - 1]`
always leaves off the very last entry, no matter what `n` is. The window
needs to run all the way to `entries.len()`:
`&entries[entries.len() - take_n..]`.
