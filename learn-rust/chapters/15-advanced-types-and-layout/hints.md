# Chapter 15 hints

## c15_e01_size_and_layout
The test prints 24 but wants 16. Draw the fields as boxes on an 8-byte grid:
where does the compiler have to insert padding so the `u64` starts on a
multiple of 8?

With `#[repr(C)]` the order you write is the order in memory. Largest
alignment first, then descending, leaves no gaps between fields.

`timestamp_ns: u64`, then `signal_id: u32`, then the two `u8`s: 8 + 4 + 1 + 1 = 14,
rounded up to the alignment (8) is 16. `padding_bytes` is
`size_of::<T>() - sum_of_field_sizes`.

## c15_e02_niche_optimization
`Option<u32>` needs a tag because every `u32` bit pattern is a valid number.
Which standard type has a bit pattern that can never occur?

`std::num::NonZeroU32` can't be 0, so `Option<NonZeroU32>` uses 0 for `None`.
`NonZeroU32::new(x)` already returns `Option<NonZeroU32>`, and `.get()`
turns it back into a `u32`.

`fn new(id: u32) -> Option<Self> { NonZeroU32::new(id).map(SignalId) }`
(a tuple-struct name is a function). For `first_free_id`, iterate `1..`,
`filter_map(SignalId::new)`, and `find` the first one not in `used`.

## c15_e03_dsts_and_fat_pointers
`rustc --explain E0277`. The message says the size of `str` can't be known.
Look at the bounds on `report`: which bound do you *not* see written, but is
there anyway?

Every type parameter is implicitly `Sized`. `report` only uses `T` through
`&T`, so it doesn't need the size.

`fn report<T: HasLen + ?Sized>(item: &T) -> String`.

## c15_e04_const_generics
`N` is a value you can use in code (`if N == 0`, `% N`) and in types
(`[Option<T>; N]`).

`[None; N]` requires `T: Copy`. Use `std::array::from_fn(|_| None)` instead.
Track `head` (index of the oldest item) and `len`. The next free slot is
`(head + len) % N`.

When full, the slot to overwrite is `head` itself: `self.slots[self.head].replace(item)`
returns the old value, then advance `head`. `iter` is
`(0..self.len).filter_map(move |i| self.slots[(self.head + i) % N].as_ref())`.
Handle `N == 0` first to avoid dividing by zero.

## c15_e05_function_pointers_vs_closures
`convert`: search the table (`iter().find(...)`) and call the function you found.

`make_scaler` must return a closure that *captures* `factor`: `move |x| x * factor`.
`pipeline` needs to store closures of different types, so it takes and returns
`Box<dyn Fn>`. The returned closure owns `steps` (so, `move`).

`pipeline`: `Box::new(move |x| steps.iter().fold(x, |acc, step| step(acc)))`.
`feed`: take `mut f: impl FnMut(f64)` and call `f(v)` in a loop.

## c15_e06_never_type_and_aliases
`parse_all`: `collect` can build a `Result<Vec<_>, _>` from an iterator of
`Result`s and stops at the first `Err` (chapter 10).

`into_ok`: an `Err(e)` where `e: Infallible` can never exist. How many arms
does a `match` on an enum with zero variants need?

`Err(never) => match never {}`. For `updates_above`, box an iterator:
`Box::new(self.values.iter().filter(move |(_, v)| **v > threshold).map(|(k, v)| (k.as_str(), *v)))`.
`doubled`: a `match` whose `None` arm is `return Err(ParseError::Empty)`.

## c15_e07_box_str_rc_str
`freeze`: `String` has a method that turns it into `Box<str>`.

For the interner, keep a `HashMap<Arc<str>, u32>` and a `Vec<Arc<str>>`. Look
up with `self.ids.get(path)`: `Arc<str>` implements `Borrow<str>`, so a
`&str` key works. Hand out `Arc::clone(&...)` so every caller shares one allocation.

`intern`: if found, return `(id, Arc::clone(&self.paths[id as usize]))`.
Otherwise `let shared: Arc<str> = Arc::from(path);`, push a clone into
`paths`, insert a clone into `ids`, and return it. `fan_out` maps
`0..subscribers` to `Notification { path: Arc::clone(path), value }`.

## c15_e08_build_calibration_pipeline
`Pipeline::new`: `Pipeline { stages }`. `run` (both structs): `self.stages
.iter().fold(input, |value, stage| stage(value))` — `fold`'s accumulator
*is* the running value, and `stage` is callable whether it's a `fn` item or
a boxed closure. `DynPipeline::add_stage`: `self.stages.push(Box::new(stage))`.

## c15_e09_review_layout_bugs
Three independent bugs:
- `average`: divides by a literal `10.0` instead of `N as f64`. `N` is in
  scope as a plain value inside the function body.
- `Window::push`: the guard `if self.len <= N` lets `len` reach `N + 1`.
  Walk `push` by hand for a `Window<3>` fed 4 values and watch `len` grow
  past 3. It should be `<`.
- `Interner::intern`: it builds a brand-new `Rc::from(text)` every call
  and never looks in `self.cache` first, so nothing is ever deduplicated.
  Check `self.cache.get(text)` and return a clone of that before falling
  through to allocating a new one.
