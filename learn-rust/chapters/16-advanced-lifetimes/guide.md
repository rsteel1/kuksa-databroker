# 16 — Advanced lifetimes

**Why this matters for the databroker (and every other codebase).** Chapter
11 taught you to *read* lifetimes on functions and structs. Real code also
has lifetimes in *bounds*: `SignalProvider: Send + Sync + 'static`
(`databroker/src/broker.rs:198`), `Box<dyn Trait + 'a>`, `for<'a> Fn(&'a str)`,
`T: 'a`. And some borrow-checker errors can't be fixed by adding an
annotation, because the design itself is impossible (a struct that points
into itself). This chapter covers the rest of the lifetime story: what
`'static` really means, trait-object lifetimes, higher-ranked bounds,
variance, reborrowing, and the self-referential problem that leads straight
to `Pin` in chapter 24. After it, no lifetime in a library signature
should surprise you.

> This chapter comes before async. Where async code needs the same ideas
> (`tokio::spawn` requires `'static`, futures are self-referential), you'll
> see a forward pointer, and chapters 20 and 24 pick it up.

---

## 'static: a bound, not a lifetime of the value

`'static` shows up in two different places, and they mean different things:

| You write | Meaning | Example |
|---|---|---|
| `&'static T` | a reference valid for the rest of the program | string literals, `static` items, `Box::leak` |
| `T: 'static` | `T` contains **no borrows shorter than `'static`** | `String`, `Vec<u8>`, `i32`, `Arc<Mutex<X>>`, `&'static str` |

The second is the one you'll meet constantly, and it does **not** mean
"the value lives forever". A `String` is `'static` and is dropped whenever
you like. It means *the type is not tied to any stack frame*, so it's safe
to hand to something that may keep it arbitrarily long:

```rust
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static, // may run after the caller returns
    T: Send + 'static,
```

A closure that captures `&local` is not `'static`, so `std::thread::spawn`
rejects it (E0373/E0310/E0597, depending on the shape). The fixes, in order:

1. **Move owned data in** (`move` closure + owned values).
2. **Share ownership** with `Arc<T>` (chapter 12).
3. **Use `std::thread::scope`**: scoped threads *may* borrow, because the
   scope joins every thread before it returns (chapter 25 goes deeper).

The databroker requires `'static` for anything it stores long-term:
`pub trait SignalProvider: Send + Sync + 'static` (`databroker/src/broker.rs:198`)
and `Box<dyn SignalProvider + Send + Sync + 'static>` in `register_signals`
(`databroker/src/broker.rs:2070`). A provider that borrowed from the gRPC
handler's stack would dangle as soon as the handler returned.

**Forward pointer:** `tokio::spawn` has the same `F: Future + Send + 'static`
bound (`~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/src/task/spawn.rs:176`),
which is why `open_provider_stream` clones the broker before
`tokio::spawn(async move { ... })` (`databroker/src/grpc/kuksa_val_v2/val.rs:933`).
Chapter 20 covers this.

## Lifetime bounds and trait-object lifetimes

**`T: 'a`** reads "every reference inside `T` outlives `'a`". You need it
when you hold a `&'a T`, because the referent must be valid for as long as
the reference. Since the 2018 edition the compiler infers it on struct
definitions (`struct Borrowed<'a, T> { inner: &'a T }` implies `T: 'a`),
but you'll still see it written out in older code and in `impl` blocks.

**Trait objects hide lifetimes.** `Box<dyn Fn(&str) -> bool>` could be
holding a closure that borrows something, so the type must say for how
long. When you don't say, the **default object lifetime** applies:

| Written | Means |
|---|---|
| `Box<dyn Trait>` | `Box<dyn Trait + 'static>` |
| `Arc<dyn Trait>` | `Arc<dyn Trait + 'static>` |
| `&'a dyn Trait` | `&'a (dyn Trait + 'a)` |
| `&'a mut dyn Trait` | `&'a mut (dyn Trait + 'a)` |
| `impl Trait` in return position (2021 edition) | captures only the lifetimes you name, e.g. `+ '_` |

So `fn make_check(prefix: &str) -> Box<dyn Fn(&str) -> bool>` fails:
the closure borrows `prefix`, but the default demands `'static`. Write
`-> Box<dyn Fn(&str) -> bool + '_>` to tie it to the argument, or give a
struct a lifetime parameter and store `Box<dyn Fn(&str) -> bool + 'a>`.

Edition note: in edition 2024, `impl Trait` in return position captures
*all* in-scope lifetimes by default (RPIT capture rules). The databroker is
on 2021, where you may need `+ '_` explicitly.

## Higher-ranked trait bounds (for<'a>)

Compare two bounds:

```rust
fn apply_once<'a, F: Fn(&'a str) -> &'a str>(f: F, s: &'a str) -> &'a str
fn apply_local<F: for<'x> Fn(&'x str) -> &'x str>(f: F) -> String
```

- In the first, **the caller** picks one lifetime `'a`, and `f` only has to
  work for that one.
- In the second, `f` must work for **every** lifetime `'x`. That's a
  **higher-ranked trait bound** (HRTB). You need it when the function
  itself creates the borrow, for example calling `f(&local_string)`
  on a value it owns internally. No caller-chosen `'a` could describe
  that local.

The good news: when you write `F: Fn(&str) -> &str` with **elided**
lifetimes, the compiler already makes it higher-ranked. You mostly write
`for<'a>` explicitly when the bound appears in a `where` clause on a trait,
or when you want to be clear.

The databroker relies on this without writing it:
`filter_map_entries<T>(&self, f: impl FnMut(EntryReadAccess) -> Option<T>)`
(`databroker/src/broker.rs:1514`) calls `f` with `EntryReadAccess<'_>` values
that borrow the database *inside* the method, while a lock is held. The
elided lifetime in `EntryReadAccess` makes the bound higher-ranked.

**Closures and HRTB.** A closure stored in a `let` and returning a
reference often gets two unrelated lifetimes inferred, and fails with
"lifetime may not live long enough" or "implementation of `Fn` is not
general enough". Fixes: write the closure directly where a higher-ranked
bound is expected (passing it to a function with such a bound), or use a
`fn` item instead.

## Variance: when a longer lifetime can stand in for a shorter one

*Variance* answers: if `'long: 'short` (`'long` outlives `'short`), can a
`Thing<'long>` be used where a `Thing<'short>` is expected?

| Type | Variance in `'a` / `T` | Meaning |
|---|---|---|
| `&'a T` | covariant in `'a` and `T` | a longer borrow can shrink to a shorter one |
| `&'a mut T` | covariant in `'a`, **invariant in `T`** | the pointee type must match exactly |
| `Box<T>`, `Vec<T>`, `Option<T>` | covariant in `T` | like owning a `T` |
| `Cell<T>`, `RefCell<T>`, `UnsafeCell<T>` | **invariant in `T`** | interior mutability behaves like `&mut` |
| `fn(T) -> U` | **contra**variant in `T`, covariant in `U` | a function taking a *longer*-lived arg is less useful |

Why is `&mut T` invariant? If you could treat `&mut Vec<&'static str>` as
`&mut Vec<&'short str>`, you could push a short-lived string into a vector
that someone else believes contains only `'static` strings, and they'd read a
dangling pointer later. So the compiler forbids it.

The classic bug, and exercise `c16_e04`:

```rust
fn add_defaults(paths: &mut Vec<&'static str>) { paths.push("Vehicle.Speed"); }

let owned = String::from("Vehicle.Cabin");
let mut paths: Vec<&str> = vec![&owned];
add_defaults(&mut paths);   // forces paths: Vec<&'static str> (invariance!)
// error[E0597]: `owned` does not live long enough
```

The fix is to not over-constrain: `fn add_defaults(paths: &mut Vec<&str>)`.
The literals are `&'static str` and shrink (covariantly) to whatever the
vector holds.

The second classic: `&'a mut Thing<'a>`. Tying the mutable borrow's
lifetime to the lifetime *inside* the thing means the variable stays
mutably borrowed for as long as its contents live, often forever
(E0499/E0502 on the next use). Give them separate lifetimes:
`&mut Thing<'a>`. The databroker does this right:
`authorized_write_access<'a, 'b>(&'a mut self, permissions: &'b Permissions)`
(`databroker/src/broker.rs:1362`) keeps independent borrows on independent
lifetimes.

Rule of thumb: **use as few named lifetimes as possible, and never force
two lifetimes to be equal unless the data really requires it.**

## Reborrowing, two-phase borrows and NLL

**Reborrowing.** `&mut T` isn't `Copy`, yet this works:

```rust
fn push_one(v: &mut Vec<u32>) { v.push(1); }
let r = &mut v;
push_one(r);
push_one(r); // fine
```

The compiler inserted `push_one(&mut *r)`: a *reborrow*, a new, shorter
`&mut` derived from `r`. It only does this when the parameter type is
*known* to be `&mut _`. For a generic parameter `fn record<W: Write>(w: W)`,
`r` is **moved** in (E0382 on the second use). Write the reborrow yourself:
`record(&mut *r, ...)`. Method calls do the same thing automatically through
auto-ref: in `update_entries`, `db_write.update(id, update)` runs once per
entry in a loop (`databroker/src/broker.rs:1543`).

**Two-phase borrows.** `v.push(v.len())` looks like it takes `&mut v` and
then `&v`. It compiles because the mutable borrow is *reserved* first and
only *activated* when `push` actually runs, after `v.len()` has been evaluated.

**Non-lexical lifetimes (NLL).** Since 2018 a borrow ends at its **last
use**, not at the end of the block:

```rust
let first = &v[0];
println!("{first}");
v.push(3);  // fine: `first` is no longer used
```

**NLL's known limit ("problem case #3"):** returning a borrow on one path
and mutating on another is rejected, even though it's sound:

```rust
fn value_or_insert<'m>(map: &'m mut HashMap<String, i64>, key: &str) -> &'m i64 {
    if let Some(v) = map.get(key) { return v; }  // borrow must last for 'm...
    map.insert(key.to_string(), 0);              // E0502: ...so this conflicts
    &map[key]
}
```

The next-generation borrow checker (Polonius) accepts it. Today,
restructure: check with `contains_key` first, or use the `entry` API.

## The self-referential struct problem

Suppose you want a database with a fast index that points straight at its
own entries:

```rust
struct Database<'a> {
    entries: Vec<Entry>,
    by_path: HashMap<&'a str, &'a Entry>,  // references into `entries`
}
```

To store `&'a Entry` taken from `self.entries`, `add` must take
`&'a mut self`, which borrows the database mutably **for its entire
lifetime** (the variance trap above). After one `add`, you can't call
anything else. And it isn't only a type-system problem: pushing into
`entries` can reallocate the `Vec` and move every entry, so the stored
references would dangle. Moving the `Database` itself would break them too.

```text
entries: Vec<Entry> ──► [ e0 | e1 | e2 ]      push e3 → reallocation
by_path: "a" ─────────────┘                    ──► [ e0 | e1 | e2 | e3 ] (new address)
                                               "a" still points at the OLD buffer ✗
```

Fixes, best first:

1. **Indices or ids instead of references.** `path_to_id: HashMap<String, usize>`
   pointing into a `Vec`, or ids into a `HashMap`. This is exactly the
   databroker's design: `path_to_id: HashMap<String, i32>` and
   `entries: HashMap<i32, Entry>` (`databroker/src/broker.rs:130-131`).
   `indexmap` does the same internally (`indices` plus a dense `entries` vec).
2. **Shared ownership**: `Arc<Entry>` in both places. It costs a refcount and
   makes mutation harder.
3. **Pin the data** and use `unsafe`, or a crate like `ouroboros`. Rarely
   worth it in application code.

**Forward pointer:** an `async fn` that holds a reference across `.await`
compiles into a state machine that *is* self-referential. That's why futures
must be pinned before they're polled. Chapter 24 explains `Pin`.

## Mental model

| Idea | Analogy |
|---|---|
| `T: 'static` | "carries no borrowed keys": it can be mailed anywhere |
| `&'static T` | a reference to a monument: it will always be there |
| `dyn Trait + 'a` | a sealed parcel labelled "contents valid until 'a" |
| HRTB `for<'x>` | a tool that fits *every* bolt size vs one that fits the size you order |
| covariance | a year pass works where a day pass is required |
| invariance of `&mut T` | you can't lend someone your day-pass holder and let them slip in a year pass someone else will rely on |
| reborrow | lending a borrowed book on, with the promise it comes back to you first |
| self-referential struct | a map drawn on the house it describes: move the house and the map lies |

**Guarantees:** no reference outlives its data, no `&mut` aliases another
live reference, and spawning or storing values can never smuggle a borrow
past the end of its owner's scope.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`T: 'static` means the value is never dropped." | It means `T` holds no non-`'static` borrows. `String: 'static`. |
| "Adding `'static` to a parameter makes it live longer." | Annotations never change how long anything lives; they only constrain callers. |
| "`Box<dyn Trait>` can hold anything implementing `Trait`." | Only `'static` things, unless you write `+ 'a`. |
| "I need `for<'a>` every time I take a closure." | Elided `Fn(&str) -> &str` is already higher-ranked. |
| "`&mut T` converts like `&T`." | `&mut T` is invariant in `T`. E0597/E0308 when you over-constrain. |
| "The borrow checker is wrong about `value_or_insert`." | It's a known NLL limitation (problem case #3); restructure. |
| "`clone()` fixes E0382 on a `&mut`." | Reborrow with `&mut *r`; cloning a `&mut` isn't possible. |
| "A struct can hold a reference to its own field if I get the lifetimes right." | Not with safe references. Use indices, `Arc`, or `Pin`. |

## Smells and idioms

- **`'static` everywhere to silence errors**: it forces callers to leak or
  own everything. Prefer owning data, `Arc`, or scoped threads.
- **`&'a mut Thing<'a>`**: almost always a bug; give the borrow its own
  lifetime.
- **`&mut Vec<&'static str>` parameters**: over-constrained; `&mut Vec<&str>`.
- **Many named lifetimes, all equal**: let elision do the work, and name only
  the relationships that matter. Chapter 11's meaningful names (`'db`, `'perm`)
  help when you do need several.
- **`Rc<RefCell<>>` or `unsafe` to build self-referential structs**: reach
  for indices first, as the databroker and `indexmap` do.
- **Fighting NLL case #3 with `unsafe`**: restructure with `contains_key` /
  `entry` instead.

## In the databroker and in the wild

| Where | What to notice |
|---|---|
| `databroker/src/broker.rs:198` | `pub trait SignalProvider: Send + Sync + 'static`: a supertrait bound so every provider can be stored and shared |
| `databroker/src/broker.rs:2070` | `signal_provider: Box<dyn SignalProvider + Send + Sync + 'static>`: the default object lifetime written out |
| `databroker/src/broker.rs:235` | `Arc<dyn SignalProvider>`: the `+ 'static` is implied by the default object lifetime |
| `databroker/src/broker.rs:1362` | `authorized_write_access<'a, 'b>(&'a mut self, permissions: &'b Permissions)`: independent lifetimes, no variance trap |
| `databroker/src/broker.rs:1067` | `DatabaseWriteAccess<'a, 'b> { db: &'a mut Database, ... }` |
| `databroker/src/broker.rs:1514` | `filter_map_entries(f: impl FnMut(EntryReadAccess) -> Option<T>)`: an implicitly higher-ranked closure bound |
| `databroker/src/broker.rs:1543` | `db_write.update(id, update)` in a loop: auto-ref reborrowing |
| `databroker/src/broker.rs:130` | `path_to_id: HashMap<String, i32>` + `entries: HashMap<i32, Entry>`: ids, not self-references |
| `databroker/src/grpc/kuksa_val_v2/val.rs:933` | `tokio::spawn(async move { ... })` after cloning the broker: satisfying `'static` |
| `~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/src/task/spawn.rs:176` | `F: Future + Send + 'static` on `tokio::spawn` |
| `~/.cargo/registry/src/index.crates.io-*/async-trait-0.1.92/src/expand.rs:155` | The comment shows what `#[async_trait]` generates: `'life0: 'async_trait` bounds and `Pin<Box<dyn Future + Send + 'async_trait>>`. Lifetime bounds on a boxed trait object, exactly this chapter |
| `~/.cargo/registry/src/index.crates.io-*/indexmap-2.14.2/src/inner.rs:29` | `Core { indices, entries }`: the index-instead-of-reference design |

## Exercises

| Exercise | Goal |
|---|---|
| `c16_e01_static_bound_vs_static_ref` | Add the `'static` bound `thread::spawn` needs; borrow with `thread::scope` instead |
| `c16_e02_lifetime_bounds` | `Box<dyn Fn + 'a>`, a lifetime-parameterised alias, and struct lifetimes for borrowing closures |
| `c16_e03_hrtb` | Fix a caller-chosen lifetime with `for<'x>`; make a closure higher-ranked |
| `c16_e04_variance` | Fix two over-constrained signatures (`&mut Vec<&'static str>`, `&'a mut &'a str`) |
| `c16_e05_reborrowing_and_nll` | Explicit reborrow through a generic; restructure around NLL problem case #3 |
| `c16_e06_self_referential_problem` | Redesign a self-referential database with ids, like `broker.rs` |

## Further reading

- The Rustonomicon, "Subtyping and Variance" and "Higher-Rank Trait Bounds": <https://doc.rust-lang.org/nomicon/subtyping.html>
- The Rust Reference, "Lifetime elision" and "Trait object lifetime bounds": <https://doc.rust-lang.org/reference/lifetime-elision.html>
- "Common Rust Lifetime Misconceptions" by pretzelhammer: <https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md>
- NLL RFC 2094 (problem case #3): <https://rust-lang.github.io/rfcs/2094-nll.html>
- *Rust for Rustaceans*, ch. 1 "Foundations" (variance, lifetimes)
- `std::thread::scope` docs: <https://doc.rust-lang.org/std/thread/fn.scope.html>

## Check yourself

1. Is `String: 'static`? Is `&'a str: 'static` for a local `'a`?
2. What does `Box<dyn Error>` mean in full, lifetime-wise?
3. When do you need `for<'a>` in a bound, and when is it implied?
4. Why is `&mut T` invariant in `T` but `&T` covariant?
5. Why does `record(out, ..)` move `out` when `record` is generic, but not when it takes `&mut String`?
6. Why can't `Database` store `&Entry` references into its own `Vec<Entry>`, even ignoring lifetimes?
7. Why does `std::thread::scope` let threads borrow local data?

<details><summary>Answers</summary>

1. Yes: it owns its data. No: it borrows from something that can go away.
2. `Box<dyn Error + 'static>`: the error must not borrow short-lived data.
3. When the function itself creates the borrows it passes to the closure. It's implied when lifetimes in an `Fn(&T) -> &U` bound are elided.
4. Through `&mut T` you can *write* a `T`. Allowing a subtype there would let you store a shorter-lived value where a longer-lived one is expected. Through `&T` you can only read.
5. Reborrowing is inserted only when the expected type is known to be a `&mut`. A generic `W` just takes the value by move.
6. Pushing can reallocate the `Vec` (and moving the struct moves it), invalidating every stored reference.
7. The scope doesn't return until every spawned thread has been joined, so the borrows provably end before the data does.
</details>
