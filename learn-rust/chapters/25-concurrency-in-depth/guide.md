# 25 — Concurrency in depth

**Why this matters for the databroker.** The databroker is a concurrent
program. Tokio's worker threads run its tasks in parallel, the database sits
behind an `RwLock`, ids come from atomics, and providers are reached through
channels. Chapter 13 introduced threads, `Arc<Mutex<T>>`, channels and
`Send`/`Sync`, and chapter 12 introduced atomics. This chapter goes to the
depth needed to read and write *any* concurrent Rust (lock-free structures,
thread pools, rayon-style parallelism, concurrent maps), and to review
concurrent code for deadlocks and ordering bugs. Everything here uses
threads; the same ideas apply unchanged inside async code.

---

## Scoped threads and disjoint borrows

`std::thread::scope` creates threads that are **guaranteed to finish before
the scope returns**. That guarantee is why they may borrow local data
without `Arc` or `'static`:

```rust
thread::scope(|s| {
    for chunk in values.chunks_mut(size) {   // disjoint &mut pieces
        s.spawn(move || for v in chunk { *v *= factor });
    }
});                                           // every thread has joined here
```

Two things are new compared with chapter 13:

- **Many writers at once, with no lock.** `chunks_mut` and `split_at_mut`
  hand out `&mut` slices that *provably* don't overlap. The borrow checker
  accepts several threads each writing its own piece, because there's no
  shared mutable data to race on.
- **Scoped threads return values.** `s.spawn(..)` returns a
  `ScopedJoinHandle<T>`, and `join()` gives back the closure's result.
  Multi-phase algorithms (compute per-chunk maxima, combine them, then
  scale) become straightforward code.

When threads need to combine counts, **merge private results at the end**
instead of sharing counters. Thousands of threads incrementing one atomic
fight over a single cache line; per-thread arrays merged once don't.

## Condvar: waiting for a condition

A `Mutex` makes changes to shared state safe, but it can't make a thread
*sleep until the state changes*. A `Condvar` (condition variable) adds that:

```rust
let guard = state.lock().unwrap();
let mut guard = not_empty
    .wait_while(guard, |s| !s.closed && s.items.is_empty())
    .unwrap();
// the condition is false and the lock is held: safe to use `guard`
```

- `wait` **atomically** releases the mutex and puts the thread to sleep; it
  re-acquires the mutex before returning. There's no window in which a
  notification could be missed.
- **Spurious wake-ups** are allowed: a thread can wake with nothing changed.
  Always re-check the condition in a loop. `wait_while` does that for you.
- `notify_one` wakes one waiter, which is right when one event can satisfy
  one waiter (one item was pushed). `notify_all` wakes everyone, which is
  right when the change affects *all* waiters, such as `close()`. If you use
  `notify_one` on close, some threads sleep for ever.
- `wait_timeout_while` adds a deadline.

A bounded queue with `not_full` and `not_empty` condvars (`c25_e02`) is the
blocking twin of tokio's `mpsc::channel(10)`, whose `send().await` waits
while the queue is full (`databroker/src/grpc/kuksa_val_v2/val.rs:929`).
Tokio's own thread parker, which a worker sleeps on when it has no work, is
a `Mutex` + `Condvar` (`tokio-1.53.1/src/runtime/park.rs:24`).

## Compare-and-swap loops

Atomics offer a few ready-made read-modify-write operations: `fetch_add`,
`fetch_sub`, `fetch_or`, `fetch_max`, `swap`. For **any other update** you
use compare-and-swap (CAS):

```rust
let mut current = a.load(Relaxed);
loop {
    let new = f(current);
    match a.compare_exchange_weak(current, new, AcqRel, Relaxed) {
        Ok(_) => break,                  // nobody changed it in between
        Err(actual) => current = actual, // someone did: retry with their value
    }
}
```

- `compare_exchange(expected, new, ..)` writes `new` **only if** the value is
  still `expected`, and reports what it found. If another thread got there
  first you retry with the fresh value, so no update is ever lost.
- The `_weak` form may fail spuriously (cheaper on ARM). Use it inside loops.
  Use the strong form for a single attempt ("set if unset", `c25_e03`).
- `fetch_update(set_order, fetch_order, |old| Option<new>)` packages the
  loop. Return `None` to give up, as in a permit counter that must not go
  below zero.
- **The ABA problem.** CAS only checks the *value*. If it went A → B → A
  meanwhile, CAS succeeds even though the world changed. That's harmless for
  numbers, but dangerous for lock-free pointer structures: a node was freed
  and its address reused. This is why real lock-free code uses epoch-based
  reclamation (crossbeam-epoch) or hazard pointers.

The crossbeam channel (`crossbeam-channel-0.5.17/src/flavors/array.rs:194`)
is built from `compare_exchange_weak` loops on head and tail indices.

## Memory ordering in practice

Compilers and CPUs reorder memory operations. Each atomic operation names an
`Ordering` that limits how much:

| Ordering | Guarantee | Typical use |
|---|---|---|
| `Relaxed` | The operation on *this variable* is atomic. There's no ordering with other memory. | Counters, unique ids, statistics |
| `Release` (store) | All my earlier writes are visible to whoever Acquire-loads this value | Publishing: "the data is ready" |
| `Acquire` (load) | After seeing a Release-stored value, I see everything written before it | Consuming published data |
| `AcqRel` | Both, for read-modify-write operations | CAS on shared structures, permit counters |
| `SeqCst` | Acquire + Release + one global order of all `SeqCst` operations | Several flags that must be observed consistently |

The **publication pattern** is the one to know by heart:

```rust
// writer                                   // reader
data.store(42, Relaxed);                    if ready.load(Acquire) {
ready.store(true, Release);  ──sync──►          assert_eq!(data.load(Relaxed), 42);
                                            }
```

The Release store and the Acquire load of the *same* variable form a
"happens-before" edge. Everything the writer did before the Release is
visible to the reader after the Acquire.

Where `SeqCst` is really needed: in `thread 1: x = 1; read y` /
`thread 2: y = 1; read x`, Acquire/Release allows **both** threads to read 0.
A store to one variable followed by a load of *another* can be reordered.
Only `SeqCst` forbids that outcome. That's the heart of Dekker/Peterson-style
algorithms.

**Testing can't prove orderings correct.** x86 hardware is strongly ordered
and hides most `Relaxed` bugs, while ARM (for example many in-vehicle ECUs)
exposes them rarely. Reason about orderings, keep the atomic code tiny, and
use tools: `loom` explores every interleaving of a model of your code, and
Miri detects data races (chapter 32).

*Databroker:* `next_request_id.fetch_add(1, Relaxed)`
(`databroker/src/grpc/kuksa_val_v2/val.rs:174`) is exactly right.
`next_id.fetch_add(1, SeqCst)` (`databroker/src/broker.rs:1326`) is stronger
than necessary, and it runs under the database write lock anyway. It's not
a bug, just a small inconsistency worth noticing in review.

## Deadlocks and lock ordering

A deadlock needs four things: mutual exclusion, holding one lock while
waiting for another, no pre-emption, and a **cycle** of waiters. Rust
prevents data races, but **not** deadlocks: they compile fine.

```
thread 1: lock(A) ... lock(B)   ┐  each holds one, waits for the other,
thread 2: lock(B) ... lock(A)   ┘  for ever
```

There are three standard defences, in order of preference:

1. **Don't hold two locks.** Copy what you need, release the lock, then take
   the next one. This is the snapshot pattern from chapter 21.
2. **A global lock order.** If code must hold A and B together, *every* path
   locks them in the same order: by id, by address, or by a documented
   hierarchy. A cycle becomes impossible (`c25_e05` `transfer`). Watch the
   edge case: locking the *same* `Mutex` twice on one thread also deadlocks,
   because `std::sync::Mutex` isn't re-entrant.
3. **`try_lock` with back-off.** When the order can't be known ahead of time,
   take the first lock, `try_lock` the second, and if it fails, release
   everything, yield and retry. No thread ever waits while holding a lock.
   The trade-off is possible livelock under heavy contention; add randomised
   back-off in production.

*Databroker:* `update_entries` (`databroker/src/broker.rs:1529`) takes the
**database** lock and then, while still holding it (downgraded to a read
lock), the **subscriptions** lock. Any path that needs both must use the same
order. `subscribe` (`databroker/src/broker.rs:1601`) avoids the question by
releasing the database lock before it takes the subscriptions write lock. A
good review question for any change in `broker.rs` is: "which locks does this
hold at the same time, and in what order?"

Deadlocks are also why exercise tests in this chapter run the risky code on a
worker thread and wait with `recv_timeout`. A test that could deadlock must
fail, never hang.

## Data parallelism by hand

**Static scheduling** splits the input into `threads` equal parts up front.
It's simple, and it's perfect when every item costs the same. It performs
badly when costs vary: one thread gets the slow items while the others sit
idle.

**Dynamic scheduling** hands out small chunks on demand: each worker loops
`let i = next.fetch_add(1, Relaxed)` and processes chunk `i` until none are
left. Fast workers simply take more chunks. The atomic only hands out unique
indices, so `Relaxed` is enough: the results travel back through `join()`,
which synchronises on its own.

**Order** is restored at the end: each worker returns `(chunk_index, results)`
and the caller sorts. That's the same "fan out, then reassemble in request
order" shape as `get_values_broker` (`databroker/src/broker.rs:2254`,
`:2323`).

**rayon** (the standard data-parallelism crate) does this with **work
stealing**: each worker has its own deque of tasks, splits work recursively
(`join`), and idle workers steal from the far end of busy workers' deques.
`items.par_iter().map(f).collect()` gives dynamic load balancing, order
preservation and a reusable thread pool. Use rayon in real code; the
exercise shows what it does for you.

## Choosing: Mutex, RwLock or channels

The same key-value store written three ways (`c25_e07`):

| Design | Wins when | Costs |
|---|---|---|
| `Mutex<HashMap>` | Short critical sections, mixed reads and writes, simplest code | Readers block each other |
| `RwLock<HashMap>` | Many concurrent readers, rare writers (the databroker's `Database`) | Heavier than a `Mutex`; writers can starve or be starved depending on the implementation; **read-modify-write still needs the write lock** |
| Actor thread + channel | State has one natural owner; operations are complex or must be serialised; the owner also does I/O | A message round-trip per operation; every access needs a reply channel |
| Atomics | The state is one number or flag | Only simple operations |
| Sharded locks (for example `dashmap`) | Very high contention on one map | More complex; iteration and snapshots get harder |

The databroker uses **both** styles, and each for a reason. The signal
`Database` is shared state behind `Arc<RwLock<..>>` inside `DataBroker`
(`databroker/src/broker.rs:179`), because reads (get, subscribe
notifications) far outnumber writes. Each provider is reached by **message
passing**: the `Provider` struct (`databroker/src/grpc/kuksa_val_v2/val.rs:59`)
holds a channel `Sender`, because the provider's state lives in another
process and the channel serialises requests.

`parking_lot` offers faster, smaller `Mutex`/`RwLock` types without
poisoning (`parking_lot-0.12.5/src/mutex.rs:86`). `dashmap` shards one map
across many `RwLock`s (`dashmap-6.2.1/src/lib.rs:91`).

---

## Mental model

| Concept | Analogy |
|---|---|
| Scoped threads | Contractors who must leave the building before you lock up, so they can borrow your tools |
| Condvar | A waiting-room buzzer: sleep until called, then check whether it's really your turn |
| CAS loop | "Update the whiteboard only if it still says what I read; otherwise re-read and try again" |
| Release/Acquire | Sealing a parcel (Release) and opening it (Acquire): everything packed before sealing is inside |
| Lock ordering | Everyone climbs the stairs in the same direction, so nobody meets head-on |
| Work stealing | Checkout lanes where idle cashiers take customers from long queues |
| Actor | A librarian: nobody touches the shelves, everyone asks at the desk |

**Guarantees:** safe Rust gives you **no data races** (`Send`/`Sync` and the
borrow rules). It does **not** guarantee freedom from deadlocks, livelocks,
lost wake-ups, starvation or wrong atomic orderings. Those are
logic errors that you must design against.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Rust prevents deadlocks." | It prevents data races only. Deadlocks compile and run until they hang. |
| "`Condvar::wait` returns only when notified." | Spurious wake-ups happen, so always re-check the condition (`wait_while`). |
| "`notify_one` is always enough." | Use `notify_all` when the change affects every waiter, such as closing. |
| "`SeqCst` everywhere is the safe choice." | It's correct but can hide intent and costs extra. Prefer the weakest *correct* ordering and document why. |
| "Tests passed on my laptop, so the ordering is right." | Strong x86 hides most ordering bugs. Reason about it, or use loom/Miri. |
| "`RwLock` read then write is fine for an increment." | Another writer can slip in between, and the update is lost. Do it under one write guard. |
| "Locking the same `Mutex` twice is fine on one thread." | `std::sync::Mutex` isn't re-entrant, so it deadlocks (or panics). |
| "Threads always speed things up." | Contention on one lock or cache line can make it slower than one thread. |

## Smells and idioms

- **A shared counter per item** in a parallel loop. Accumulate locally and
  merge once.
- **`Condvar::wait` without a loop.** Use `wait_while` or
  `wait_timeout_while`.
- **Waking while holding the lock you just changed** is fine for `Condvar`,
  but not for arbitrary callbacks or wakers (chapter 24).
- **Two locks held in different orders on different paths.** Pick an order,
  document it, and review every path that needs both.
- **`Arc<Mutex<T>>` around a single integer or flag.** Use an atomic.
- **Hand-rolled lock-free pointer structures.** Use crossbeam or a
  well-reviewed crate. ABA and memory reclamation are subtle.
- **`thread::sleep` to "wait for" another thread** in tests. Use a
  `Barrier`, a channel or a join, so the test is deterministic.
- *Databroker note:* `next_id` uses `SeqCst` while `next_request_id` uses
  `Relaxed`. Both work, but consistency (and a comment on *why*) helps the
  next reader.

## In the databroker and in the wild

Databroker:
- `databroker/src/main.rs:418`: `worker_threads`, the thread pool that
  provides real parallelism.
- `databroker/src/broker.rs:179`: `DataBroker` with its
  `Arc<RwLock<Database>>` and `Arc<RwLock<Subscriptions>>`.
- `databroker/src/broker.rs:1326`: `next_id.fetch_add(1, Ordering::SeqCst)`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:174`:
  `next_request_id.fetch_add(1, Ordering::Relaxed)`.
- `databroker/src/broker.rs:1529` (`update_entries`) and `:1601` (`subscribe`):
  the order in which locks are taken and released.
- `databroker/src/broker.rs:2254` and `:2323`: fan out, then restore the
  order.
- `databroker/src/grpc/kuksa_val_v2/val.rs:59` (`Provider`) and `:929`
  (bounded `mpsc::channel(10)`): message passing to a peer that owns its state.

In the wild (under `~/.cargo/registry/src/index.crates.io-*/`):
- `tokio-1.53.1/src/runtime/park.rs:24` and `:59`: a `Mutex` + `Condvar`
  parker.
- `crossbeam-channel-0.5.17/src/flavors/array.rs:194`: a
  `compare_exchange_weak` loop in a real bounded channel.
- `parking_lot-0.12.5/src/mutex.rs:86`: `parking_lot::Mutex`.
- `dashmap-6.2.1/src/lib.rs:91`: `shards: Box<[CachePadded<RwLock<HashMap<K, V>>>]>`,
  a sharded concurrent map.
- rayon isn't among this course's dependencies. Read it on docs.rs
  (`rayon::join` and `rayon-core`'s work-stealing registry).

## Exercises

| Exercise | Goal |
|---|---|
| `c25_e01_scoped_threads` | Lock-free parallel writes to disjoint chunks; two-phase normalisation; private histograms |
| `c25_e02_condvar` | A blocking bounded MPMC queue with close and timeouts |
| `c25_e03_atomics_cas_loop` | CAS max, a permit counter with `fetch_update`, first-writer-wins |
| `c25_e04_memory_ordering` | Publish data with Release/Acquire; choose the weakest correct ordering |
| `c25_e05_deadlock_and_lock_ordering` | Deadlock-free transfers by lock order, and with `try_lock` back-off |
| `c25_e06_data_parallelism_by_hand` | Dynamic scheduling with an atomic chunk counter; order-preserving `par_map` |
| `c25_e07_rwlock_vs_mutex_vs_channels` | One store, three designs (`Mutex`, `RwLock`, actor thread) |
| `c25_e08_review_ordering_and_lock_order` | Review: 2 planted bugs — a `Relaxed` publish flag, and a transfer that deadlocks on lock order |

## Further reading

- *Rust Atomics and Locks* by Mara Bos (free online), especially chapters 2–3
  (atomics and memory ordering) and 9 (building locks):
  <https://marabos.nl/atomics/>
- The Rust Book, chapter 16, "Fearless Concurrency"
- `std::sync::Condvar` docs and the `std::sync::atomic` module docs,
  including the "Memory model" section
- The Rustonomicon, "Atomics": <https://doc.rust-lang.org/nomicon/atomics.html>
- rayon docs (`join` and parallel iterators): <https://docs.rs/rayon>
- loom (model checking concurrent code): <https://docs.rs/loom>
- *Rust for Rustaceans*, chapter 10, "Concurrency (and Parallelism)"

## Check yourself

1. Why can several scoped threads hold `&mut` references into the same `Vec` at once?
2. Why must `Condvar` waits sit in a loop, and when do you need `notify_all` rather than `notify_one`?
3. In a CAS loop, what does `Err(actual)` from `compare_exchange_weak` tell you, and what do you do next?
4. Why is `Relaxed` correct for a request-id counter but wrong for a "config ready" flag?
5. Name three ways to avoid a two-lock deadlock.
6. Why is dynamic scheduling better than static splitting when item costs vary?
7. Why does the databroker use an `RwLock` for its database but channels to reach providers?

<details><summary>Answers</summary>

1. They come from `chunks_mut` / `split_at_mut`, which produce provably
   non-overlapping slices, so no two threads can touch the same element. The
   scope guarantees every thread ends before the `Vec` can be used again.
2. Because of spurious wake-ups, and because another thread may have
   consumed the event first, the condition must be re-checked. Use
   `notify_all` when the change matters to every waiter, such as closing the
   queue.
3. Another thread changed the value first. `actual` is the current value:
   recompute the new value from it and try again.
4. Unique ids depend only on the counter itself, so atomicity is enough. A
   ready flag *publishes other data*: without Release on the store and
   Acquire on the load, a reader could see `true` but stale data.
5. Don't hold two locks at once (snapshot and release); always lock in one
   global order; use `try_lock` and release everything on failure, then retry
   with back-off.
6. Workers pull small chunks on demand, so fast workers take more work, and
   no worker is stuck with all the slow items while others sit idle.
7. The database is local shared state that is read far more than written,
   which suits many concurrent readers behind an `RwLock`. A provider's state
   belongs to another process, so requests are serialised as messages over
   its channel/stream.
</details>
