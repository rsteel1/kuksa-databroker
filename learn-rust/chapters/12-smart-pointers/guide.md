# 12 — Smart pointers, Drop & interior mutability

**Why this matters for the databroker.** The databroker is one big piece of
*shared state* — the database, the subscriptions, the providers — used at
the same time by many gRPC connections running on many threads. Chapter 11's
borrowing views are perfect for "look at it for one operation", but a spawned
task or a registered provider must *own* its access for as long as it lives.
The answer is the pair you'll see everywhere in `broker.rs`:
`Arc<RwLock<Database>>`. This chapter builds that up from the pieces: `Box`,
`Drop`, `Rc`/`RefCell`, `Arc`, locks and their guards, atomics, and
lazily-initialised globals.

---

## Box: one owner, on the heap

`Box<T>` puts a `T` on the heap and owns it. When the box is dropped, so is
the `T`. It has exactly one owner — the rules are those of a plain value.

You need it when:

1. **A type contains itself.** Every type needs a size known at compile
   time. `enum Filter { Not(Filter), … }` would be infinitely large (E0072).
   `Not(Box<Filter>)` is one pointer wide.
2. **You want a trait object with one owner:** `Box<dyn SignalProvider>`.
   The concrete type is unknown at compile time, so its size is too; the box
   gives it a fixed-size handle. The databroker's `register_signals` receives
   `signal_provider: Box<dyn SignalProvider + Send + Sync + 'static>`
   (`databroker/src/broker.rs:2070`) and converts it to an `Arc` with
   `Arc::from(...)` (`:2100`) to share it.
3. Moving a large value around cheaply (moving a box moves 8 bytes).

Method calls see through a box automatically (`Deref`), so `boxed.matches(p)`
works as if you had the `Filter` itself.

## Drop and RAII guards

When a value goes out of scope Rust calls its destructor — the `Drop` trait:

```rust
impl Drop for SubscriptionHandle<'_> {
    fn drop(&mut self) {
        // release whatever this handle represents
    }
}
```

This is **RAII**: acquiring a resource = creating a value; releasing it =
that value being dropped. Because drops are automatic, release happens on
*every* way out of a scope — end of block, `return`, `?`, even a panic
unwinding. You can't forget.

Rules worth memorising:

- Locals are dropped at the end of their scope in **reverse order** of
  declaration.
- A temporary (e.g. the guard in `db.read().await.get(x)`) is dropped at the
  end of the **statement** (the `;`).
- `drop(x)` ends a value's life early. It's an ordinary function: it takes
  ownership and returns, and the value is dropped.
- A moved-from variable is not dropped again — the new owner drops it.
- `let _ = expr;` drops immediately. `let _guard = expr;` keeps it until the
  end of scope. (That underscore difference has caused real bugs.)

**Lock guards are RAII.** `mutex.lock()` returns a `MutexGuard`; the lock is
held until the guard is dropped. In the databroker:

```rust
let providers = {
    let subscriptions = self.broker.subscriptions.read().await;   // guard
    subscriptions.signal_provider_subscriptions.values()…collect()
};                                                                   // guard dropped → lock released
// … now call the (possibly slow) providers without holding the lock
```

(`get_values_broker`, `databroker/src/broker.rs:2210`.) The extra `{ }`
block exists *only* to end the guard's life early. This was the fix in
commit `e1fdfd1`: a slow provider must never block everyone else.

**Channel ends are RAII too.** When a gRPC client disconnects, tonic drops
the response stream; that drops the `broadcast::Receiver` inside it; the
housekeeping task then sees `sender.receiver_count() == 0` and removes the
subscription (`cleanup_change_subscriptions`, `databroker/src/broker.rs:892`).
No explicit "unsubscribe" message is ever needed.

## Rc and RefCell: shared, single-threaded

Sometimes one value genuinely has several owners.

`Rc<T>` (reference counted): `Rc::clone(&rc)` makes another owning pointer
to the same `T` and bumps a counter. When the last `Rc` drops, the `T` is
dropped. `Rc::strong_count(&rc)` shows the count. Shared ownership means
*shared* access: you get `&T`, never `&mut T`.

`RefCell<T>` moves the borrow rules from compile time to **runtime**:
`cell.borrow()` returns a `Ref<T>` (like `&T`), `cell.borrow_mut()` a
`RefMut<T>` (like `&mut T`). Break the rule (a `borrow_mut` while another
borrow is alive) → **panic** "already borrowed". `try_borrow`/
`try_borrow_mut` return a `Result` instead.

That's called **interior mutability**: mutating through a shared reference,
safely, because *something* still enforces "many readers XOR one writer".
`Cell<T>` is the minimal version for `Copy` values (`get`/`set`, no borrows).

`Rc<RefCell<T>>` = "shared, mutable, one thread". Neither `Rc` nor
`RefCell` is thread-safe (their counters aren't atomic), and the compiler
won't let them cross threads (chapter 13). The databroker never uses them;
it uses the thread-safe equivalents below.

## Arc: shared across threads

`Arc<T>` = **atomically** reference-counted `Rc`. Same API, safe across
threads.

**What does `Arc::clone` cost?** One atomic increment — a few nanoseconds.
It does *not* copy `T`. That's why this is idiomatic:

```rust
#[derive(Clone)]
pub struct DataBroker {                          // databroker/src/broker.rs:179
    database: Arc<RwLock<Database>>,
    subscriptions: Arc<RwLock<Subscriptions>>,
    …
}
```

Cloning a `DataBroker` clones a handful of `Arc`s and two short `String`s;
every clone sees the *same* database. Each gRPC task gets its own clone
(`let broker = self.clone();` in `open_provider_stream`). "Clone
everywhere" is a smell for `String`/`Vec`; for `Arc` it's the design.

Prefer `Arc::clone(&x)` over `x.clone()` when you want the reader to see
"this is a cheap handle copy". The databroker mostly writes `.clone()` —
both are correct.

`Arc<dyn Trait>`: shared ownership of a trait object. Providers are stored as
`Arc<dyn SignalProvider>` (`:235`) so the broker can snapshot them
(`provider.signal_provider.clone()`, `:2225`) and call them *after* releasing
the subscriptions lock.

`Arc::ptr_eq(&a, &b)` tells you whether two handles point to the same
allocation.

## Mutex and RwLock: guards release locks

`Arc` gives shared *ownership* but only `&T` access. To mutate shared data
across threads, put a lock inside: `Arc<Mutex<T>>` or `Arc<RwLock<T>>`.

| Type | Access | Use when |
|------|--------|----------|
| `Mutex<T>` | one holder at a time | writes are common, critical sections tiny |
| `RwLock<T>` | many readers **or** one writer | reads dominate — like the databroker's database |
| atomics | lock-free single values | counters, flags |

```rust
let guard = values.read().expect("lock poisoned");  // std: returns Result
let v = guard.get("Vehicle.Speed").copied();
drop(guard);                                        // or let it fall out of scope
```

Things to know:

- **Guards deref** to the data: `guard.get(..)`, `*guard += 1`.
- **Keep critical sections short.** Copy what you need out ("snapshot"),
  drop the guard, then do slow work.
- **Poisoning (std only).** If a thread panics while holding a std lock, the
  lock is marked poisoned and later `lock()` returns `Err`. Most code does
  `.expect("lock poisoned")` — a poisoned lock means a bug already happened.
- **Lock ordering.** If code ever holds two locks, always take them in the
  same order everywhere, or two threads can deadlock waiting on each other.
  The databroker takes `database` then `subscriptions` (see
  `update_entries`, `databroker/src/broker.rs:1534`–`1567`).
- **std vs tokio locks.** The databroker uses `tokio::sync::RwLock`, whose
  `read()`/`write()` are `async` (`.await`) and don't poison. The rule
  "don't hold a *std* guard across `.await`" and the tokio lock's extra
  tricks (like `downgrade()`) are chapter 21.

## Atomics and memory ordering

For a single integer or boolean, a lock is overkill. `AtomicI32`,
`AtomicU32`, `AtomicUsize`, `AtomicBool` offer operations that complete as
one indivisible step even when threads race:

```rust
let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);  // val.rs:174
```

`fetch_add` reads, adds, writes *atomically* and returns the old value. Two
threads can never get the same id. A `get` followed by a `set` (two steps)
can.

Every atomic operation takes an `Ordering`, which answers: *what else, besides
this one variable, is guaranteed to be visible to other threads?*

| Ordering | Guarantees | Typical use |
|----------|-----------|-------------|
| `Relaxed` | The operation itself is atomic; nothing about other memory | Counters, id generators, statistics |
| `Release` (store) / `Acquire` (load) | Everything written before the Release store is visible after an Acquire load that sees it | Flags that *publish* data: "ready", "shutdown requested" |
| `SeqCst` | Acquire/Release plus a single global order of all SeqCst ops | Default when unsure |

**Why `Relaxed` is fine for an id counter:** all we need is uniqueness, and
atomicity alone gives that. No other data is "published" by incrementing the
counter. The databroker's `Provider::next_request_id` uses `Relaxed`;
`Database::next_id` uses `SeqCst` (`databroker/src/broker.rs:1326`) — also
correct, just stronger than necessary (and it's already under the write lock
there, so ordering is moot).

`Cell<T>` is **not** `Sync`: using it from several threads is a compile error
(E0277). Atomics are `Sync`.

## Globals: LazyLock, OnceLock and lazy_static

A `static` must be initialised by a `const` expression at compile time.
`Vec::new()` is const, but `vec![...]`, `String::from`, `HashMap::new()`, or
any regex compile are not (E0015). For globals that need runtime work:

- **`std::sync::LazyLock<T>`** (stable since Rust 1.80): holds an init
  closure; the first access runs it, thread-safely, exactly once.

  ```rust
  static ALLOW_ALL: LazyLock<Permissions> = LazyLock::new(Permissions::all);
  ```

- **`std::sync::OnceLock<T>`**: an empty slot that can be filled once, from
  wherever you have the data. `get_or_init(f)` fills it on first call;
  concurrent callers wait.

  ```rust
  fn config() -> &'static Config {
      static CONFIG: OnceLock<Config> = OnceLock::new();
      CONFIG.get_or_init(load_config)
  }
  ```

- **`lazy_static!`** — the macro crate that did this before std could. The
  databroker still uses it:

  ```rust
  lazy_static! {                                   // databroker/src/permissions.rs:21
      pub static ref ALLOW_ALL: Permissions = Permissions { … };
  }
  ```

  It works and there's no urgency to change it, but new code should use
  `LazyLock` (one less dependency, plain Rust syntax).

  **A twist worth noticing:** look at what `ALLOW_ALL` is built from —
  `expires_at: None` and unit variants like `PathMatcher::Everything`. All of
  those are const-constructible, so `ALLOW_ALL` doesn't need lazy
  initialisation at all: a plain `pub static ALLOW_ALL: Permissions =
  Permissions { … };` compiles (this course checked it). Lazy init is only
  required when the initialiser calls non-`const` code — as in `c12_e07`,
  where `Permissions::all()` builds a `Vec<String>`. Spotting "this macro
  isn't needed any more" is exactly the kind of small cleanup PR a new
  contributor can make.

Globals should be rare. `ALLOW_ALL` is a good one: an immutable constant-like
value used when authorization is disabled and in tests. Mutable global state
is a smell — pass an `Arc` instead.

## Weak: breaking reference cycles

`Rc` frees its value when the **strong** count reaches zero. If two values
own each other (a parent with `Rc` children whose children hold an `Rc`
back to the parent), neither count ever reaches zero: the memory **leaks**.
This isn't unsafe (Rust's safety guarantees don't cover leaks), but it's a bug.

`Weak<T>` is a non-owning pointer:

```rust
let parent = Rc::new(node);
let back_link: Weak<Node> = Rc::downgrade(&parent); // weak count +1, strong unchanged
match back_link.upgrade() {                         // Option<Rc<Node>>
    Some(p) => println!("parent is {}", p.name),     // still alive
    None => println!("parent was dropped"),
}
```

The rule for trees and graphs: **owners point down with `Rc`, back-references
point up with `Weak`.** `Rc::strong_count` and `Rc::weak_count` let tests
prove there's no cycle. The thread-safe equivalent is `std::sync::Weak`
(from `Arc::downgrade`). tokio uses it internally so that a background task
doesn't keep the object that spawned it alive forever.

Often the simplest fix is to avoid parent pointers altogether: store nodes
in a `Vec` and refer to them by index, or, like the databroker's `vss.rs`,
flatten the tree into a map keyed by full path.

## Mental model

| Concept | Analogy |
|---------|---------|
| `Box<T>` | A storage unit with one key. Hand over the key = move the box. |
| `Drop` | The automatic "lights off, door locked" when the last person leaves a room |
| Lock guard | A bathroom key on a big wooden block: while you hold the block, nobody else gets in; put it down (drop) and it's free |
| `Rc<T>` | Several TV remotes for one TV; the TV turns off when the last remote is thrown away |
| `Arc<T>` | Same, but the remotes work from any room (thread) |
| `RefCell<T>` | A sign-out sheet checked at runtime; break the rules and an alarm (panic) goes off |
| Atomic `fetch_add` | A ticket dispenser: everyone gets a different number, no queue manager needed |
| `LazyLock` | A vending machine that is only stocked the first time someone presses a button |

**What the compiler guarantees:** memory is freed exactly once (no leaks via
forgetting, no double free); shared ownership never hands out `&mut` without
a lock or runtime check; non-thread-safe types (`Rc`, `Cell`, `RefCell`) can't
be shared across threads (E0277).

## Common misconceptions

| Wrong model | Correct model | Error |
|-------------|---------------|-------|
| "Recursive enums just work" | A type containing itself has infinite size; box the recursion | E0072 |
| "`Arc::clone` deep-copies" | It bumps an atomic counter; the data is shared | — |
| "`Arc<T>` lets me mutate T" | `Arc` gives `&T`; add `Mutex`/`RwLock`/atomics for mutation | E0596 |
| "The lock is released when I'm done using the data" | It's released when the **guard** is dropped (end of scope/statement) | deadlock / contention |
| "`let _ = mutex.lock()` holds the lock" | `_` drops immediately; `_guard` holds it | — |
| "`Relaxed` is unsafe" | It's always *memory-safe*; it only weakens cross-variable visibility | — |
| "I can put a `Vec` in a `static`" | Only const initialisers; use `LazyLock` | E0015 |
| "`Rc` is fine, my code is single-threaded" | tokio's runtime is multi-threaded; spawned futures must be `Send` | E0277 |

## Smells and idioms

| Smell | Idiomatic |
|-------|-----------|
| `Arc<Mutex<T>>` everywhere | Message passing (channels) or clear ownership often simpler |
| `Rc` when there's a single owner | Plain ownership or `Box` |
| `Mutex<bool>` / `Mutex<usize>` | `AtomicBool` / `AtomicUsize` |
| Holding a guard while doing slow work | Snapshot/clone out, drop the guard, then work |
| `lock().unwrap()` | `lock().expect("lock poisoned")` — says why it's fine to panic |
| `lazy_static!` in new code | `std::sync::LazyLock` / `OnceLock` |
| `RefCell::borrow_mut` where a runtime conflict is possible | `try_borrow_mut` and handle the error |

## In the databroker

- `databroker/src/broker.rs:128`–`132` — `Database` with `next_id: AtomicI32`.
- `databroker/src/broker.rs:1326` — `self.db.next_id.fetch_add(1, Ordering::SeqCst)`.
- `databroker/src/broker.rs:179`–`186` — `DataBroker`: `#[derive(Clone)]` over
  `Arc<RwLock<…>>` fields.
- `databroker/src/broker.rs:218`, `:235` — `Arc<dyn ActuationProvider + Send + Sync + 'static>`
  and `Arc<dyn SignalProvider>`.
- `databroker/src/broker.rs:1685`, `:1712` and `:2070`, `:2100` — providers
  arrive as `Box<dyn …>` and are converted with `Arc::from(...)`.
- `databroker/src/broker.rs:2210`–`2231` — snapshot providers inside a block
  so the read guard drops before any provider is called.
- `databroker/src/broker.rs:2362`–`2367` — `start_housekeeping_task` clones
  the `Arc`s, then `tokio::spawn(async move { … })` owns the clones.
- `databroker/src/broker.rs:892`–`908` — `cleanup_change_subscriptions`: a
  dropped receiver (RAII) is how the broker learns a subscriber left.
- `databroker/src/permissions.rs:21` — `lazy_static!` `ALLOW_ALL` / `ALLOW_NONE`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:59`–`64` — `Provider` holds a
  `tokio::sync::Mutex<Option<BroadcastStream<…>>>` and an `AtomicU32`
  request-id counter (`fetch_add(1, Ordering::Relaxed)` at `:174`).

## Exercises

Some tests in this chapter run code on several threads with
`std::thread::scope` or `std::thread::spawn`. Read those as "run this closure on
another thread at the same time". Chapter 13 covers threads properly.

| Exercise | Goal |
|----------|------|
| `c12_e01_box_recursive` | Fix E0072 with `Box`; recursive `matches`/`depth` |
| `c12_e02_drop_raii` | Implement `Drop` for a subscription handle; drop order; `drop(x)` |
| `c12_e03_rc_refcell` | Shared single-threaded subscribers; `try_borrow` instead of panicking |
| `c12_e04_arc_shared` | `Arc` fan-out and `Arc<dyn Provider>` snapshots; `strong_count`, `ptr_eq` |
| `c12_e05_mutex_rwlock` | `RwLock`/`Mutex` behind `&self`; snapshot; no leaked guards |
| `c12_e06_atomics` | Replace `Cell` with atomics; `fetch_add`; Relaxed vs Acquire/Release |
| `c12_e07_oncelock_lazylock` | `LazyLock` for `ALLOW_ALL`; `OnceLock` config loaded exactly once |
| `c12_e08_weak_parent_links` | A VSS tree with `Weak` parent links; prove no cycle with the counts |

## Further reading

- The Rust Book, ch. 15 "Smart Pointers" (Box, Deref, Drop, Rc, RefCell) —
  https://doc.rust-lang.org/book/ch15-00-smart-pointers.html
- The Rust Book, 16.3 "Shared-State Concurrency" (Mutex, Arc) —
  https://doc.rust-lang.org/book/ch16-03-shared-state.html
- `std::sync::atomic` module docs, "Memory model" section —
  https://doc.rust-lang.org/std/sync/atomic/
- Mara Bos, *Rust Atomics and Locks*, ch. 1–3 (free online) —
  https://marabos.nl/atomics/
- `std::sync::LazyLock` / `OnceLock` docs.

## Check yourself

1. Why does `enum Filter { Not(Filter) }` not compile, and why does `Box`
   fix it?
2. In `get_values_broker`, why is the provider snapshot built inside an
   extra `{ … }` block?
3. How does the databroker find out that a subscriber disconnected?
4. What does `Arc::clone(&broker_db)` cost? What doesn't it do?
5. Why is `Ordering::Relaxed` sufficient for a request-id counter but not
   for a "config is ready" flag?
6. What's the difference between `let _ = m.lock()` and `let _g = m.lock()`?
7. In `c12_e07`, why can't `ALLOW_ALL` be a plain `static`? Does the same
   reason apply to the databroker's `ALLOW_ALL`?

<details><summary>Answers</summary>

1. The type would contain itself, so its size would be infinite (E0072). A
   `Box<Filter>` is a fixed-size pointer; the inner value lives on the heap.
2. To drop the read guard (release the subscriptions lock) *before* calling
   providers, which may be slow or hang.
3. The client's stream is dropped → the `broadcast::Receiver` is dropped →
   `receiver_count()` becomes 0 → housekeeping removes the subscription.
4. One atomic increment. It does not copy the `RwLock<Database>`; both handles
   point to the same allocation.
5. For ids only uniqueness matters, which atomicity guarantees. A "ready"
   flag publishes *other* data; Release/Acquire ensures that data is visible
   to a thread that sees the flag.
6. `_` drops the guard immediately (lock released at once); `_g` binds it and
   keeps the lock until the end of the scope.
7. In the exercise the initialiser calls `Permissions::all()`, which builds
   a `Vec<String>` — not a `const` expression (E0015). The databroker's
   `ALLOW_ALL` is only `None` + unit enum variants, which *are* const, so it
   could be a plain `static`; `lazy_static` there is historical, not required.

</details>

## Where this goes next

Chapter 15 covers what these types look like in memory (fat pointers, niches, `Arc<str>`). Chapter 21 covers the async locks and `downgrade()`, and **chapter 25** covers atomics, compare-and-swap loops and memory ordering. Chapter 26 shows the `unsafe` code inside types like `Vec` and `Arc`.
