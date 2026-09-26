# 35 — Capstone: build a mini-databroker

> **Builds on:** chapters 03–05 (data modelling), 11 (lifetimed views), 20–23 (async, broadcast, streams).

**Why this matters for the databroker.** `databroker/src/broker.rs` is ~5,700
lines, and on first read it is a wall. It gets much easier once you've written
the same thing yourself at 1/10th the size. In this chapter you build a
*mini-databroker* in six steps. Each step's file ships with the finished code of
the step before, so you never carry a broken step forward. Names and shapes
match `broker.rs` on purpose: after chapter 35, reading the real one should
feel like reading a longer version of your own code.

What you'll have at the end:

```text
            ┌────────────────────── DataBroker (Clone = Arc clones) ─────────────────────┐
 gRPC task ─┤ authorized_access(&perms) ──► AuthorizedAccess<'b,'p>                      │
            │     update_entries ──► RwLock<Database>  (write → downgrade → read)        │
            │                           └► RwLock<Subscriptions>.notify ──► broadcast tx │
            │     subscribe ────────► broadcast rx ──► BroadcastStream ──► client stream │
            │ housekeeping task: every 1 s drop subscriptions with receiver_count()==0   │
            └────────────────────────────────────────────────────────────────────────────┘
```

## Step 1: the Database

The `Database` is three fields:

```rust
pub struct Database {
    next_id: AtomicI32,                 // id generator
    path_to_id: HashMap<String, i32>,   // "Vehicle.Speed" -> 7
    entries: HashMap<i32, Entry>,       // 7 -> Entry { metadata, datapoint, .. }
}
```

Why two maps instead of one `HashMap<String, Entry>`? Clients mostly talk in
ids after the first lookup (`SubscribeById`, provider streams), and an `i32`
hashes and compares much faster than a string. The path map is only needed to
resolve names.

Registration rules are the real ones: invalid path → error; an existing path
returns the **existing** id (registration is idempotent — the broker loads VSS
files that may overlap); `allowed` values must fit the data type.

`fetch_add` returns the *previous* value, so ids start at 0. Honest critique:
`add` already takes `&mut self`, which proves exclusive access, so a plain
`i32` would suffice. The atomic is harmless but not necessary.

A small idiom win: the real `DatabaseWriteAccess::add` takes ten positional
arguments and carries `#[allow(clippy::too_many_arguments)]`. We use a
`NewEntry` struct with builder-style methods instead — the usual fix for
argument lists that long.

## Step 2: updates and validation

`EntryUpdate` is a *partial* update: every field is an `Option`, and `None`
means "leave it alone". For the actuator target that isn't enough, because
"set the target to nothing" must be expressible too — so it is
`Option<Option<Datapoint>>`:

| value            | meaning                     |
|------------------|-----------------------------|
| `None`           | don't touch the target      |
| `Some(None)`     | clear the target            |
| `Some(Some(dp))` | set the target to `dp`      |

`Database::update` runs a small pipeline, exactly like
`DatabaseWriteAccess::update`:

1. **diff** — for `OnChange` signals drop a datapoint whose value equals the
   current one. (`Continuous` signals, like a tick counter, always pass.)
2. **validate** — type, min/max (inclusive), allowed list. Any error aborts and
   leaves the entry untouched: validate everything *before* mutating anything.
3. **apply** — move the new values in and return the `HashSet<Field>` that
   changed. That set drives notifications in step 4, which is why `Field`
   derives `Eq + Hash`.

## Step 3: authorized access

Every access in the real broker goes through a *view* that pairs the database
with the caller's permissions:

```rust
pub struct DatabaseReadAccess<'db, 'perm> {
    db: &'db Database,
    permissions: &'perm Permissions,
}
```

Two lifetimes because the two borrows are unrelated: the database lives in the
broker, the permissions came from this request's JWT. The compiler now
guarantees a view can't outlive either one — and since every getter checks
permissions, forgetting a check requires going out of your way.

Look closely at the return type you implement:

```rust
impl<'db> DatabaseReadAccess<'db, '_> {
    pub fn get_entry_by_id(&self, id: i32) -> Result<&'db Entry, ReadError>
```

The entry borrows the **database** (`'db`), not the view (`&self`). So the
entry reference stays usable after the view is dropped — the
`entries_outlive_the_view` test proves it. The real code returns `&Entry` tied
to `&self` (elided lifetime), which is slightly more restrictive; both are
sound.

The `From<PermissionError>` impls turn `?` into a translator: inside a function
returning `Result<_, ReadError>`, `self.permissions.can_read(path)?` converts
`PermissionError::Denied` into `ReadError::PermissionDenied` automatically. The
real broker writes those `match`es out by hand at every call site.

## Step 4: subscriptions and notify

Now the database becomes shared, concurrent state:

```rust
#[derive(Clone)]
pub struct DataBroker {
    database: Arc<RwLock<Database>>,        // tokio::sync::RwLock
    subscriptions: Arc<RwLock<Subscriptions>>,
    shutdown_trigger: broadcast::Sender<()>,
}
```

Every gRPC call runs in its own tokio task with its own clone of `DataBroker`;
cloning bumps three reference counts, nothing more.

A `ChangeSubscription` owns a `broadcast::Sender<Option<EntryUpdates>>` plus
the subscriber's permissions and "id → fields I care about". `notify` has two
modes: `None` builds the initial snapshot, `Some(changed)` sends only the
**intersection** of subscribed and changed fields, skips unreadable entries,
and treats an expired token as an error (so the subscription gets closed).

`update_entries` shows the most important lock choreography in the broker:

```rust
let mut db = self.broker.database.write().await;   // 1. exclusive
/* apply all updates */                             // 2.
let db = db.downgrade();                            // 3. write → read, atomically
subscriptions.read().await.notify(Some(&changed), &db); // 4.
```

`downgrade` turns the write guard into a read guard *without releasing the
lock in between*. Other readers can proceed immediately, but no writer can
slip in before the subscribers have seen a consistent snapshot of exactly
these changes. The lock order is always *database, then subscriptions*;
keeping one global order is what prevents deadlocks.

## Step 5: subscribe as a Stream

`subscribe` validates input, creates a `broadcast::channel(capacity)`, sends
the initial notification, registers the subscription and hands back the
receiver wrapped as a `Stream`:

```rust
BroadcastStream::new(receiver).map(|result| match result {
    Ok(message) => message,
    Err(BroadcastStreamRecvError::Lagged(_)) => None,
})
```

A broadcast channel keeps at most `capacity` messages. A receiver that falls
further behind gets `Lagged(n)` once and then continues with the oldest message
still available. Capacity is `buffer_size + 1` — "n old messages plus the
latest" — and with no buffer, 1: a slow client simply sees the newest state.
Lag becomes `None` on the stream, which the gRPC layer turns into an error
message for the client.

**Spot the race.** The real `subscribe` releases the database read lock after
the initial notification and *then* takes the subscriptions write lock to
register. An `update_entries` can run in between: it notifies the existing
subscribers, but ours isn't registered yet, so it misses that update while its
snapshot is already stale. The solution holds the database read guard until
registration is done. Writers need the write lock, so they wait, and the lock
order (database → subscriptions) matches `update_entries`, so this can't
deadlock.

`subscribe_paths` shows the opposite rule: **release a guard before calling
something that takes the same lock again.** tokio's `RwLock` is fair. If a
writer is queued between your two read acquisitions, the second read waits for
the writer, and the writer waits for your first read: deadlock.

We return `Pin<Box<dyn Stream<..> + Send>>` instead of `impl Stream`. It gives
the type a name (the gRPC layer needs one: `type SubscribeStream = Pin<Box<dyn
Stream<..>>>`), and a boxed stream is `Unpin`, so `.next().await` just works.

## Step 6: housekeeping and shutdown

There is no "unsubscribe" RPC. When a client goes away, tonic drops the
response stream → our `BroadcastStream` → the broadcast **receiver**. A task
wakes up every second and removes subscriptions whose
`sender.receiver_count()` is 0 or whose token expired. Dropping a value *is*
the goodbye message; that's RAII working across a network boundary.

```rust
let mut shutdown = self.shutdown_trigger.subscribe(); // before spawn!
tokio::spawn(async move {
    let mut interval = tokio::time::interval(HOUSEKEEPING_INTERVAL);
    loop {
        tokio::select! {
            _ = interval.tick() => { subscriptions.write().await.cleanup_change_subscriptions(); }
            _ = shutdown.recv() => break,
        }
    }
})
```

Two lessons are packed in here:

- A spawned task must be `'static`, so it takes `Arc` clones, never `&self`.
- A broadcast receiver only sees messages sent *after* it was created.
  Subscribe before spawning, or an early shutdown is lost. The
  `shutdown_right_after_start_still_stops_the_task` test catches exactly that.

The real housekeeping task has no shutdown branch. It loops until the runtime
is dropped at process exit. That's fine for a binary, but a library that
starts background work should give callers a way to stop it, and ours does.

`#[tokio::test(start_paused = true)]` makes the housekeeping tests
deterministic: with a paused clock, `sleep(1.1 s)` fast-forwards virtual time
as soon as every task is idle. The tests run in microseconds and never flake.

## Mental model

| Piece | Analogy | Guarantee |
|---|---|---|
| `Database` behind `RwLock` | a ledger in a reading room: many readers or one writer | no data races (compile-time `Sync` + runtime lock) |
| `DatabaseReadAccess<'db,'perm>` | a visitor badge valid only while you're in the building | the view can't outlive the database or the permissions |
| `downgrade()` | the writer puts the pen down but keeps the book open for readers | no writer between "apply" and "notify" |
| broadcast channel | a radio station with a short replay buffer | slow listeners lose old messages, never block the station |
| `receiver_count() == 0` | an empty room — nobody's listening, turn off the speaker | cleanup without an explicit "unsubscribe" |
| `Arc` clone into a task | handing a spare key to a helper | the data lives as long as any key-holder |

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "A slow subscriber slows down publishers." | Broadcast `send` never waits; the slow receiver lags and loses messages. |
| "Holding a read lock while calling a function that also reads is fine." | With a fair `RwLock` and a queued writer it deadlocks. Scope guards tightly. |
| "`Option<Option<T>>` is a code smell." | It's the honest type for tri-state partial updates. |
| "The task can borrow `&self.subscriptions`." | `tokio::spawn` needs `'static` (E0597/E0521). Move an `Arc` clone in. |
| "I'll get a message when a client disconnects." | You get a *drop*. Detect it (`receiver_count`) and clean up. |
| "`&self` methods can only return references tied to `&self`." | Return `&'db T` when the data lives in something the view merely borrows. |

## Smells and idioms

- **Ten positional arguments** (`DatabaseWriteAccess::add`) → a parameter
  struct/builder (`NewEntry`).
- **Hand-written error translation at every call site** → `From` impls + `?`.
- **Validate, then mutate.** Never half-apply an update.
- **Snapshot, then await.** Clone the `Arc`s or values you need out of a lock,
  drop the guard, and only then `.await` slow things. `get_values_broker` does
  this with providers (see chapter 21).
- **Background tasks need an off switch** (the real housekeeping task lacks one).
- **`get_` prefixes** (`get_entry_by_id`, `get_shutdown_trigger`) are common in
  the databroker but un-idiomatic in Rust APIs. We kept them where they mirror
  the real names and dropped them elsewhere (`id_by_path`,
  `shutdown_trigger`).

## In the databroker

Read these side by side with your chapter-22 code:

| Mini broker | Real code |
|---|---|
| `DataValue`, `DataType` | `databroker/src/types.rs:90`, `databroker/src/types.rs:17` |
| `Metadata`, `Datapoint`, `Entry` | `databroker/src/broker.rs:90`, `:106`, `:113` (the real `Datapoint` also has `source_ts`; `Entry` also has `lag_datapoint`) |
| `Database` | `databroker/src/broker.rs:128` |
| `is_valid_path` | `databroker/src/glob.rs:202` (a regex) |
| `Database::add` | `databroker/src/broker.rs:1257` (`fetch_add` at `:1326`) |
| `UpdateError`, `ReadError`, `RegistrationError` | `databroker/src/broker.rs:64`, `:76`, `:83` |
| `Field` | `databroker/src/broker.rs:121` |
| `EntryUpdate` | `databroker/src/broker.rs:251` (see the comments on `Option<Option<..>>`) |
| `Entry::diff` / `validate` / `apply` | `databroker/src/broker.rs:276`, `:302`, `:758`; value checks at `:517`, `:497`, `:364` |
| `Database::update` | `databroker/src/broker.rs:1202` |
| `Permissions` | `databroker/src/permissions.rs:39`, `can_read` at `:167` |
| `DatabaseReadAccess` / `DatabaseWriteAccess` | `databroker/src/broker.rs:1062`, `:1067`; constructors at `:1351` |
| `get_entry_by_id` through a view | `databroker/src/broker.rs:1137` |
| `ChangeNotification`, `EntryUpdates` | `databroker/src/broker.rs:153`, `:160` (the real one wraps an `EntryUpdate`) |
| `ChangeSubscription` | `databroker/src/broker.rs:239` (adds `interval_duration`/`last_emitted` for min-sample-interval) |
| `ChangeSubscription::notify` | `databroker/src/broker.rs:923` |
| `Subscriptions::notify` | `databroker/src/broker.rs:824` |
| `DataBroker` | `databroker/src/broker.rs:179` (+ `filter_manager`, version info) |
| `AuthorizedAccess` | `databroker/src/broker.rs:1373` |
| `update_entries` (downgrade) | `databroker/src/broker.rs:1529`, downgrade at `:1559` with the explanatory comment at `:1556` |
| `subscribe` | `databroker/src/broker.rs:1601`; initial notify `:1640`; registration `:1647`; lag mapping `:1667`; `MAX_SUBSCRIBE_BUFFER_SIZE` at `:38` |
| `subscribe_paths` | `databroker/src/grpc/kuksa_val_v2/val.rs:383` (paths → ids via `get_signal`, `:1308`) |
| `cleanup_change_subscriptions` | `databroker/src/broker.rs:892` |
| `start_housekeeping_task` | `databroker/src/broker.rs:2362` (started from `databroker/src/grpc/server.rs:205`) |
| `shutdown` | `databroker/src/broker.rs:2495`; trigger receiver `:2504`; used by `databroker/src/grpc/server.rs:95` |

Things the real broker has that the mini one doesn't: providers
(`SignalProvider` at `databroker/src/broker.rs:198`, `ActuationProvider` at
`:189`), the `FilterManager` for sample intervals
(`databroker/src/filter/filter_manager.rs:25`, fed by
`propagate_new_filter_to_provider` at `databroker/src/broker.rs:2011`), and
`lag_datapoint` bookkeeping. Chapter 36 uses them.

## Exercises

| Exercise | Goal |
|---|---|
| `c35_e01_database_register` | ids, `path_to_id`, idempotent registration, path validation |
| `c35_e02_update_and_validate` | diff → validate → apply, `HashSet<Field>` of changes, tri-state target |
| `c35_e03_authorized_access` | lifetime-scoped read/write views, `From` + `?` for errors |
| `c35_e04_subscriptions_notify` | `Arc<RwLock<..>>` state, intersection-based notify, write→downgrade→notify |
| `c35_e05_subscribe_stream` | broadcast → `Stream`, lag → `None`, buffer rules, a race you can fix |
| `c35_e06_housekeeping` | interval task with `select!`, RAII-driven cleanup, graceful shutdown |

## Further reading

- Tokio tutorial, "Shared state" and "Channels" — https://tokio.rs/tokio/tutorial
- `tokio::sync::RwLockWriteGuard::downgrade` and `tokio::sync::broadcast` docs — https://docs.rs/tokio
- `tokio_stream::wrappers::BroadcastStream` — https://docs.rs/tokio-stream
- Rust Book ch. 10.3 "Validating References with Lifetimes" and ch. 16 "Fearless Concurrency" — https://doc.rust-lang.org/book/
- `tokio::time::pause` / `#[tokio::test(start_paused = true)]` — https://docs.rs/tokio/latest/tokio/time/fn.pause.html

## Check yourself

1. Why does the database keep both `path_to_id` and `entries` instead of one map keyed by path?
2. What does `Some(None)` mean in `EntryUpdate::actuator_target`, and why can't a plain `Option` express it?
3. Why may `get_entry_by_id` return `&'db Entry` rather than a reference tied to `&self`?
4. What would go wrong if `update_entries` released the write lock and then took a fresh read lock to notify?
5. A client subscribes with no buffer and reads slowly while three updates happen. What does its stream yield?
6. How does the broker find out that a subscriber disconnected?
7. Why must the housekeeping task subscribe to the shutdown trigger before `tokio::spawn`?
8. Why can calling `subscribe` while still holding a database read guard deadlock?

<details><summary>Answers</summary>

1. Ids are the hot-path key: cheap to hash and compare, and used by
   `SubscribeById` and provider streams. Paths are only resolved once.
2. "Clear the target". `None` already means "don't touch it", so you need a
   second layer to say "set it to nothing".
3. The view holds `&'db Database`; anything reached through it lives as long as
   the database borrow. The view itself is only a pair of references.
4. A writer could change the database in the gap, so subscribers would be
   notified about ids whose data had already changed again, or see a mix of two
   updates.
5. `None` (it lagged), then the latest update. The ones in between are lost.
6. It doesn't get told. Dropping the stream drops the broadcast receiver, and
   housekeeping sees `receiver_count() == 0`.
7. A broadcast receiver only gets messages sent after it was created. If
   `shutdown()` runs before the task first polls, a receiver created inside
   the task never sees it.
8. tokio's `RwLock` is fair. A writer queued between your two read
   acquisitions blocks the second one, while the writer waits for your first
   guard.
</details>
