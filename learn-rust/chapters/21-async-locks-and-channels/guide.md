# 21 — Async locks & channels

**Why this matters for the databroker.** The databroker's shared state — the
signal database and the table of subscriptions/providers — sits behind
`tokio::sync::RwLock`s. Everything that moves between tasks moves through
channels: provider requests/responses through `mpsc`, subscription updates
and the shutdown signal through `broadcast`. Almost every bug fixed in the
three most recent commits (e1fdfd1, 748a914, 7a210d0) is about *how long a
lock is held* and *what happens when the other side of a channel is slow*.
This chapter teaches both.

## Two ways to share: locks or messages

| Pattern | Idea | In the databroker |
|---|---|---|
| Shared state | `Arc<RwLock<T>>`: many tasks lock and touch the same data | `DataBroker { database: Arc<RwLock<Database>>, subscriptions: Arc<RwLock<Subscriptions>>, ... }` (`databroker/src/broker.rs:179`) |
| Message passing | One owner; others send it messages over channels | each provider connection task, each subscription's `broadcast::Sender` |

Chapter 13 introduced both patterns with threads (`Arc<Mutex<T>>` and
`std::sync::mpsc`). This chapter covers their async versions, and chapter 25
compares them in depth.

"Do not communicate by sharing memory; share memory by communicating" is a
good default, but a broker with a big lookup table legitimately needs shared
state. Use each where it fits.

## std vs tokio locks

| | `std::sync::Mutex` / `RwLock` | `tokio::sync::Mutex` / `RwLock` |
|---|---|---|
| Acquire | `lock()` blocks the **thread** | `lock().await` suspends the **task** |
| Guard `Send`? | No | Yes |
| Hold across `.await`? | No (compile error in spawned tasks) | Yes |
| Speed | Faster | Slower (async bookkeeping) |
| Poisoning | Yes (`lock()` returns `Result`) | No |

Rule of thumb (from the Tokio docs): **use `std::sync::Mutex` for short
critical sections that never contain an `.await`; use tokio's locks only when
the guard must live across an `.await`.** The databroker's `Database` guard is
held while subscribers are notified with async code, so it uses tokio's
`RwLock` (`databroker/src/broker.rs:21`).

An `RwLock` allows **many readers or one writer**. Tokio's `RwLock` is
*fair*: once a writer is waiting, new readers queue behind it — so a reader
that never finishes can starve all writers *and* all later readers.

## mpsc: many producers, one consumer

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel::<Msg>(10);  // capacity 10
tx.send(msg).await?;            // waits if the channel is full
tx.try_send(msg)?;              // never waits: Err(Full) / Err(Closed)
while let Some(msg) = rx.recv().await { ... }   // None: all senders dropped
```

* **Bounded = backpressure.** A slow consumer makes `send().await` wait, which
  slows the producer instead of growing memory without limit.
* `Sender` is `Clone`; the channel closes when the last `Sender` is dropped,
  and `recv()` then returns `None` after draining the buffer.
* `tx.is_closed()` tells a producer the receiver is gone. The databroker uses
  exactly that as the "is this provider still connected?" check
  (`databroker/src/grpc/kuksa_val_v2/val.rs:104`).

`open_provider_stream` creates `mpsc::channel(10)` for messages to the
provider (`databroker/src/grpc/kuksa_val_v2/val.rs:929`) and hands the
receiving end to tonic as the response stream.

## oneshot: exactly one reply

`tokio::sync::oneshot::channel()` carries a single value — the natural
"reply-to" slot for request/response between tasks:

```rust
enum Command { Get { path: String, reply: oneshot::Sender<Option<i32>> } }
let (reply, response) = oneshot::channel();
store.send(Command::Get { path, reply }).await?;
let value = response.await?;   // Err if the actor dropped `reply` without answering
```

This "actor" pattern — one task owns the state, others message it — avoids
locks entirely. The databroker's provider exchange is the same idea over the
network: request down one stream, response back on another, correlated by a
`request_id` (chapter 23).

## broadcast: fan-out and lag

`broadcast::channel(capacity)` delivers **every message to every receiver**.
New receivers come from `tx.subscribe()` and only see messages sent *after*
they subscribed. Values must be `Clone` (each receiver gets its own copy).

Crucially, **the sender never waits.** The channel is a ring buffer of
`capacity` slots. If a receiver falls more than `capacity` messages behind,
the oldest are overwritten, and that receiver's next `recv()` returns
`Err(RecvError::Lagged(n))` — "you missed `n` messages" — after which it
continues from the oldest message still buffered. `Err(RecvError::Closed)`
means all senders are gone.

```
capacity 2, sender sends 1 2 3 4 5, receiver hasn't read:
buffer holds [4, 5]  ->  recv: Lagged(3), then 4, then 5, then Closed
```

That is exactly the policy a broker wants: **one slow subscriber must never
slow down sensor updates for everyone else.** In `subscribe`
(`databroker/src/broker.rs:1611-1630`) the capacity is the client's requested
`buffer_size + 1` (default 1: "latest value only"), capped at
`MAX_SUBSCRIBE_BUFFER_SIZE` = 1000 (`databroker/src/broker.rs:38`). When a
subscriber lags, the broker logs "Slow subscriber with capacity {} lagged,
missed {} signal updates" (`databroker/src/broker.rs:1669`).

Two more broadcast facts used by the databroker:

* `tx.send(v)` returns `Err` if there are **no receivers** — often not an
  error for you (`let _ = tx.send(v);`).
* `tx.receiver_count()` — housekeeping removes a subscription when its count
  is 0, i.e. the client's stream was dropped (`databroker/src/broker.rs:895`).

## The shutdown trigger pattern

A `broadcast::Sender<()>` makes a perfect "everybody stop" signal:

```rust
// databroker/src/broker.rs
let (shutdown_trigger, _) = broadcast::channel::<()>(1);          // in DataBroker::new
pub fn get_shutdown_trigger(&self) -> broadcast::Receiver<()> {    // :2504
    self.shutdown_trigger.subscribe()
}
pub async fn shutdown(&self) {                                     // :2495
    self.subscriptions.write().await.clear();
    let _ = self.shutdown_trigger.send(());
}
```

Every long-running task subscribes and puts `shutdown.recv()` into its
`select!`. **Subscribe before spawning**, not inside the task: a receiver
created after `send(())` never sees it, so a task that subscribes late may run
forever. `open_provider_stream` does it right — `let mut shutdown_trigger =
self.get_shutdown_trigger();` happens before `tokio::spawn`
(`databroker/src/grpc/kuksa_val_v2/val.rs:924`).

For completeness: `tokio::sync::watch` (latest value only) and
`tokio_util::sync::CancellationToken` are common alternatives for shutdown.

## tokio RwLock and downgrade

`update_entries` (`databroker/src/broker.rs:1529`) must (1) apply updates and
then (2) notify every subscriber *about that exact state*. Holding the write
lock during (2) would block all readers while slow subscribers are served.
Dropping the write lock and re-acquiring a read lock would open a gap in
which another writer could change the data. The answer:

```rust
let mut db = self.broker.database.write().await;
// ... apply updates ...
let db = db.downgrade();                    // databroker/src/broker.rs:1559
self.broker.subscriptions.read().await.notify(Some(&changed), &db).await
```

`RwLockWriteGuard::downgrade` atomically turns the write guard into a read
guard: readers may enter immediately, writers still can't.

## Snapshot, then await

**Never hold a lock while awaiting something you don't control** — a provider,
a network send, a user callback. If it hangs, the lock is held forever, and
because tokio's RwLock is fair, one waiting writer then blocks *every* future
reader too. The whole broker freezes.

That's what commit **e1fdfd1** ("Make sure blocking/slow providers can not
globally block other databroker operations") fixed. Before:

```rust
let read_subscription_guard = self.broker.subscriptions.read().await;
let actuation_subscriptions = &read_subscription_guard.actuation_subscriptions;
// ...
actuation_subscription.actuation_provider.actuate(actuation_changes).await?  // guard still held!
```

After — copy what's needed under the lock, release, then await:

```rust
async fn snapshot_actuation_subscriptions(&self) -> Vec<ActuationSubscriptionSnapshot> {
    self.broker.subscriptions.read().await      // guard is a temporary...
        .actuation_subscriptions.iter()
        .map(|s| (s.vss_ids.clone(), s.actuation_provider.clone(), s.permissions.clone()))
        .collect()                               // ...dropped at the end of this statement
}
let actuation_subscriptions = self.snapshot_actuation_subscriptions().await;
actuation_provider.actuate(actuation_changes).await?   // no lock held
```

To make the snapshot cheap, the commit also changed providers from
`Box<dyn ActuationProvider>` to `Arc<dyn ActuationProvider>`: cloning an `Arc`
is a counter increment. See `databroker/src/broker.rs:1836`
(`snapshot_actuation_subscriptions`), `:2445` (`snapshot_signal_providers`)
and the `let providers = { ... }` block at `:2210` in `get_values_broker`.
Look at the commit yourself: `git show e1fdfd1 -- databroker/src/broker.rs`.

A snapshot can be slightly stale (a provider may disconnect right after you
copied it) — the code must tolerate that, e.g. by checking `is_available()`
and handling send errors. That's the price of not blocking.

## Deadlines across several steps

`timeout(1s, step)` bounds one step. When an operation has several awaits,
giving each its own 1 s lets the whole thing take N seconds. Use one
**deadline** and give each step what's left:

```rust
// databroker/src/grpc/kuksa_val_v2/val.rs:158 onwards
let deadline = std::time::Instant::now() + Duration::from_secs(1);
let guard = timeout(deadline.saturating_duration_since(Instant::now()), self.receiver.lock()).await;
// ... send with the remaining time ...
// ... and every receive in the loop with the remaining time
```

`saturating_duration_since` returns zero instead of panicking when the
deadline has already passed (and `timeout(Duration::ZERO, f)` still polls `f`
once, so an already-ready result isn't thrown away). In tests prefer
`tokio::time::Instant` so the paused clock applies.

## watch: only the latest value matters

`tokio::sync::watch` is a channel with **one slot**. `tx.send(v)` overwrites
the slot, and every receiver can read the current value or wait for a newer one:

| | broadcast | watch |
|---|---|---|
| Stores | the last *N* messages | only the latest value |
| Slow receiver | lags and loses the oldest (`Lagged(n)`) | sees just the newest; values in between are coalesced |
| Initial value | none | required (`watch::channel(initial)`) |
| Typical use | event streams where every update counts (databroker subscriptions with `buffer_size`) | state: config, health, "current value of X", shutdown flag |

Receiver API: `rx.borrow()` peeks (it holds a read lock, so copy or clone out
and don't keep it across `.await`); `rx.changed().await` waits for a value
this receiver hasn't marked seen; `rx.borrow_and_update()` reads and marks
it seen; `rx.wait_for(|v| predicate).await` waits until the predicate holds
(returning immediately if it already does). Once the sender is dropped,
`changed()` returns `Err`.

A `watch<bool>` is also a neat shutdown signal: unlike `broadcast`, a task
that subscribes *after* shutdown was triggered still sees `true`.

## Mental model

| Concept | Analogy |
|---|---|
| `Mutex` | A single-occupancy bathroom key |
| `RwLock` | A library room: many readers, or one person rearranging shelves |
| `downgrade` | The shelf-rearranger finishes and, without leaving, lets readers in |
| Lock across `.await` | Taking the bathroom key home for the weekend |
| Snapshot | Photocopying the pages you need, returning the book, reading at home |
| `mpsc` | A mailbox with N slots — the postie waits when it's full |
| `oneshot` | A self-addressed envelope for exactly one reply |
| `broadcast` | A radio station with a DVR buffer; fall too far behind and you skip ahead |
| Shutdown trigger | The fire alarm: one signal, everyone hears it — if they were in the building |

**Guarantee:** no data races — shared mutable data is only reachable through a
lock or a channel. **Not guaranteed:** no deadlocks, no starvation, no lock
held too long. Those are design responsibilities.

## Common misconceptions

| Wrong model | Correct model | You'll see |
|---|---|---|
| "Always use tokio's Mutex in async code" | std Mutex is better for short, await-free sections | needless slowness |
| "A read lock can't hurt anyone" | With a fair RwLock a stuck reader blocks a waiting writer and everyone after | the e1fdfd1 bug: global freeze |
| "broadcast send waits for slow receivers" | It never waits; slow receivers get `Lagged(n)` | missed updates |
| "`Lagged` means the receiver is dead" | It keeps working from the oldest buffered value | premature disconnects |
| "A new subscriber sees earlier messages" | Only messages sent after `subscribe()` | missed shutdown signal |
| "`send` error on broadcast is a bug" | It just means zero receivers right now | panics from `.unwrap()` |
| "Per-step timeouts bound the whole call" | They add up; use one deadline | 3× longer stalls |

## Smells and idioms

* A guard variable that lives across `.await`s on something external → snapshot.
* `Arc<Mutex<T>>` for a counter or flag → atomics (`AtomicU32`, `AtomicBool`).
* Unbounded channels (`unbounded_channel`) for data from untrusted peers → a
  bounded channel gives backpressure and caps memory.
* Lock ordering: if code ever holds two locks, always take them in the same
  order. The databroker takes `database` then `subscriptions` in `update_entries`.
* `last_emitted: Arc<RwLock<Instant>>` in `ChangeSubscription`
  (`databroker/src/broker.rs:244`) is an async lock around a tiny value that's
  never held across an await — a `std::sync::Mutex<Instant>` would be cheaper
  and simpler. Real code has warts; recognising them is part of the skill.
* `let _ = tx.send(..)` is fine when "nobody listening" is expected; for mpsc
  sends that *should* succeed, log the error like the databroker does
  (`debug!("Failed to send response: {}", err)`).

## In the databroker

* `databroker/src/broker.rs:179-186` — `DataBroker`: `Arc<RwLock<Database>>`,
  `Arc<RwLock<Subscriptions>>`, `broadcast::Sender<()>` shutdown trigger. The
  struct derives `Clone`, and every clone shares the same state.
* `databroker/src/broker.rs:239-245` — `ChangeSubscription` owns a
  `broadcast::Sender<Option<EntryUpdates>>`.
* `databroker/src/broker.rs:1529-1598` — `update_entries`: write lock →
  apply → `downgrade()` (`:1559`) → notify under read lock → cleanup.
* `databroker/src/broker.rs:1601-1680` — `subscribe`: channel sizing and the
  initial notification.
* `databroker/src/broker.rs:1836` — `snapshot_actuation_subscriptions` (e1fdfd1).
* `databroker/src/broker.rs:2200-2334` — `get_values_broker`: snapshot (`:2210`),
  validate, concurrent provider calls, fallback.
* `databroker/src/broker.rs:2495-2506` — `shutdown` and `get_shutdown_trigger`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:59-64` — `Provider`: an `mpsc::Sender`
  to the client, a `tokio::sync::Mutex` around the response receiver, an
  `AtomicU32` request counter.
* `databroker/src/grpc/kuksa_val_v2/val.rs:154-247` — the deadline-budget
  request/response exchange.

## Exercises

| Exercise | Goal |
|---|---|
| `c21_e01_tokio_mpsc_backpressure` | `try_send` outcomes, draining, `is_closed` |
| `c21_e02_oneshot_request_reply` | An actor that owns a map; replies via oneshot |
| `c21_e03_broadcast_and_lag` | Count lagged messages; understand ring-buffer semantics |
| `c21_e04_shutdown_trigger` | The `DataBroker` shutdown pattern, including subscribe-before-spawn |
| `c21_e05_rwlock_downgrade` | Write, downgrade, notify — readers allowed, writers blocked |
| `c21_e06_snapshot_then_await` | Reproduce and fix the e1fdfd1 freeze |
| `c21_e07_deadline_budget` | One deadline shared by send and receive |
| `c21_e08_watch_latest_value` | Latest-value state with coalescing; `wait_for`; sender-dropped handling |
| `c21_e09_review_shutdown_and_joinhandle` | Review: fix a dropped `JoinHandle` and a shutdown branch that skips draining queued work |

## Further reading

* Tokio tutorial — "Shared state" and "Channels": https://tokio.rs/tokio/tutorial
* `tokio::sync` module docs (which lock when, `broadcast` lag semantics): https://docs.rs/tokio/latest/tokio/sync/
* Tokio docs — `Mutex`: "Which kind of mutex should you use?"
* Alice Ryhl, "Actors with Tokio": https://ryhl.io/blog/actors-with-tokio/
* `git show e1fdfd1` in this repo.

## Check yourself

1. When is `std::sync::Mutex` the better choice in async code?
2. A broadcast channel has capacity 3; 10 messages are sent before a receiver reads. What does it observe?
3. Why must a task subscribe to the shutdown trigger before being spawned?
4. What does `downgrade()` guarantee that "drop write guard, take read guard" doesn't?
5. Explain how one hung provider could freeze the whole broker before e1fdfd1.
6. Why did e1fdfd1 change `Box<dyn ActuationProvider>` into `Arc<dyn ActuationProvider>`?
7. With a 1 s deadline, the send takes 0.7 s. How long may the receive take?

<details><summary>Answers</summary>

1. When the critical section is short and contains no `.await` — it's faster and simpler.
2. `Err(Lagged(7))`, then messages 8, 9, 10, then (once senders are gone) `Closed`.
3. A receiver only sees messages sent after it subscribed; a task that subscribes itself may do so after `shutdown()` was already sent and then never stop.
4. That no writer can modify the data between the write and the read phase — the notifier sees exactly the state it just wrote.
5. `actuate` held `subscriptions.read()` while awaiting the provider. The next `subscriptions.write()` (new subscription, cleanup) queued behind it, and because the RwLock is fair, every later reader queued behind that writer — so everything touching subscriptions stopped.
6. To snapshot providers cheaply: cloning an `Arc` shares the provider; a `Box` can't be shared without moving it out of the table.
7. At most the remaining 0.3 s.

</details>

## Where this goes next

Chapter 22 turns channel receivers into streams. **Chapter 24** shows how a channel wakes a waiting task, and chapter 25 covers the same trade-offs for threads, plus memory ordering.
