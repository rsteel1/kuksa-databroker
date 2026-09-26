# 13 — Threads, Send & Sync

**Why this matters for the databroker.** The databroker never calls
`std::thread::spawn` for its main work — it uses Tokio tasks (chapter 20).
But Tokio's default runtime is **multi-threaded**: `main.rs` builds it with
one worker thread per CPU core, and any task can be resumed on any of them.
So every rule in this chapter applies: data moved into a task must be
`Send`, data shared between tasks must be `Sync`, and that's why the
databroker's provider traits read `SignalProvider: Send + Sync + 'static`.
Learning these rules on plain threads first — where there's no `async` noise —
makes the async compiler errors later much less mysterious.

---

## Spawning threads: move and join

```rust
use std::thread;

let handle = thread::spawn(move || {
    // runs on a new OS thread
    42
});
let answer = handle.join().expect("thread panicked");   // wait + get the result
```

- `thread::spawn` takes a closure and returns a `JoinHandle<T>`, where `T` is
  what the closure returns.
- `join()` blocks until the thread finishes and returns `Result<T, _>` —
  `Err` if the thread panicked.
- The closure must be **`'static`**: the new thread might outlive the
  function that spawned it, so it can't borrow that function's locals
  (E0373). Use `move` to transfer ownership into the thread.
- Dropping a `JoinHandle` doesn't stop the thread; it *detaches* it.

**Scoped threads** lift the `'static` requirement:

```rust
thread::scope(|s| {
    s.spawn(|| left.iter().max());     // may borrow `left`!
    s.spawn(|| right.iter().max());
});                                     // every scoped thread is joined here
```

Because `scope` doesn't return until all its threads have finished, borrowing
locals is safe — the compiler can prove the data outlives the threads.

## Shared state: Arc<Mutex<T>>

Threads that need the same mutable data combine chapter 12's pieces:

```rust
let counts = Arc::new(Mutex::new(HashMap::new()));
for batch in batches {
    let counts = Arc::clone(&counts);             // 1. clone the handle
    thread::spawn(move || {                       // 2. move the clone in
        for path in batch {
            *counts.lock().expect("poisoned")     // 3. lock briefly
                .entry(path).or_insert(0) += 1;
        }                                          //    guard dropped each iteration
    });
}
```

That `let x = Arc::clone(&x);` *shadowing* line right before a `move`
closure is the most common idiom in concurrent Rust. The databroker does it
in `start_housekeeping_task`:

```rust
let subscriptions = self.subscriptions.clone();   // databroker/src/broker.rs:2364
let filter_manager = self.filter_manager.clone();
tokio::spawn(async move { … });                   // :2367
```

After all threads are joined, only one `Arc` is left, so
`Arc::try_unwrap(counts)` succeeds and gives back the `Mutex` (and
`into_inner()` the data) without cloning.

## Message passing with channels

The alternative to sharing memory is **sending ownership**:

```rust
let (tx, rx) = std::sync::mpsc::channel();   // multi-producer, single-consumer
let tx2 = tx.clone();
thread::spawn(move || tx.send(("Vehicle.Speed".to_string(), 50)).expect("receiver alive"));
thread::spawn(move || tx2.send(("Vehicle.Horn".to_string(), 1)).expect("receiver alive"));
for (path, value) in rx {    // ends when EVERY sender is dropped
    …
}
```

- `send` moves the value into the channel; the receiver becomes its owner.
  No locks, no shared mutable state.
- `send` fails only if the receiver is gone; `recv` returns `Err` (and the
  `for` loop ends) only when **all** senders are gone.
- **The #1 channel bug:** keeping the original `tx` alive in the receiving
  function. The loop never ends; your program hangs. `drop(tx)` after
  cloning it into the producers.

The databroker uses the async versions (`tokio::sync::mpsc`,
`tokio::sync::broadcast`) with exactly the same ownership rules. In
`open_provider_stream` it creates `mpsc::channel(10)`
(`databroker/src/grpc/kuksa_val_v2/val.rs:929`); tonic owns the receiving end
as the response stream, and the spawned task (`:933`) and the `Provider`
object own senders. When the provider disconnects and everything drops, the
channel closes — and `Provider::is_available()` is literally
`!self.sender.is_closed()`.

Channels vs. locks: prefer channels when data flows one way (producers →
consumer), locks when many parties read and update one shared structure
(like the database).

## Send and Sync

Two **marker traits** (no methods) describe thread-safety:

| Trait | Meaning | Not implemented by |
|-------|---------|--------------------|
| `Send` | A value of this type may be **moved** to another thread | `Rc<T>`, raw pointers, `MutexGuard` |
| `Sync` | A **`&T`** may be shared between threads (T is `Sync` ⇔ `&T` is `Send`) | `Rc<T>`, `Cell<T>`, `RefCell<T>` |

They're **auto traits**: a struct is `Send`/`Sync` automatically iff all its
fields are. You almost never implement them by hand (doing so needs
`unsafe`).

Common combinations:

| Type | Send | Sync | Why |
|------|:----:|:----:|-----|
| `i32`, `String`, `Vec<T: Send>` | ✓ | ✓ | plain owned data |
| `Rc<T>` | ✗ | ✗ | non-atomic reference count |
| `Arc<T>` (T: Send + Sync) | ✓ | ✓ | atomic count |
| `RefCell<T>` | ✓ | ✗ | runtime borrow flag isn't atomic |
| `Mutex<T>` (T: Send) | ✓ | ✓ | the lock serialises access |
| `Arc<Mutex<T>>` | ✓ | ✓ | the standard shared-mutable combo |

`thread::spawn` requires `F: Send + 'static` (and its result `T: Send`).
When you break the rule, E0277 names the culprit: "`Rc<Vec<String>>` cannot
be sent between threads safely".

**Trait objects don't inherit thread-safety automatically.** `dyn Provider`
could be *any* type implementing `Provider`, including an `Rc`-based one, so
the compiler assumes nothing. You must say it:

```rust
Arc<dyn Provider + Send + Sync>                 // at each use site, or…
trait Provider: Send + Sync { … }               // …once, as a supertrait
```

The databroker uses both styles:

```rust
pub trait SignalProvider: Send + Sync + 'static { … }            // broker.rs:198
actuation_provider: Arc<dyn ActuationProvider + Send + Sync + 'static>,  // :218
```

- `Send` — the provider may be moved into a task on another worker thread.
- `Sync` — several tasks may call `&self` methods concurrently through the
  shared `Arc`.
- `'static` — it holds no short-lived borrows, so it can be stored in the
  long-lived subscriptions map.

## Checkpoint

`c13_e05_checkpoint_part1` closes Part I. It has no step-by-step section:
the signatures and tests are the whole specification, which is what real
work looks like. It draws on:

| You'll need | From chapter |
|---|---|
| `trim`, `split_once`, `parse`, `starts_with` | 01, 06 |
| Owned `String` vs borrowed `&str`, moving data into threads | 02, 13 |
| Structs, enums with data, `Display` | 03, 04, 09 |
| `BTreeMap`/`HashMap`, the entry API | 05 |
| Collecting *all* errors instead of stopping at the first | 07 |
| Returning `impl Iterator` from a method | 10 |
| An iterator that borrows two things at once (`'a` on both) | 11 |
| `Arc<Mutex<_>>`, `Arc::try_unwrap`, `into_inner` | 12, 13 |

One requirement is subtle on purpose: the result must not depend on which
thread runs first. Before you write any code, ask yourself: *if two threads
update the same path, which value should win, and how will my code know?*
If the tests pass only sometimes, you have a race. Rust prevents data races,
not logic races.

Work through it like a real task: read every test first, write the types,
then make the tests pass one at a time. Use the hints only for strategy.

## Mental model

| Concept | Analogy |
|---------|---------|
| `thread::spawn` + `move` | Hiring a contractor: hand over the materials (ownership); they may still be working after you've gone home |
| `thread::scope` | A team meeting: nobody leaves until everyone is done, so you can share the whiteboard (borrow) |
| `join` | Waiting at the door for the contractor to hand you the result |
| Channel | A conveyor belt: items change owner as they move; the belt stops when every loader has left |
| `Send` | "Can be shipped to another office" |
| `Sync` | "Can be looked at from several offices at once" |

**What the compiler guarantees:** no data races — ever — in safe Rust. Two
threads can't access the same memory with at least one writing unless a
synchronisation primitive mediates it. This is checked entirely at compile
time through `Send`/`Sync`.

## Common misconceptions

| Wrong model | Correct model | Error |
|-------------|---------------|-------|
| "The thread will finish before my function returns" | `spawn` may outlive the caller; use `move` or `scope` | E0373 |
| "Add `.clone()` inside the closure" | Clone *before* the `move` closure, move the clone in | E0382 |
| "`Rc` is fine if I'm careful" | The compiler forbids it; use `Arc` | E0277 |
| "`Arc` makes my data thread-safe to mutate" | `Arc` shares; `Mutex`/atomics make mutation safe | E0596 / E0277 |
| "`dyn Trait` is Send if all my impls are" | Trait objects are only `Send`/`Sync` when declared | E0277 |
| "My receive loop hangs because of a deadlock" | Usually a sender you forgot to drop | (hang) |
| "Rust prevents all concurrency bugs" | It prevents data races; deadlocks and logic races are still possible | — |

## Smells and idioms

| Smell | Idiomatic |
|-------|-----------|
| `Arc<Mutex<T>>` for data flowing one way | A channel |
| `thread::spawn` + `join` just to borrow locals | `thread::scope` |
| `handle.join().unwrap()` | `.expect("worker panicked")` |
| `std::thread::sleep` / blocking calls inside async code | `tokio::time::sleep`, `spawn_blocking` (chapter 20) |
| Adding `Send + Sync` to every generic "just in case" | Add bounds where the value actually crosses threads |
| Implementing `unsafe impl Send` to silence an error | Fix the type (use `Arc`/`Mutex`) |

## In the databroker

- `databroker/src/main.rs:414`–`418` — `available_parallelism()` and
  `.worker_threads(*worker_threads)`: the multi-threaded Tokio runtime.
- `databroker/src/broker.rs:198` — `pub trait SignalProvider: Send + Sync + 'static`.
- `databroker/src/broker.rs:218` — `Arc<dyn ActuationProvider + Send + Sync + 'static>`.
- `databroker/src/broker.rs:2070` — `register_signals(…, signal_provider: Box<dyn SignalProvider + Send + Sync + 'static>)`.
- `databroker/src/broker.rs:2362`–`2367` — clone `Arc`s, then
  `tokio::spawn(async move { … })`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:929`–`933` — an mpsc channel whose
  sender is moved into a spawned task; `Provider::is_available` checks
  `sender.is_closed()` (`:104`–`106`).

## Exercises

| Exercise | Goal |
|----------|------|
| `c13_e01_spawn_join_move` | Fix E0373 with `move`; use `thread::scope` to borrow |
| `c13_e02_arc_mutex_counter` | Count updates from many threads with `Arc<Mutex<HashMap>>`; `try_unwrap` |
| `c13_e03_std_mpsc_channel` | Producers/consumer with `mpsc`; drop the extra sender |
| `c13_e04_send_sync_providers` | Turn an `Rc`/`RefCell` provider registry into a thread-safe one; `Send + Sync` supertrait |
| `c13_e05_checkpoint_part1` | **Checkpoint:** parse a log, apply it with worker threads, query a borrowing snapshot |

## Further reading

- The Rust Book, ch. 16 "Fearless Concurrency" —
  https://doc.rust-lang.org/book/ch16-00-concurrency.html
- The Rustonomicon, "Send and Sync" —
  https://doc.rust-lang.org/nomicon/send-and-sync.html
- `std::thread::scope` docs — https://doc.rust-lang.org/std/thread/fn.scope.html
- Mara Bos, *Rust Atomics and Locks*, ch. 1 "Basics of Rust Concurrency" —
  https://marabos.nl/atomics/basics.html

## Check yourself

1. Why must a closure passed to `thread::spawn` be `'static`, and why doesn't
   `thread::scope` need that?
2. Where do you put `Arc::clone` relative to the `move` closure, and why?
3. A consumer's `for msg in rx` loop never ends. What's the most likely cause?
4. Why is `RefCell<T>` `Send` but not `Sync`?
5. Why does the databroker write `SignalProvider: Send + Sync + 'static`?
   What does each bound buy?
6. Does Rust prevent deadlocks?

<details><summary>Answers</summary>

1. A spawned thread may outlive the spawning function, so borrowed locals
   could be freed while it runs. `scope` joins all its threads before
   returning, so borrows provably outlive them.
2. Before: `let x = Arc::clone(&x);` then `move || …` moves that clone in,
   leaving the original for the next iteration/thread.
3. A `Sender` is still alive — often the original `tx` in the receiving
   function. The channel only closes when all senders drop.
4. Moving a `RefCell` to another thread is fine (one owner at a time), but
   sharing `&RefCell` would let two threads update the non-atomic borrow flag
   concurrently.
5. `Send`: the provider can be moved to whichever worker thread runs a task.
   `Sync`: several tasks can call it through a shared `Arc` at once.
   `'static`: it holds no borrowed data, so it can live in the long-lived
   subscriptions map.
6. No. It prevents data races. Deadlocks (e.g. inconsistent lock order) are
   still possible and must be avoided by design.

</details>

## Where this goes next

**Chapter 25 (Concurrency in depth)** continues from here: scoped threads, `Condvar`, compare-and-swap loops, memory ordering, deadlocks and lock ordering, and choosing between locks and channels. The databroker itself runs on Tokio tasks rather than threads, which starts in chapter 20.
