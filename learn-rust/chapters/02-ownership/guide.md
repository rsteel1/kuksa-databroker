# 02 — Ownership and borrowing

**Why this matters for the databroker.** Ownership is *the* Rust idea.
Everything else — lifetimes, `Arc`, `Mutex`, `Send`, async — is built on
it. In the databroker you'll constantly see decisions like "this function
takes the update *by value*", "this returns a *reference into the
database*", "this *clones* the datapoint so the lock can be released".
Each of those is an ownership decision, and after this chapter you'll be
able to read them as easily as a type signature.

Take this chapter slowly. It's worth it.

---

## Stack and heap: where values live

Your program has two main places to keep data:

- **The stack.** Every function call gets a *frame*: a fixed-size slab of
  memory for its local variables. Frames are pushed when a function is
  called and popped when it returns. Extremely fast, automatically cleaned
  up — but everything in a frame must have a size known at compile time.
- **The heap.** A big pool for data whose size is only known at runtime or
  that must outlive the function that created it — like the text of a
  signal path read from a file. You ask the allocator for N bytes, get back
  an address, and *someone must give it back exactly once* when done.

That last sentence is the whole problem. In C you free heap memory by hand
(forget → leak; do it twice → crash or security hole; use it after freeing
→ corruption). In Java/Python/Go a garbage collector finds unused memory at
runtime (costs CPU and unpredictable pauses — not great in a car).

Rust's answer: **every heap allocation has exactly one owner, and it is
freed automatically when that owner goes out of scope.** The compiler
checks the rules at compile time. No GC, no manual `free`.

A `String` shows both places at once:

```
   stack (in the current frame)            heap
  ┌───────────────────────────┐
  │ s: String                 │
  │   ptr ────────────────────┼──────▶  V e h i c l e . S p e e d
  │   len      = 13           │
  │   capacity = 13           │
  └───────────────────────────┘
```

The `String` value itself is three machine words on the stack; the
characters are on the heap. When `s` goes out of scope, Rust runs its
*drop* code, which returns the heap bytes to the allocator.

## What a move really is

```rust
let s1 = String::from("Vehicle.Speed");
let s2 = s1;           // move
println!("{s1}");      // error[E0382]: borrow of moved value: `s1`
```

Physically, `let s2 = s1;` copies the three stack words (ptr, len, cap)
into `s2`. Now *two* variables point at the same heap bytes. If both were
allowed to drop, the bytes would be freed twice. So Rust declares `s1`
**moved-from**: it's a dead name, the compiler refuses any use of it, and
it won't be dropped. Ownership has been *handed over*.

```
  s1 (dead) ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐
  s2: ptr ──────────────────────▶  V e h i c l e . S p e e d   (one owner)
```

The same happens when you pass a value to a function or return it:

```rust
fn append_unit(mut label: String, unit: &str) -> String { ...; label }

let speed = String::from("Vehicle.Speed");
let with_unit = append_unit(speed, "km/h");  // speed moved INTO the function
                                              // the result moved OUT to with_unit
```

A move is cheap (a few words copied, no heap work). It's a transfer of
*responsibility*, not of data.

In the databroker, `Entry::apply(&mut self, update: EntryUpdate)`
(`broker.rs:758`) takes the update by value. The caller is done with it;
`apply` may move its pieces straight into the entry without copying:

```rust
if let Some(datapoint) = update.datapoint {
    self.lag_datapoint = self.datapoint.clone();
    self.datapoint = datapoint;          // moved, not copied
```

### `Copy` types

```rust
let a: u32 = 7;
let b = a;       // copy
println!("{a}"); // fine
```

Integers, floats, `bool`, `char`, and tuples/arrays of those implement
`Copy`: they have no heap part, so duplicating the bits *is* a complete,
independent copy, and nothing needs freeing. For `Copy` types "move" is
just "copy". A type can only be `Copy` if it owns nothing on the heap —
which is why `String` and `Vec` never are. The databroker's `SignalId`
(`types.rs:490`) is `Copy` because it's just an `i32`.

## Clone: paying for a second copy

When you genuinely need two independent owners, ask for a deep copy:

```rust
let s2 = s1.clone();   // allocates new heap memory, copies the bytes
```

Now there are two allocations and two owners; each frees its own.

`clone()` is explicit on purpose: it costs an allocation and a copy, and
Rust wants that cost visible. Beginners often sprinkle `.clone()` until the
compiler stops complaining. Sometimes that's right; often it hides a better
design (borrow instead, or do things in a different order — exercise
`c02_e02`). Ask: *does this code really need its own copy?*

A legitimately needed clone: `AuthorizedAccess::get_datapoint`
(`broker.rs:1431-1439`) reads an entry while holding the database lock,
and returns `entry.datapoint.clone()`. The database must keep its value,
the caller wants one it can keep after the lock is released → two owners →
clone.

## Borrowing: lending instead of giving

Most functions only need to *look* at data. Moving it in and handing it
back is clumsy. Instead, **borrow**:

```rust
fn average(readings: &[u32]) -> u32 { ... }

let readings = vec![10, 20, 60];
let avg = average(&readings);   // lend it
let max = maximum(&readings);   // lend it again
drop(readings);                  // we still own it
```

`&readings` creates a **reference**: a pointer plus a compile-time promise
that the data will stay alive and unchanged for as long as the reference is
used. The borrower can read but not modify, and can't free it (it's not the
owner). Any number of shared references may exist at once.

`Entry::validate(&self, update: &EntryUpdate)` (`broker.rs:302`) is a
typical signature: it validates an update without consuming it, so the
caller can go on to apply it afterwards.

**Slices.** For "a list of numbers" prefer `&[u32]` (a *slice*: pointer +
length into someone else's contiguous data) over `&Vec<u32>`. A `&Vec<u32>`
turns into a `&[u32]` automatically, and a slice also accepts arrays and
sub-ranges (`&v[1..3]`). Clippy warns about `&Vec<T>` parameters.

## Mutable borrows and the one-writer rule

```rust
fn push_reading(buffer: &mut Vec<i32>, value: i32) { buffer.push(value); }

let mut readings = Vec::new();
push_reading(&mut readings, 5);
```

`&mut T` is an **exclusive** borrow: while it exists, nobody else can read
or write the value — not even the owner. The rule, enforced at compile time:

> At any moment a value has **either** any number of `&T` readers
> **or** exactly one `&mut T` writer. Never both.

Why so strict? Look at this innocent code:

```rust
let first = &readings[0];      // a pointer INTO the Vec's heap buffer
readings.push(99);             // may reallocate: copy to a bigger buffer, free the old one
println!("{first}");           // would read freed memory!
```

In C++ this compiles and corrupts memory at random ("iterator
invalidation"). In Rust it's error E0502: `push` needs `&mut readings`
while `first` (a `&`) is still alive. Fix: copy the value out
(`let first = readings[0];` — `i32` is `Copy`), or finish using `first`
before pushing.

The same rule, applied across threads, is why safe Rust has **no data
races**: a data race needs two accesses at once with at least one write,
and the rule forbids exactly that. You'll see this pay off in chapters
12-21 where the databroker wraps its database in an `RwLock` — the runtime
version of "many readers or one writer".

The databroker encodes the rule in a type: `DatabaseWriteAccess`
(`broker.rs:1067-1070`) holds `db: &'a mut Database`. While a write access
exists, nothing else can even look at the database.

Other points:
- You can only take `&mut x` if `x` itself is `mut` (E0596).
- To change the value behind a `&mut i32`, dereference it: `*r = 5;`.
  Method calls dereference automatically (`r.push(..)`), plain assignment
  doesn't.
- Borrows end at their **last use**, not at the end of the block
  ("non-lexical lifetimes"). That's why reordering lines often fixes E0502.

## String vs &str

| | `String` | `&str` |
|---|---|---|
| What | an owned, growable buffer of UTF-8 text | a borrowed view (pointer + length) into text owned by someone else |
| Where the bytes live | the heap, owned by this value | wherever the owner keeps them (a `String`, or the program binary for literals) |
| Can grow? | yes (`push_str`) | no |
| Use as a parameter when… | the function must *keep* the text | the function only *reads* it (most of the time) |
| Use as a return type when… | you created new text | you return a piece of an input |

`"Vehicle.Speed"` (a literal) is a `&'static str` — text baked into the
binary. `&String` converts to `&str` automatically ("deref coercion"), so a
`&str` parameter accepts both:

```rust
fn is_vehicle_signal(path: &str) -> bool { path.starts_with("Vehicle.") }
is_vehicle_signal("Vehicle.Speed");        // &'static str
is_vehicle_signal(&owned_string);          // &String → &str
```

Returning a slice of the input costs nothing — no allocation:

```rust
fn leaf_name(path: &str) -> &str {
    match path.rsplit_once('.') { Some((_, leaf)) => leaf, None => path }
}
```

Lookups in the databroker take `&str` because they only read the path:
`get_metadata_by_path(&self, path: &str)` (`broker.rs:1161`).
`get_entry_by_path(&self, path: impl AsRef<str>)` (`broker.rs:1148`) is a
generic version that accepts `&str`, `String`, `&String`… (chapter 09).

## Returning references

A reference must never outlive the thing it points to. Two consequences:

**1. You can't return a reference to a local.**

```rust
fn default_branch() -> &str {
    let branch = String::from("Vehicle");
    &branch           // branch is freed when we return → dangling pointer
}
```

The compiler rejects it (E0106 first — "missing lifetime specifier" —
because there's no input it could be borrowing from; then E0515). Fix:
return the owned `String` and let the caller own it.

**2. When returning a reference, the compiler must know *which input* it
borrows from.** With one reference parameter it's obvious and inferred
("lifetime elision"). With two, you say so with a lifetime parameter:

```rust
fn longer_path<'a>(a: &'a str, b: &'a str) -> &'a str { ... }
```

Read `'a` as a name for "some region of the program during which both
inputs are alive". The result is valid within that same region. Lifetimes
never change how long anything lives; they only *describe* relationships
so the compiler can check them. Chapter 11 is all about this.

`DatabaseReadAccess::get_entry_by_id` (`broker.rs:1137`) returns
`Result<&Entry, ReadError>` — a reference into the database. That's allowed
because the database outlives the call; the caller can look but has to
clone if it wants to keep the entry after the lock is gone.

## Drop: when owners die

When an owner goes out of scope — end of block, reassignment, or explicit
`drop(x)` — Rust runs its destructor. For `String`/`Vec` that frees the heap
buffer. For a lock guard it releases the lock. For a channel sender it
tells the receiver "no more messages". Deterministic cleanup at a known
point is called **RAII**, and the databroker relies on it everywhere
(chapter 12 and the async chapters).

---

## Mental model

| Concept | Analogy |
|---|---|
| Ownership | You hold the only key to a storage unit. When you leave town (scope ends), the unit is emptied. |
| Move | You hand the key to someone else. You no longer have a key. |
| `Copy` | The "storage unit" was just a note on paper — you photocopy it. |
| `clone()` | You rent a second unit and copy everything into it. Costs money. |
| `&T` | You lend someone a read-only visitor pass. Many passes can be out at once; nobody may rearrange the unit while any pass is out. |
| `&mut T` | You lend the *only* working key for a while. Until it's returned, even you can't go in. |
| Lifetime `'a` | The date printed on the visitor pass: "valid until…" |
| Drop | The unit is emptied the moment the last key-holder leaves. |

**What the compiler guarantees:** no use-after-free, no double free, no
dangling references, no iterator invalidation, and (with the same rules
across threads) no data races — all checked at compile time with zero
runtime cost.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "Assigning a String copies it" | It moves; the old name is dead | E0382 |
| "Clone whenever the compiler complains" | First ask who should own it; borrow or reorder | — |
| "`&T` is a pointer I can keep forever" | It's valid only while the owner is alive and unchanged | E0597, E0515 |
| "The owner can always read its own data" | Not while a `&mut` borrow of it is alive | E0502, E0499 |
| "I can move out of something I borrowed" | You only borrowed it; clone it or borrow it | E0507 |
| "Lifetimes make values live longer" | They only describe; they never extend anything | E0106 |
| "`String` and `&str` are two string types to pick from randomly" | Owned vs borrowed — same as `Vec<T>` vs `&[T]` | E0308 |

## Smells and idioms

- **Clone to silence the borrow checker** — the #1 beginner smell. Try
  borrowing, reordering, or restructuring first. Clone when two independent
  owners are genuinely needed (and note: cloning an `Arc` is cheap and
  idiomatic — chapter 12).
- **`&String` / `&Vec<T>` parameters** → `&str` / `&[T]`.
- **`String` parameters for read-only text** → `&str`. Take `String` only
  when the function stores it.
- **Returning references to locals** — return owned values.
- **Holding a reference while mutating the owner** — copy the small value
  out first.
- The databroker sometimes clones where a borrow would do (e.g. cloning
  `Permissions` when a subscription is created). Often that's a deliberate
  trade-off: the clone lets the data outlive a lock or a request, which
  keeps the code simple. When you read a `.clone()` in real code, ask *what
  lifetime problem it solves*.

## In the databroker

1. `broker.rs:758-779` `Entry::apply(&mut self, update: EntryUpdate)` —
   `&mut self` (changes the entry), `update` by value (consumed, fields
   moved into the entry).
2. `broker.rs:276` `Entry::diff(&self, mut update: EntryUpdate) -> EntryUpdate`
   — takes an update, edits its own copy (`mut update`), hands it back.
   Ownership flows *through* the function.
3. `broker.rs:302` `Entry::validate(&self, update: &EntryUpdate)` — only
   looks, so borrows both.
4. `broker.rs:1431-1439` `get_datapoint` — `.clone()` to get data out from
   behind a lock.
5. `broker.rs:1137-1153` `get_entry_by_id` / `get_entry_by_path` — return
   `&Entry`, references *into* the database.
6. `broker.rs:1067-1070` `DatabaseWriteAccess { db: &'a mut Database, .. }`
   — the one-writer rule as a struct.
7. `broker.rs:2338` `DataBroker::new(version: impl Into<String>, ..)` —
   takes ownership of the version text (it stores it) but accepts `&str` or
   `String` from the caller.

## Exercises

| Exercise | Goal |
|---|---|
| `c02_e01_move` | E0382: reorder so you read a String before moving it |
| `c02_e02_clone_vs_move` | Avoid a needless clone; use a needed one; E0507 |
| `c02_e03_borrow` | Borrow as `&[u32]` instead of moving a `Vec` |
| `c02_e04_mut_borrow` | E0596 and E0502; mutate through `iter_mut()` and `*` |
| `c02_e05_string_vs_str` | `&str` params; return slices of the input; build a `String` |
| `c02_e06_slices_and_returning_refs` | E0106/E0515; a first lifetime annotation; safe sub-slicing |
| `c02_e07_build_path_buffer` | Build: dedup/common-prefix/take-first over a path buffer from a spec |
| `c02_e08_review_reading_buffer` | Review: find 3 planted bugs (clone hides a mutation, stale read, off-by-one) |

## Further reading

- The Rust Book, ch. 4 "Understanding Ownership" (4.1 What is Ownership?,
  4.2 References and Borrowing, 4.3 The Slice Type) —
  <https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html>
- Rust by Example: "Scoping rules" (RAII, Ownership and moves, Borrowing).
- `rustc --explain E0382`, `E0502`, `E0499`, `E0507`, `E0106`, `E0515`.
- "Visualizing memory layout of Rust's data types" (search the title; a
  well-known video) — for the stack/heap pictures.

## Check yourself

1. After `let b = a;` where `a: String`, what exactly was copied, and why
   can't `a` be used any more?
2. Why is `u32` `Copy` but `String` not?
3. What bug does E0502 prevent in `let x = &v[0]; v.push(1); use(x);`?
4. When should a function take `String` rather than `&str`?
5. Why can't `fn f() -> &str` return a reference to a local `String`?
6. Why does `get_datapoint` in `broker.rs` return a clone instead of `&Datapoint`?

<details><summary>Answers</summary>

1. The three stack words (pointer, length, capacity). If `a` stayed usable,
   two owners would free the same heap buffer. So `a` is moved-from.
2. `u32` owns nothing on the heap, so copying its bits is a complete copy.
   A `String` owns a heap buffer; bit-copying would create two owners.
3. `push` may reallocate the vector's buffer, leaving `x` pointing at freed
   memory.
4. When it keeps/stores the text (e.g. puts it in a struct or collection).
5. The local is dropped (freed) when the function returns; the reference
   would dangle. Return an owned `String` instead.
6. The datapoint lives inside the database behind a lock. A reference would
   only be valid while the lock is held; the method releases the lock before
   returning, so it hands the caller its own copy.

</details>

## Where this goes next

Ownership is the base of everything that follows. Lifetimes (chapter 11) name how long a borrow lasts. Smart pointers (chapter 12) give you shared ownership when one owner isn't enough. Chapter 16 covers the harder borrow-checker cases: variance, reborrowing and NLL.
