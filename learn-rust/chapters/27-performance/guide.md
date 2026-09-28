# 27 — Performance & profiling

> **Builds on:** chapters 05 (collections), 10 (iterators), 12 (smart pointers), 15 (layout), 26 (unsafe, for the counting allocator).

**Why this matters for the databroker, and beyond.** The databroker sits on
the hot path of a vehicle: every published value is validated, stored and
fanned out to subscribers, often hundreds of times a second, on a small
embedded CPU. Rust makes fast code *possible*, not automatic. This chapter
teaches the habits that separate fast Rust from merely safe Rust:
**measure first**, remove hidden allocations, lay out data for the cache,
choose collections deliberately, and know when to stop. All the tools shown
here run in the course's Linux devcontainer.

---

## Measure first: counting allocations

The first rule of performance work: **don't guess.** Intuition about what's
slow is wrong more often than right. Measure, change one thing, and measure
again.

Heap allocation is Rust's most common hidden cost. `String`, `Vec`, `Box`,
`HashMap`, `format!`, `to_string()`, `clone()` of owned data and
`collect()` can all call the allocator, which may take a lock, touch cold
memory and fragment the heap.

Rust lets you replace the allocator for a whole program:

```rust
#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;   // any type implementing GlobalAlloc
```

`databroker/src/main.rs:14-16` does exactly this to switch to jemalloc
when built with `--features jemalloc`. In e01 you wrap the system allocator
to *count* calls, and every later exercise uses that counter to turn "this
should allocate less" into an assertion a test can check. The counter is
per thread (`thread_local!` with `const` init, which never allocates),
because `cargo test` runs tests in parallel.

Tools for real programs on Linux:

| Question | Tool |
|---|---|
| "Where is the time going?" | `cargo flamegraph` (uses `perf`; needs `linux-perf` and `echo 1 \| sudo tee /proc/sys/kernel/perf_event_paranoid`), or `samply record` |
| "Is version B faster than A?" | **criterion** benchmarks (statistics, warm-up, outlier detection) |
| "How much does it allocate, and where?" | `dhat` (the dhat-rs crate), `heaptrack`, `valgrind --tool=massif` |
| "Cache misses?" | `perf stat -e cache-misses`, `valgrind --tool=cachegrind` |
| "What machine code did this become?" | `cargo asm` (cargo-show-asm), or godbolt.org |
| "Is the whole binary faster?" | `hyperfine` for end-to-end timing |

Benchmarks need `std::hint::black_box(x)`. Otherwise the optimiser may
compute the result at compile time or delete the work, and you end up
benchmarking nothing.

Always measure **release** builds. Debug builds are 10–100× slower and
distort everything. The databroker's release profile (`Cargo.toml:43-48`)
uses `lto = true` (whole-program optimisation), `opt-level = "s"` (optimise
for size, a sensible choice on embedded targets), `codegen-units = 1` (better
optimisation, slower build) and `strip = true`. `release-with-debug`
(`Cargo.toml:50-53`) keeps symbols so profilers can name functions.

## Fewer allocations: the everyday toolkit

Four techniques cover most real wins:

| Technique | Before | After |
|---|---|---|
| **Pre-size** | `Vec::new()` then 1000 pushes: about 11 reallocations | `Vec::with_capacity(1000)`: 1 |
| **Borrow** | `fn f(s: String)`, `.to_string()` everywhere | `fn f(s: &str)`, keys as `&'a str` |
| **Allocate only when needed** | `path.replace('/', ".")` always allocates | `Cow::Borrowed` when nothing changes |
| **Reuse** | `format!` per line into a fresh `String` | `buf.clear()` + `write!(buf, ...)` |

The databroker does the first one where it knows sizes up front:
`HashMap::with_capacity(size)` at `grpc/kuksa_val_v2/val.rs:403` and
`IndexMap::with_capacity(vss_signals.len())` at `broker.rs:2324`.

`clear()` keeps a collection's capacity: that's what makes buffer reuse
work. `shrink_to_fit()` gives it back, when memory matters more than speed.

Watch out for these hidden allocations: `to_lowercase()` in a comparison
(use `eq_ignore_ascii_case`), `format!` just to compare strings,
`collect::<Vec<_>>()` only to iterate once more (chain the iterators
instead), and cloning `String`s out of a lock when an `Arc<str>` would
do (chapter 15).

## Zero-cost iterators and bounds checks

"Zero-cost abstraction" means you couldn't write it faster by hand.
Iterator chains like `values.iter().map(|v| v * v).sum()` compile to the
same tight loop as manual indexing, and are often *faster*, because
`values[i]` needs a bounds check on every access unless the optimiser can
prove it away, while an iterator over a slice can't go out of bounds by
construction.

Useful adaptors for performance:

- `chunks_exact(n)` / `.remainder()`: fixed-size blocks without per-item
  checks (e03's checksum), which the compiler can vectorise (SIMD).
- `windows(n)`: overlapping views, no copying.
- `zip`: walk two slices in lock-step without indexing either.
- Exact-size iterators (`map` over a slice, `windows`, `chunks`): `collect`
  allocates once. `filter` loses the size hint, so pre-count or use
  `with_capacity`.

To check, look at the assembly:

```sh
cargo install cargo-show-asm
cargo asm -p solutions --test c27_e03_iterators_are_zero_cost --rust checksum
```

Look for `panic_bounds_check` calls inside the loop, and for vector
instructions (SSE/AVX on x86_64, NEON on aarch64). Unit tests never
assert timing: it's too noisy. They assert correctness and allocation
counts, and benchmarks answer "how fast".

## Data layout: the cache is the real bottleneck

A modern CPU reads memory in **64-byte cache lines** and is ~100× faster
at hitting L1 cache than going to RAM. Data layout often matters more than
instruction count.

- **Padding.** Fields are aligned to their size, so a `bool` next to an
  `f64` wastes 7 bytes. Rust reorders fields to minimise padding (a
  `{bool, f64, u32}` struct is 16 bytes); `#[repr(C)]` keeps declaration
  order (24 bytes). On 64-bit Linux (x86_64 and aarch64) pointers and
  `usize` are 8 bytes, a `Vec`/`String` is 24 bytes, and `&[T]`/`&str` is 16.
- **Array of structs vs struct of arrays.** `Vec<Signal>` puts all of a
  signal's fields together, which is great when you use all of them.
  `SignalsSoA { ids: Vec<u32>, values: Vec<f64>, valid: Vec<bool> }` puts
  each field together, which is great for "sum every value", because the loop
  streams through exactly the bytes it needs and vectorises well.
- **Contiguous beats linked.** `Vec` and `VecDeque` beat linked lists and
  trees of `Box`es for almost everything, because following pointers
  defeats the prefetcher.
- **Inline small collections.** Most signal paths have 3–6 segments. A
  `SmallVec<[T; N]>` stores up to N items inline (no heap at all) and
  spills only for big cases (e04 builds a simple version).

The databroker's `Entry` (`broker.rs:113`) is a classic array-of-structs
record: two `Datapoint`s, an optional target, and `Metadata` with several
`String`s. That's the right trade-off for "look up one signal and read
everything about it", which is its main access pattern. A struct of arrays
would only pay off for whole-database scans.

## Hashing and choosing a collection

`HashMap`'s default hasher, SipHash-1-3, is designed so attackers can't
make many keys collide (HashDoS). That's the right default for any key that
comes from outside (paths from gRPC clients, JWT claims). For small keys
*you* generate, such as signal ids, a simpler hash like FxHash, foldhash or
ahash is several times faster. Plug one in with a `BuildHasher`:

```rust
type IdMap<V> = HashMap<i32, V, BuildHasherDefault<IdHasher>>;
```

hashbrown, the table behind std's `HashMap`, uses foldhash as its own
default (`hashbrown-0.15.5/Cargo.toml:60`). std keeps SipHash for safety.

Other costs to know:

- **Rehashing on growth** moves every entry: use `with_capacity`.
- **Lookups by borrowed key:** `HashMap<String, V>::get(&str)` works
  without allocating because `String: Borrow<str>`. Never build a `String`
  just to look something up.
- **Choosing a collection:**

| Need | Use |
|---|---|
| Lookup by key, order irrelevant | `HashMap` |
| Sorted keys, range queries (`"Vehicle.Cabin."..`) | `BTreeMap` |
| Insertion order kept, O(1) lookup | `IndexMap` (the databroker's `get_values_broker`, chapter 05) |
| Small N (< ~16), or mostly iteration | a `Vec` of pairs with a linear search often wins |
| Queue | `VecDeque` |
| Membership | `HashSet`/`BTreeSet` |

The databroker's `Database` (`broker.rs:128-132`) resolves requests in two
hops: `path_to_id: HashMap<String, i32>`, then `entries: HashMap<i32, Entry>`.
Both use the default hasher. For `path_to_id`, whose keys come from clients,
that's the correct choice.

## When not to optimise

- Profile first: optimise the 5% of code that takes 95% of the time.
- Prefer clarity until a measurement says otherwise.
  `broker.rs:2254-2262` (`join_all` over providers) is a performance
  change driven by a real problem: latency adding up across providers. It
  isn't micro-tuning.
- Algorithmic wins (O(n²) → O(n log n), avoiding a lock, batching I/O)
  dwarf micro-optimisations.
- Keep optimised code tested. The allocation-budget tests in this chapter
  are a pattern you can use in real projects to stop regressions.

## Checkpoint

`c27_e06_checkpoint_part4` asks for a **read-mostly signal cache**:
consistent snapshots that are free to take and read, allocation budgets for
publishing, and no `unsafe` in the API. It draws on:

- `Arc`, `RwLock` and atomics with `Acquire`/`Release` (chapters 12 and 25);
- `Arc<str>` and borrowed-key lookups (chapters 14 and 15);
- allocation counting and pre-sizing (this chapter);
- thinking about which operation is hot (reads) and which may be slower (publishing).

No design is given: pick your fields, then let the tests tell you whether
the budgets hold. In the wild, the `arc-swap` crate makes the snapshot swap
itself lock-free.

## Mental model

| Idea | Analogy |
|---|---|
| Allocation | A trip to the warehouse: fine occasionally, ruinous in a loop |
| `with_capacity` | Renting a big enough truck before moving |
| `Cow` | Borrow the book; buy a copy only if you need to write in it |
| Cache line | The CPU fetches a whole shelf, not one book |
| Struct of arrays | Shelving all volumes of the same series together |
| Profiler | A speed camera: shows where time goes instead of where you think it goes |

The compiler guarantees that **iterators and generics cost nothing extra**
(monomorphisation plus inlining), and that the safe versions of indexing and
parsing are as fast as unsafe ones in the common case. It doesn't
guarantee that your algorithm, allocation pattern or data layout is good.
That's your job.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Iterators are slower than index loops." | They compile to the same or better code, often without bounds checks. |
| "Rust is fast, so I don't need to measure." | Allocation and layout mistakes cost as much in Rust as anywhere else. |
| "Debug build timings tell me something." | Only release (`--release`) numbers mean anything. |
| "`unsafe` makes it faster." | Rarely; measure first. Safe `from_le_bytes`, `get` and iterators are usually just as fast. |
| "`HashMap` is always the fastest lookup." | Small `Vec` scans, `BTreeMap` for ranges, and custom hashers each win in their niche. |
| "`clone()` is always expensive." | Cloning an `Arc` is a counter increment. Cloning a `String` allocates. Know which you're doing. |

## Smells and idioms

- `.to_string()` or `.clone()` in a loop to satisfy the borrow checker:
  restructure to borrow (chapter 02).
- `collect()` followed by `.iter()` on the result: chain the iterators instead.
- `format!` for comparisons, and `to_lowercase()` for case-insensitive checks.
- Growing a `Vec`/`String`/`HashMap` whose final size you already know.
- Micro-optimising code that a profile shows is cold.
- `Box<dyn Fn>` or `Box<dyn Trait>` on a hot path where generics would
  inline (chapter 09). Conversely, generics everywhere can bloat the binary,
  which matters with `opt-level = "s"`.
- **In the databroker:** the per-request `with_capacity` calls are good
  practice. The notification path clones each changed `Datapoint` (and its
  `String` path) per subscriber (`broker.rs:957-961`). That's simple and
  correct, and a candidate for `Arc<str>` paths if profiling ever shows
  notification cost dominating.

## In the databroker and in the wild

- `databroker/src/main.rs:14-16`: `#[global_allocator]` with jemalloc.
- `Cargo.toml:43-53` (repository root): release and release-with-debug
  profiles.
- `databroker/src/grpc/kuksa_val_v2/val.rs:403` and `broker.rs:2324`:
  pre-sized maps.
- `databroker/src/broker.rs:113` `Entry` and `:128-132` `Database`: layout
  and two-hop lookup.
- `databroker/src/broker.rs:2254-2262`: concurrency as the optimisation
  (latency = the slowest provider, not the sum).
- `smallvec-1.16.2/src/lib.rs:833` `SmallVec`: the production version of
  e04's `InlineVec`.
- `hashbrown-0.15.5/Cargo.toml:60`: `default-hasher = ["dep:foldhash"]`.
- `bytes-1.12.1/src/bytes_mut.rs:328`: `split_off` shares one buffer
  instead of copying, which is how tonic avoids copying message bodies.

(Registry paths are under `~/.cargo/registry/src/index.crates.io-*/`.)

## Exercises

| Exercise | Goal |
|---|---|
| c27_e01_counting_allocator | Write a `GlobalAlloc` that counts allocations per thread |
| c27_e02_avoid_allocations | Meet allocation budgets with `with_capacity`, `Cow`, buffer reuse and byte comparisons |
| c27_e03_iterators_are_zero_cost | Iterator-only implementations with `windows`, `chunks_exact` and exact-size collects |
| c27_e04_data_layout_for_speed | Struct of arrays, padding, and an inline-then-heap vector |
| c27_e05_hashing_and_collections | A custom hasher, pre-sized maps with borrowed keys, allocation-free lookups |
| c27_e06_checkpoint_part4 | Design a copy-on-write snapshot cache under allocation budgets |
| c27_e07_review_hot_loop_costs | Review: 2 planted bugs — an accidentally O(n^2) dedup, and a clone of the whole buffer per update |

## Further reading

- *The Rust Performance Book* (Nicholas Nethercote): <https://nnethercote.github.io/perf-book/>
- criterion.rs user guide: <https://bheisler.github.io/criterion.rs/book/>
- `std::alloc::GlobalAlloc` docs: <https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html>
- cargo-flamegraph: <https://github.com/flamegraph-rs/flamegraph>
- cargo-show-asm: <https://github.com/pacak/cargo-show-asm>
- *Rust for Rustaceans*, ch. 2 "Types" (layout) and ch. 6 "Testing" (benchmarks)

## Check yourself

1. Why is the allocation counter thread-local instead of one global atomic?
2. Name four techniques that remove allocations, and give an example of each.
3. Why can an iterator chain be faster than an index loop over the same slice?
4. When does a struct of arrays beat an array of structs, and when not?
5. Why does std's `HashMap` use SipHash, and when is it fine to swap it out?
6. How do you look up a `HashMap<String, V>` with a `&str` without allocating?
7. Why must benchmarks use `black_box` and release builds?

<details><summary>Answers</summary>

1. `cargo test` runs tests on several threads at once. A global counter
   would include other tests' allocations and make the numbers flaky.
2. Pre-size (`Vec::with_capacity`), borrow (`&str` keys instead of
   `String`), allocate on demand (`Cow::Borrowed` when nothing changes),
   and reuse buffers (`clear()` + `write!`).
3. The iterator can't go out of bounds, so there are no per-element bounds
   checks. That also helps the compiler vectorise the loop.
4. It wins when a hot loop reads a few fields of many elements (better cache
   use, SIMD). It loses when you usually need all fields of one element,
   like the databroker's single-signal lookups.
5. SipHash resists hash-flooding attacks on keys an attacker controls.
   Swap it for keys you generate yourself (internal ids), never for
   client-supplied strings.
6. `map.get(path)` with `path: &str` works directly because `String: Borrow<str>`.
7. Without `black_box`, the optimiser can precompute or delete the work.
   Without release mode, timings reflect unoptimised code that nobody ships.
</details>
