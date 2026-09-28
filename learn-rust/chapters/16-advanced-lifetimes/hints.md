# Chapter 16 hints

## c16_e01_static_bound_vs_static_ref
`rustc --explain E0310`. The compiler even suggests the bound to add. Ask
yourself why `thread::spawn` needs it: could the new thread still be running
after `describe_in_background` returns?

Add `'static` to `T`'s bounds. `String`, `Vec<i32>` and `&'static str` all
satisfy it; a borrow of a local wouldn't.

For `count_matching_scoped`: `std::thread::scope(|s| { let a = s.spawn(|| ...); ... })`.
Split with `paths.split_at(paths.len() / 2)`, count each half in its own
scoped thread, and add the two `join()` results.

## c16_e02_lifetime_bounds
Start with E0107: the tests write `CheckSet<'static>`, so `CheckSet` needs a
lifetime parameter. Then read the "lifetime may not live long enough" error
in `make_prefix_check`: what lifetime does `Box<dyn Fn(&str) -> bool>` get by
default?

`Box<dyn Trait>` means `Box<dyn Trait + 'static>`. Closures that borrow
`prefix`, `root` or `banned` aren't `'static`.

`-> Box<dyn Fn(&str) -> bool + '_>`; `type PathCheck<'a> = Box<dyn Fn(&str) -> bool + 'a>;`;
`struct CheckSet<'a> { checks: Vec<PathCheck<'a>> }`; `impl<'a> CheckSet<'a>`;
and `add(&mut self, check: impl Fn(&str) -> bool + 'a)`.

## c16_e03_hrtb
E0597 says `trimmed` doesn't live long enough. Who chooses `'a` in
`normalize_all<'a, F: Fn(&'a str) -> &'a str>`? Can a local created inside
the function live for a lifetime the caller chose?

The closure must work for every lifetime: `F: for<'x> Fn(&'x str) -> &'x str`
(or simply `F: Fn(&str) -> &str`, whose elided form means the same).

Give `hr` that same bound (`where F: for<'x> Fn(&'x str) -> &'x str`), and in
`segment_fn` write `hr(|s: &str| s.split('.').next().unwrap_or(s))`, passing the
closure literal straight into `hr` instead of binding it with `let`.

## c16_e04_variance
Both bugs are in signatures. For E0597: after
`add_defaults(&mut paths)`, what type does the compiler think `paths` has?

`&mut T` is invariant in `T`: passing `&mut Vec<&str>` where
`&mut Vec<&'static str>` is expected forces the vector's element type to be
`&'static str`, so `&owned` must live forever.

`fn add_defaults(paths: &mut Vec<&str>)`. For `set_slot`, don't tie the
`&mut` to the data's lifetime: `fn set_slot<'a>(slot: &mut &'a str, value: &'a str)`.

## c16_e05_reborrowing_and_nll
E0382: `record` is generic in `W`, so passing `out` moves the `&mut String` into
it. How do you create a new, temporary `&mut` from an existing one?

`record(&mut *out, line)`: an explicit reborrow.

For `value_or_insert`, the returned borrow from `map.get` is considered live
until the end of the function (NLL problem case #3). Don't hold a borrow when
you mutate: `if !map.contains_key(key) { map.insert(key.to_string(), 0); }`
then `&map[key]`.

## c16_e06_self_referential_problem
Every error points at the tests, but the problem is `add(&'a mut self, ...)`.
How long does that borrow last, given that `'a` is the struct's own lifetime
parameter?

Storing references into `self.entries` inside `self` can't work: the `Vec`
may reallocate. Store *positions* instead, the way the databroker stores ids.

Remove the lifetime parameter. Replace `by_path` with
`path_to_id: HashMap<String, usize>`. `add` checks `contains_key`, pushes the
entry, and inserts `(path.to_string(), index)`. `entry_by_path` looks up the
index and calls `self.entry(id)`. `set_value` indexes `self.entries[id]`.
`#[derive(Default)]` gives you `new`.

## c16_e07_build_window_visitor
`for_each_window`: `for window in data.windows(size) { f(window); }` — the
`for<'s>` bound is already satisfied by any ordinary closure body; you
don't write `for<'s>` yourself anywhere, it's just what makes calling `f`
with a fresh, shorter-lived slice on every iteration type-check.
`PathLogger::new`: `PathLogger { label: label.into(), entries: Vec::new() }`.
`make_logger`: clone the label into a local (`let label =
self.label.clone();`) BEFORE building the closure, then `move |path: &str|
format!("{label}: {path}")`. Capturing `&self.label` instead would tie the
closure to `self`'s lifetime and fail the `'static` bound.

## c16_e08_review_static_and_reborrow_bugs
Three independent bugs:
- `Cache::update`: always `push`es. Look for an existing entry first
  (`iter_mut().find(...)`) and overwrite its value in place; only push if
  none was found.
- `make_snapshots`: every closure holds `Rc::clone(&counter)` — the SAME
  cell — so they all see the counter's final value once the loop ends.
  Copy the current value out of the `RefCell` (`*counter.borrow()`) into a
  plain `i32` before building the closure, and capture that instead.
- `quadruple`: `v.clone()` operates on a throwaway copy. Just call
  `double_in_place(v)` twice, directly — passing a `&mut` reference to a
  function reborrows it implicitly, so `v` is still yours to use again
  right after.
