# 15 — Advanced types & memory layout

> **Builds on:** chapters 03 (newtypes), 09 (trait objects), 10 (closures), 12 (Box, Arc).

**Why this matters for the databroker (and every other codebase).** Until now
a type was "what a value is". This chapter is about what a value *occupies*:
how many bytes, where the pointers are, which types can only live behind a
pointer, and which types cost nothing at all. You need this to read
signatures like `Pin<Box<dyn Stream<Item = ...> + Send + Sync + 'static>>`
(`databroker/src/grpc/kuksa_val_v2/val.rs:369`), to understand why
`Option<Box<T>>` is free while `Option<u32>` isn't, and to judge whether
cloning a signal path 10,000 times per second is cheap or expensive.
Performance-minded crates (`bytes`, `indexmap`, `tokio`) are built out of
exactly these ideas.

All numbers in this chapter assume a **64-bit target** (x86_64 or aarch64,
Linux or otherwise): `usize` and pointers are 8 bytes. The Rust ABI rules
used here are the same on both architectures.

---

## Size, alignment and padding

Every type has two numbers:

- **size**: how many bytes a value takes (`std::mem::size_of::<T>()`);
- **alignment**: the address of a value must be a multiple of this
  (`std::mem::align_of::<T>()`). A `u64` has alignment 8, a `u32` 4, a `u8` 1.

A struct's alignment is the largest alignment of its fields, and its size
is rounded up to a multiple of that alignment. When a field needs a
stricter alignment than where the previous field ended, the compiler inserts
**padding** bytes.

```text
#[repr(C)] struct Bad  { a: u8, b: u64, c: u8, d: u32 }

offset 0        8                16       20       24
       ┌─┬───────┬────────────────┬─┬───┬────┬──────┐
       │a│ pad 7 │       b        │c│pad│ d  │ pad 4│   size 24, only 14 used
       └─┴───────┴────────────────┴─┴───┴────┴──────┘

#[repr(C)] struct Good { b: u64, d: u32, a: u8, c: u8 }

offset 0                8        12 13 14 16
       ┌────────────────┬────────┬─┬─┬───┐
       │       b        │   d    │a│c│pad│              size 16
       └────────────────┴────────┴─┴─┴───┘
```

Two representations matter:

| Representation | Field order | Use it when |
|---|---|---|
| default (`repr(Rust)`) | **unspecified**: the compiler may reorder to cut padding (current rustc sorts by alignment) | always, unless you need a fixed layout |
| `#[repr(C)]` | exactly as written, C rules | sharing memory with C (chapter 26), wire formats, memory-mapped hardware |

With `repr(C)` good field order is *your* job: largest alignment first.
`std::mem::offset_of!(Type, field)` tells you where a field starts.

## Niches: Option for free

`Option<T>` must record whether it's `Some` or `None`. Naively that's a tag
byte plus padding, so `Option<u32>` is 8 bytes. But many types have **bit
patterns that can never occur**, called *niches*:

| Type | Invalid pattern | `size_of::<Option<T>>()` |
|---|---|---|
| `&T`, `&mut T`, `Box<T>`, `Arc<T>`, `NonNull<T>` | null | same as `T` (8) |
| `NonZeroU32` | 0 | 4 |
| `bool` | 2..=255 | 1 |
| `char` | values above `0x10FFFF` | 4 |
| enum with fewer than 256 variants | unused discriminants | same as the enum |
| `u32` | none | 8 |

The compiler uses a niche to mean `None`, so the tag disappears. That's why
Rust can afford "no null pointers, use `Option`": `Option<&T>` compiles to
exactly a nullable pointer.

Design lesson: if 0 is never a valid id, say so in the type (`NonZeroU32`),
and you get cheaper `Option<Id>` *and* one less invalid state. The
databroker's `SignalId` (`databroker/src/types.rs:491`) wraps a plain `i32`,
and ids start at 0 (`databroker/src/broker.rs:1326`), so there is no niche.
That's a fine choice for them; exercise `c15_e02` shows the alternative.

## Dynamically sized types and fat pointers

Some types have no size known at compile time:

- `str`: some number of UTF-8 bytes;
- `[T]`: some number of `T`s;
- `dyn Trait`: some concrete type implementing `Trait`.

These are **dynamically sized types (DSTs)**. You can't have a local
variable of type `str`, because the compiler must know how much stack space
to reserve. You always hold them behind a pointer, and that pointer carries
the missing information, which makes it **fat** (two words):

```text
&u64        [ ptr ]                         thin: 1 word
&str        [ ptr | len ]                   len in bytes
&[T]        [ ptr | len ]                   len in elements
&dyn Trait  [ ptr | vtable ]                vtable = table of method pointers + size + drop
String      [ ptr | len | capacity ]        an owned, growable buffer (3 words)
```

Every generic parameter has an implicit `Sized` bound: `fn f<T>(x: &T)`
really means `fn f<T: Sized>(x: &T)`. When you only touch `T` through a
reference, write `T: ?Sized` ("may be unsized") and the function also
accepts `str`, `[T]` and `dyn Trait`. The standard library does this
everywhere: `impl<T: ?Sized> Deref for Box<T>`, `fn size_of_val<T: ?Sized>`.

Where you meet this in the databroker: `get_entry_by_path(&self, path: impl
AsRef<str>)` (`AsRef<str>` targets the unsized `str`), and
`Arc<dyn SignalProvider>` (`databroker/src/broker.rs:235`), a fat pointer
whose vtable is how `provider.get_signals_values_from_provider(..)` finds
the right code at runtime.

## Const generics

`[T; N]` has always been generic over a number. Since Rust 1.51 your own
types and functions can be too:

```rust
struct RingBuffer<T, const N: usize> {
    slots: [Option<T>; N],
    head: usize,
    len: usize,
}

fn average<const N: usize>(samples: [f64; N]) -> f64 { ... }
```

- `RingBuffer<u8, 4>` and `RingBuffer<u8, 8>` are **different types**.
- `N` costs no memory: it isn't stored, it's part of the type.
- Constant expressions like `N == 0` are evaluated at compile time, so
  whole branches disappear.
- `std::array::from_fn(|i| ...)` builds a `[T; N]` without needing `T: Copy`.

Use const generics for fixed capacities, matrix dimensions, buffer sizes
known at compile time, and "exactly N items" APIs. Don't use them when the
number comes from configuration at runtime: the databroker's subscription
`buffer_size` comes from the client, so `broadcast::channel(capacity)`
(`databroker/src/broker.rs:1630`) takes a runtime `usize`.

## Function pointers, closures and Box<dyn Fn>

| Type | Size | Captures data? | Dispatch | Typical use |
|---|---|---|---|---|
| `fn(f64) -> f64` | 1 word | no | indirect call | tables of functions, C callbacks |
| `impl Fn(f64) -> f64` (argument or return) | size of the captures | yes | static, inlined | most closure parameters |
| `Box<dyn Fn(f64) -> f64>` | 2 words + heap | yes | vtable | storing *different* closures together |
| `&dyn Fn(f64) -> f64` | 2 words | yes | vtable | borrowing one without generics |

Rules to remember:

- Every closure has its **own anonymous type**. Two closures with identical
  code are still different types, so a `Vec` of them needs `Box<dyn Fn>`.
- A closure that **captures nothing** coerces to a `fn` pointer. That's how
  a `const` table of conversions can contain closure literals.
- `Fn` / `FnMut` / `FnOnce` (chapter 10) describe *how* the closure uses its
  captures. Choose the least demanding bound your code needs: `FnOnce` if
  you call it once, `FnMut` if it needs to mutate, `Fn` if it's shared.

The databroker takes closures by `impl Fn*` (static dispatch):
`with_read_lock(&self, f: impl FnOnce(&DatabaseReadAccess) -> T)`
(`databroker/src/broker.rs:1411`) and `filter_map_entries`
(`databroker/src/broker.rs:1514`).

## The never type and type aliases

**`!`, the never type**, is the type of expressions that never finish
normally: `return`, `break`, `continue`, `panic!()`, `loop {}` with no
break, `std::process::exit`. Because such an expression never produces a
value, `!` coerces to *any* type, which is why this compiles:

```rust
let value: i64 = match map.get(path) {
    Some(v) => *v,
    None => return Err(NotFound), // type `!`, fits where `i64` is needed
};
```

**`std::convert::Infallible`** is an enum with no variants, the stable
stand-in for `!` in APIs. `Result<T, Infallible>` can only be `Ok`. The
standard library uses it wherever an operation can't fail:
`<String as FromStr>::Err`, and `TryFrom` for every type that has a `From`.
Matching on it takes no arms: `match never {}`.

**`type` aliases** give a *name* to an existing type. They don't create a
new type (for that, use a newtype, chapter 03), so the alias and the original
are fully interchangeable. Use them to name long types once:

```rust
// databroker/src/grpc/kuksa_val_v2/val.rs:369
type SubscribeStream = Pin<Box<dyn Stream<Item = Result<proto::SubscribeResponse, tonic::Status>>
                                + Send + Sync + 'static>>;
// databroker/src/broker.rs:222
type ActuationSubscriptionSnapshot = (Vec<i32>, Arc<dyn ActuationProvider + Send + Sync + 'static>, Permissions);
```

Aliases can have generic and lifetime parameters: `type ParseResult<T> =
Result<T, ParseError>;` is how `std::io::Result<T>` and `fmt::Result` are
defined. Clippy's `type_complexity` lint tells you when a type deserves a
name.

## Box<str>, Arc<str> and interning

`String` is 3 words (pointer, length, capacity) and growable. If text never
changes after it's created (VSS paths, metric names, config keys), you have
better options:

| Type | Words | Clone cost | Mutable? |
|---|---|---|---|
| `String` | 3 | allocate + copy all bytes | yes |
| `Box<str>` | 2 | allocate + copy | no |
| `Arc<str>` | 2 | atomic increment | no |
| `&'static str` | 2 | free | no |
| interned id (`u32`) | ½ | free | no; look up to read |

`Arc<str>` and `Box<str>` are built with `Arc::from(s: &str)` /
`String::into_boxed_str`. Because `Arc<str>: Borrow<str>`, a
`HashMap<Arc<str>, V>` can be queried with a plain `&str`, with no
allocation just to ask.

**Interning** stores each distinct string once and hands out shared handles
or small ids. It pays off when the same strings are cloned or compared very
often.

In the databroker, every change notification clones the signal path for
every subscriber: `update.path = Some(entry.metadata.path.clone())`
(`databroker/src/broker.rs:957` and `:1021`). With `Arc<str>` paths each of
those would be a reference-count increment instead of an allocation. That's
a real (if modest) optimisation opportunity: measure first (chapter 27).

## Mental model

| Idea | Analogy |
|---|---|
| alignment & padding | parking spaces for trucks must start at a truck-sized line; small cars leave gaps |
| `repr(C)` | "park in exactly the order you arrived" |
| niche | using an impossible house number (0) to mean "no house" instead of a separate flag |
| fat pointer | an address plus a note: "12 rooms" (length) or "see the floor plan" (vtable) |
| `?Sized` | "I only need your address, not your dimensions" |
| const generic | the size is printed on the box at the factory |
| `Box<dyn Fn>` | a remote control for some device; you don't know which until you press a button |
| `Arc<str>` | one printed poster, many people pointing at it |

**Guarantees:** sizes and layouts are fixed at compile time, a fat pointer
always carries what's needed to use its target safely (you can't index past
a slice's length), and `repr(Rust)`'s freedom to reorder never changes
behaviour, only memory use.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Fields are stored in the order I wrote them." | Only with `#[repr(C)]` (or `repr(packed)`/`repr(transparent)`). |
| "`Option<T>` always costs an extra byte." | Not when `T` has a niche: `Option<&T>`, `Option<Box<T>>`, `Option<NonZeroU32>` are free. |
| "`&str` is a pointer." | It's a pointer *and* a length: two words. |
| "`str` is like `String`." | `str` is the unsized bytes; `String` owns a heap buffer of them. You never hold a bare `str`. |
| "`?Sized` makes a type unsized." | It *allows* unsized types; sized ones still work. |
| "A `type` alias is a new type." | It's a second name. Use a newtype (chapter 03) to create a distinct type. |
| "Closures with the same body have the same type." | Every closure literal has its own type. E0308 "no two closures have the same type". |
| "`Arc<String>` and `Arc<str>` are the same." | `Arc<String>` is a pointer to a pointer (two allocations); `Arc<str>` stores the bytes inline. |

## Smells and idioms

- **Padding in `repr(C)` structs**: order fields by descending alignment.
  In default structs, don't bother: the compiler does it.
- **`Option<u32>` for ids where 0 is impossible**: use `NonZeroU32` for a
  free niche and a clearer contract.
- **`fn f<T: Trait>(x: &T)` that should accept `str`/slices/trait objects**:
  add `?Sized`.
- **`Vec<Box<dyn Fn>>` where every closure is actually the same function**:
  a `fn` pointer table is simpler and needs no allocation.
- **`&String` / `&Vec<T>` parameters**: take `&str` / `&[T]`, which also
  accept literals and arrays (clippy `ptr_arg`).
- **Cloning immutable `String`s on a hot path**: consider `Arc<str>` or
  interning, after measuring. The databroker clones `metadata.path` per
  notification; it's simple and correct, and fast enough for its load.
- **Deeply nested types repeated in several signatures**: name them with
  `type` (the databroker does this well with `SubscribeStream`).

## In the databroker and in the wild

| Where | What to notice |
|---|---|
| `databroker/src/types.rs:491` | `SignalId { id: i32 }`: a newtype with no niche; ids start at 0 (`broker.rs:1326`) |
| `databroker/src/broker.rs:218` | `Arc<dyn ActuationProvider + Send + Sync + 'static>`: fat pointer (data + vtable) behind a refcount |
| `databroker/src/broker.rs:222` | `type ActuationSubscriptionSnapshot = (...)`: a tuple alias |
| `databroker/src/grpc/kuksa_val_v2/val.rs:369` | `type SubscribeStream = Pin<Box<dyn Stream<...> + Send + Sync + 'static>>` |
| `databroker/src/broker.rs:957` | `entry.metadata.path.clone()` per notification: the `Arc<str>` opportunity |
| `databroker/src/broker.rs:1411` | `f: impl FnOnce(&DatabaseReadAccess) -> T`: static-dispatch closure parameter |
| `~/.cargo/registry/src/index.crates.io-*/bytes-1.12.1/src/bytes.rs:101` | `Bytes` is 4 words: `ptr`, `len`, `data`, and `vtable: &'static Vtable`, a **hand-written vtable of `fn` pointers** (`:113`). This is how one type can be backed by a `Vec`, a static slice or shared memory |
| `~/.cargo/registry/src/index.crates.io-*/indexmap-2.14.2/src/inner.rs:29` | `Core { indices, entries }`: an index table pointing into a dense `Vec` by position, the same idea as `c16_e06` |
| `~/.cargo/registry/src/index.crates.io-*/serde_json-1.0.151/src/value/mod.rs:116` | `pub enum Value`: an enum of `String`, `Vec`, map, etc. Check `size_of::<serde_json::Value>()` yourself |

(The `index.crates.io-*` directory name differs per machine; the `*` matches it.)

## Exercises

| Exercise | Goal |
|---|---|
| `c15_e01_size_and_layout` | Reorder a `repr(C)` header from 24 to 16 bytes; compute padding |
| `c15_e02_niche_optimization` | Make `Option<SignalId>` free with `NonZeroU32` |
| `c15_e03_dsts_and_fat_pointers` | Relax a bound with `?Sized` so `str`, slices and `dyn` work |
| `c15_e04_const_generics` | A fixed-capacity ring buffer `RingBuffer<T, N>` |
| `c15_e05_function_pointers_vs_closures` | `fn` tables, returning closures, a boxed pipeline |
| `c15_e06_never_type_and_aliases` | `!`, `Infallible`, aliases for results and boxed iterators |
| `c15_e07_box_str_rc_str` | Intern signal paths with `Arc<str>`; share them in a fan-out |
| `c15_e08_build_calibration_pipeline` | build: a const-generic `fn`-pointer pipeline plus a `Box<dyn Fn>` one |
| `c15_e09_review_layout_bugs` | review: find and fix 3 bugs — hardcoded divisor, off-by-one window length, interner that never reuses |

## Further reading

- The Rust Reference, "Type layout": <https://doc.rust-lang.org/reference/type-layout.html>
- The Rustonomicon, "Data Representation in Rust" and "Exotically Sized Types": <https://doc.rust-lang.org/nomicon/>
- The Book, ch. 20 "Advanced Features" (Advanced Types; Advanced Functions and Closures): <https://doc.rust-lang.org/book/ch20-00-advanced-features.html>
- `std::mem` docs (`size_of`, `align_of`, `offset_of!`): <https://doc.rust-lang.org/std/mem/>
- *Rust for Rustaceans*, ch. 2 "Types"
- Const generics MVP announcement: <https://blog.rust-lang.org/2021/02/26/const-generics-mvp-beta.html>

## Check yourself

1. Why is `#[repr(C)] struct { a: u8, b: u64 }` 16 bytes?
2. Why is `size_of::<Option<Box<u64>>>()` 8 but `size_of::<Option<u64>>()` 16?
3. What are the two words in a `&dyn Trait`?
4. Why can't you write `fn show(s: str)`?
5. When would you pick `Box<dyn Fn()>` over `impl Fn()`?
6. What does `match x {}` on an `Infallible` value do, and why does it type-check?
7. Name one cost `Arc<str>` saves compared to `String` when a value is cloned per subscriber.

<details><summary>Answers</summary>

1. `b` needs 8-byte alignment, so 7 padding bytes follow `a`. 1 + 7 + 8 = 16.
2. `Box` is never null, so null encodes `None`. A `u64` uses all bit patterns, so `Option` adds a tag, padded to 8.
3. A pointer to the data and a pointer to the vtable (methods, size, alignment, drop).
4. `str` is unsized; a by-value parameter needs a known size. Use `&str`.
5. When you need to store closures of different types together, or return different closures from branches, or avoid monomorphising a large function per closure type.
6. It has no arms because there are no variants; its type is `!`, which coerces to whatever type is expected.
7. An allocation plus a byte copy per clone. An `Arc` clone is one atomic increment.
</details>
