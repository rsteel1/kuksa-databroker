# Chapter 27 hints

## c27_e01_counting_allocator
`bump`: `let _ = ALLOCS.try_with(|count| count.set(count.get() + 1));`.
`try_with` returns an error instead of panicking when the thread-local is
already destroyed (during thread exit). An allocator must never panic,
because the panic machinery itself allocates.

Put `bump();` at the top of `alloc`, `alloc_zeroed` and `realloc`. Leave
`dealloc` alone.

`count_allocs`: read `ALLOCS.with(Cell::get)` before and after calling
`f()`, and return `(result, after - before)`. Counting per thread is what
keeps the numbers exact while `cargo test` runs other tests in parallel.

## c27_e02_avoid_allocations
`join_path`: the shipped code allocates one `String` per segment, and every
`out + "."` may reallocate. Compute the final length first (sum of segment
lengths plus the dots), use `String::with_capacity(len)`, then `push_str`
and `push('.')`.

`normalize`: only allocate when there's something to replace.
`if path.contains('/') { Cow::Owned(path.replace('/', ".")) } else { Cow::Borrowed(path) }`.

`render_updates`: `buf.clear()` keeps the capacity. Then
`writeln!(buf, "{path}={value}")` writes straight into it (you need
`use std::fmt::Write as _;` in scope). `format!` would allocate a fresh
`String` per line.

`count_matching`: `to_lowercase()` allocates twice per path. Compare bytes
instead: check the length first, then
`path.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())`.

## c27_e03_iterators_are_zero_cost
`sum_of_squares`: `values.iter().map(|v| v * v).sum()`.

`moving_average`: `values.windows(window)` yields overlapping slices. Map
each to `sum / window as f64` and `collect`. `windows` knows its exact
length, so `collect` allocates once. Handle `window == 0` first:
`windows(0)` panics.

`checksum`: `bytes.chunks_exact(4)` yields full 4-byte chunks, and
`.remainder()` gives the tail. Read the remainder before consuming the
iterator. Copy the tail into a zeroed `[u8; 4]` and XOR it in.

`ids_above`: after `filter`, the iterator no longer knows its length, so
`collect` grows the `Vec` several times. Count first, then
`Vec::with_capacity(count)` and `extend` with the same filter.

## c27_e04_data_layout_for_speed
`from_aos`: three `iter().map(...).collect()` calls, one per field.

`sum_valid`: `self.values.iter().zip(&self.valid)`, filter on the bool,
map to the value, and sum. The loop only touches the two arrays it needs.

`InlineVec::push` has three cases. Inline with room: write `items[len]`
and increment. Inline and full: build a `Vec::with_capacity(...)`, copy the
`len` inline items, push the new one, and replace `*self` with
`InlineVec::Heap(vec)`. Heap: just push. A `match self { ... }` with a guard
`if *len < N` separates the first two.

`as_slice`: `&items[..*len]` or `&heap[..]`.

## c27_e05_hashing_and_collections
`IdHasher::write_u64`: `self.hash = (self.hash.rotate_left(5) ^ n).wrapping_mul(SEED);`.
`write` loops over the bytes and calls `write_u64(u64::from(byte))`.
`write_i32` converts with `u64::from(n as u32)`, and `finish` returns
`self.hash`.

`index_paths`: `HashMap::with_capacity(paths.len())`, then insert
`(*path, i)`. The keys are `&'a str`, so no `String`s are created.

`lookup`: `map.get(path).copied()`. `HashMap<String, _>::get` accepts
`&str` because `String: Borrow<str>`.

## c27_e06_checkpoint_part4
Strategy, not code:

- One way to design it: the cache holds
  `RwLock<Arc<HashMap<Arc<str>, f64>>>` plus an `AtomicU64` version, and a
  `Snapshot` is just a clone of the inner `Arc`.
- Readers lock only to clone the `Arc`, then read with no lock.
- Writers copy-on-write: clone the current map, apply the updates, and swap
  in a new `Arc`.
- Why `Arc<str>` keys? Cloning the map then clones keys by bumping a
  refcount, not by copying text. One allocation for the new table and one
  for the new `Arc` fits the budget of 2.
- Update existing entries through `get_mut(&str)`; only unknown paths need
  `Arc::from(path)`.
- Bump the version with `Release` and read it with `Acquire` (chapter 25).
