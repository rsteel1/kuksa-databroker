# 20 — Async basics with Tokio

**Why this matters for the databroker.** The databroker is one process that
talks to many clients at once: dozens of gRPC subscribers waiting for signal
updates, providers streaming sensor values, a housekeeping loop ticking every
second, a VISS websocket server. Almost all of that time is spent *waiting* —
for bytes on a socket, for a lock, for a timer. Async Rust lets one thread
juggle thousands of such waits cheaply. Nearly every interesting function in
`databroker/src/broker.rs` is `async fn`, and the whole server runs inside a
Tokio runtime built in `databroker/src/main.rs`. You can't read the codebase
without being fluent here.

## Why async at all?

The classic way to serve many clients is one OS thread per client. Threads are
simple, but each has its own stack (often 2–8 MB reserved) and switching
between them costs a trip through the kernel. 10 000 idle subscribers would
mean 10 000 threads doing nothing.

Async flips this: work is split into small **tasks** that voluntarily give up
the thread whenever they would wait. A handful of OS threads (the databroker
uses one per CPU core by default) run all tasks, picking up whichever one can
make progress.

| Use | When |
|---|---|
| Threads (`std::thread`, chapter 13) | CPU-bound work, few long jobs, blocking libraries |
| Async (`tokio`) | I/O-bound work, many concurrent waits — **a network broker** |
| Both (`spawn_blocking`) | Async server that occasionally does heavy CPU work |

## Futures are lazy state machines

An `async fn` does not run when you call it. It returns a **future**: a value
that *describes* the work and knows how to make progress on it.

```rust
async fn fetch_speed(sensor: u32) -> u32 { sensor * 10 }

let f = fetch_speed(1);   // nothing happened yet; f: impl Future<Output = u32>
let speed = f.await;      // now it runs, and we get the u32
```

The compiler turns the body of every `async fn` into a **state machine** — an
enum with one variant per `.await` point, holding the local variables that are
alive at that point:

```rust
async fn example() -> u32 {
    let a = step_one().await;   // state 1: waiting on step_one, nothing else alive
    let b = step_two(a).await;  // state 2: waiting on step_two, `a` must be stored
    a + b
}
// roughly becomes:
enum ExampleFuture {
    Start,
    WaitingStepOne(StepOneFuture),
    WaitingStepTwo { a: u32, fut: StepTwoFuture },
    Done,
}
```

Consequences you will meet over and over:

* **Futures are lazy.** Create one and drop it without awaiting, and nothing
  happens (exercise `c20_e01`). Clippy/rustc warn with "unused implementer of
  `Future` that must be used".
* **Everything alive across an `.await` lives inside the future.** That is why
  a lock guard held across `.await` becomes part of the future and affects
  whether it is `Send` (see below).
* **A future is just a value.** You can store it, pass it to `join_all`, wrap it
  in `timeout`, or drop it to cancel it.

### Polling, wakers and the executor

A future is driven by calling its `poll` method, which returns either
`Poll::Ready(value)` or `Poll::Pending`. When it returns `Pending`, it has
arranged for a **waker** to be called when it's worth polling again (a socket
became readable, a timer fired, a lock was released). The **executor** (Tokio)
keeps a queue of tasks whose wakers fired and polls them.

```
 task A: poll -> Pending (waiting for socket)      ... socket readable -> wake(A)
 task B: poll -> Pending (waiting for timer)       ... timer fires     -> wake(B)
 executor: pick A from ready queue -> poll -> Ready(...)
```

So **`.await` is a yield point**: the only places a task can pause and let
another task use the thread are its `.await`s. Between two `.await`s your code
runs uninterrupted on one thread — which is also why a long computation with
no `.await` in it blocks everything else on that worker thread.

You will almost never call `poll` yourself. `.await` does it for you.

### The Tokio runtime

Futures need an executor. The databroker builds one explicitly
(`databroker/src/main.rs:417`):

```rust
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(*worker_threads)
    .enable_all()
    .build()
    .unwrap();
runtime.block_on(async { /* the whole server */ });
```

Most programs use the shortcut `#[tokio::main]`, and tests use
`#[tokio::test]` (a fresh single-threaded runtime per test unless you ask for
`flavor = "multi_thread"`).

## Tasks: tokio::spawn and JoinHandle

`tokio::spawn(future)` hands a future to the runtime as an independent
**task**. It starts running concurrently right away and you get back a
`JoinHandle<T>` — itself a future resolving to `Result<T, JoinError>`
(`Err` if the task panicked or was aborted).

```rust
let handle = tokio::spawn(async move { expensive_query().await });
// ... do other things ...
let answer = handle.await.expect("task panicked");
```

`spawn` has this signature:

```rust
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where F: Future + Send + 'static, F::Output: Send + 'static
```

* **`'static`**: the task may outlive the function that spawned it, so it must
  not borrow anything from that function's stack. It must **own** its data.
* **`Send`**: the runtime may move the task to another worker thread between
  polls.

The standard recipe — used throughout the databroker — is *clone a handle,
then `async move`*:

```rust
// databroker/src/grpc/kuksa_val_v2/val.rs, open_provider_stream
let broker = self.clone();                 // DataBroker is a bundle of Arcs: cheap
tokio::spawn(async move {                  // `move`: the task owns `broker`
    let broker = broker.authorized_access(&permissions);
    loop { /* ... */ }
});
```

Dropping a `JoinHandle` does **not** cancel the task; it just detaches it. Use
`handle.abort()` to cancel.

## Time: sleep, timeout and paused clocks

* `tokio::time::sleep(d).await` — pause *this task* (the thread is free for
  others). **Never** use `std::thread::sleep` in async code: it freezes the
  whole worker thread and every task scheduled on it.
* `tokio::time::timeout(d, fut).await` → `Result<T, Elapsed>`. If time runs
  out, `fut` is **dropped** — cancelled at whatever `.await` it was paused on.
* `tokio::time::interval(d)` — a ticker; the housekeeping task uses it
  (`databroker/src/broker.rs:2368`). The first `tick()` completes immediately.
* `tokio::time::Instant` — like `std::time::Instant` but follows Tokio's clock.

**Testing time deterministically.** With `#[tokio::test(start_paused = true)]`
Tokio's clock is frozen; whenever every task is waiting on a timer, the clock
*jumps* to the next deadline. A test that "waits an hour" runs in
microseconds and `Instant::elapsed()` returns exact values. All timing tests
in this course use this, and so should yours. (Requires Tokio's `test-util`
feature.)

## Concurrency: join and join_all

Sequential awaits add their latencies:

```rust
let a = query(p1).await;   // 300 ms
let b = query(p2).await;   // + 300 ms = 600 ms
```

To overlap the waiting, create the futures first and await them together:

```rust
let (a, b) = tokio::join!(query(p1), query(p2));                // 300 ms
let all = futures::future::join_all(providers.iter().map(query)).await; // many
```

`join!`/`join_all` run the futures **concurrently on the same task** — no new
threads, no `'static` requirement (they may borrow). `join_all` returns results
in input order. This is concurrency, not parallelism: all polling happens on
one thread, but the *waits* overlap.

Before commit **748a914** ("Query providers concurrently and added test"), the
databroker asked providers one at a time in `get_values_broker`, so each extra
slow provider added its full latency. Now it builds one future per provider
and awaits them with `join_all` (`databroker/src/broker.rs:2254-2262`).

## select! and cancellation safety

`tokio::select!` waits on several futures and runs the branch of whichever
completes **first**; all other branch futures are **dropped**.

```rust
// databroker/src/main.rs:40 — shutdown_handler
select! {
    _ = sigint.recv() => info!("received SIGINT"),
    _ = sighup.recv() => info!("received SIGHUP"),
    _ = sigterm.recv() => info!("received SIGTERM"),
};
```

In a loop it becomes the event loop of a connection:

```rust
loop {
    select! {
        message = stream.message() => { /* handle, or break on None/Err */ }
        _ = shutdown_trigger.recv() => break,
    }
}
```

**Cancellation safety.** Since the losing branch's future is dropped, ask: *if
this future is dropped half-way, is anything lost?* `mpsc::Receiver::recv`,
`broadcast::Receiver::recv` and `tonic::Streaming::message` are cancellation
safe — a message is only removed when the future completes. Something like
"read a length header, then `.await` the body" is not: dropping it between the
two awaits loses the header. Tokio's docs state cancellation safety for each
method; check before putting a future into `select!` in a loop.

If several branches are ready at once, `select!` picks one at random (to avoid
starvation). Add `biased;` as the first line to poll in written order.

## Send futures and blocking work

A spawned future must be `Send`, and a future is `Send` only if everything it
holds across an `.await` is `Send`. The classic failure:

```rust
let mut guard = std_mutex.lock().unwrap();   // std::sync::MutexGuard: !Send
*guard += 1;
sleep(ms(1)).await;                           // guard still alive here!
```
→ `error: future cannot be sent between threads safely`.

Fixes, in order of preference:

1. **Scope the guard** so it's dropped before the await:
   `let v = { let mut g = m.lock().unwrap(); *g += 1; *g }; sleep(..).await;`
2. If you genuinely need to hold the lock across an await, use
   `tokio::sync::Mutex`/`RwLock`, whose guards are `Send` and whose `lock()` is
   itself async. The databroker's `database` and `subscriptions` are
   `tokio::sync::RwLock` for exactly this reason (`databroker/src/broker.rs:21`).
   It still costs something: see chapter 21 on holding locks across awaits.

**Blocking work.** Code between two awaits monopolises its worker thread.
For CPU-heavy work (hashing, big parsing) or blocking libraries (std file I/O,
a sync database driver), use `tokio::task::spawn_blocking(move || ...)`, which
runs the closure on a separate pool of threads meant for blocking.

## Mental model

| Concept | Analogy |
|---|---|
| Future | A recipe card: describes the dish; nothing is cooked until someone follows it |
| `.await` | "I'll wait here — chef, go work on another order meanwhile" |
| Executor / runtime | The chef(s) moving between orders that are ready for the next step |
| Waker | The kitchen timer that rings to say "this order can continue" |
| `tokio::spawn` | Handing an order to the kitchen; you get a ticket (`JoinHandle`) |
| `join_all` | Putting all pots on the stove at once |
| `select!` | "Whichever is done first — and throw the other pot away" |
| `spawn_blocking` | Sending the dough to the bakery next door so the stove stays free |

**Guarantee the compiler gives:** a future that crosses threads is `Send`, and
spawned tasks can't dangle references into dead stack frames (`'static`).
What it does **not** guarantee: that you never block a worker thread or hold a
lock too long — those are on you.

## Common misconceptions

| Wrong model | Correct model | You'll see |
|---|---|---|
| "Calling an async fn starts it" | It only builds a future; `.await`/spawn runs it | E0369/E0308 when you use the future as a value; "unused future" warning |
| "async = multithreaded" | `join_all` is concurrent on one task; only spawn can go parallel | surprising sequential timings |
| "`.await` blocks the thread" | It yields the thread to other tasks | — |
| "A spawned task can borrow my locals" | It must own everything (`'static`) | E0597, E0373 |
| "`std::thread::sleep` is fine in async" | It blocks every task on that worker | mysterious latency spikes |
| "Dropping a JoinHandle cancels the task" | It detaches; use `abort()` | leaked background tasks |
| "`timeout` stops the work cleanly" | It drops the future at its current `.await` | half-done operations if not cancellation-safe |

## Smells and idioms

* `std::thread::sleep` / blocking I/O inside `async fn` → `tokio::time::sleep`,
  `tokio::fs`, or `spawn_blocking`.
* Sequential `.await` in a loop over independent I/O → `join_all` (bounded if
  there can be thousands, e.g. `futures::stream::iter(..).buffer_unordered(n)`).
* Spawning a task and immediately awaiting its handle → just `.await` the
  future (spawn only buys you something if you do work in between or need
  `'static` isolation). The databroker's `actuate_stream`
  (`databroker/src/grpc/kuksa_val_v2/val.rs:565`) spawns and then awaits the
  handle — this keeps a panic inside the loop from tearing down the RPC
  handler (it becomes a `JoinError`), a reasonable trade-off.
* `.unwrap()` on `JoinHandle` results in library code → `expect("why")` or map
  the `JoinError` to an error your caller understands.
* `select!` over futures that aren't cancellation safe, inside a loop.
* `runtime.block_on` inside async code → panics ("Cannot start a runtime from
  within a runtime"). Only at the very top (like `main.rs`).

## In the databroker

* `databroker/src/main.rs:417-423` — the runtime is built by hand so the number
  of worker threads is configurable (`--worker-threads`).
* `databroker/src/main.rs:40` — `shutdown_handler`: a `select!` over three Unix
  signals. Its future is passed down to the gRPC server as the "stop" signal.
* `databroker/src/main.rs:568` — the optional VISS server runs in its own
  spawned task; note the `let broker = broker.clone();` right before `async move`.
* `databroker/src/broker.rs:2362-2442` — `start_housekeeping_task`: `tokio::spawn`
  + `interval.tick().await` loop. Clones of the `Arc`s are moved in first
  (`let subscriptions = self.subscriptions.clone();`).
* `databroker/src/broker.rs:2254-2262` — building one future per provider and
  `futures::future::join_all(provider_calls).await` (commit 748a914).
* `databroker/src/broker.rs:2477` — `tokio::time::timeout(PROVIDER_UPDATE_FILTER_TIMEOUT, ...)`
  bounds a provider call; the constant is at `databroker/src/broker.rs:39`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:933-1070` — the provider connection
  task: `tokio::spawn(async move { loop { select! { ... } } })`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:565-642` — `actuate_stream`: spawn,
  select loop, then `join_handle.await` mapped to a gRPC status.

## Exercises

| Exercise | Goal |
|---|---|
| `c20_e01_async_await` | See that futures are values and lazy; await them |
| `c20_e02_tokio_spawn_joinhandle` | Satisfy `'static` with `async move`; collect `JoinHandle`s |
| `c20_e03_sleep_and_timeout` | `sleep`, `timeout`, and deterministic paused-clock tests |
| `c20_e04_join_all_concurrency` | Turn sequential provider queries into concurrent ones |
| `c20_e05_select` | Write the provider connection loop with shutdown |
| `c20_e06_blocking_and_send` | Fix a `!Send` future; move CPU work to `spawn_blocking` |

## Further reading

* Async Book — "Why Async?", "Under the Hood: Executing Futures and Tasks":
  https://rust-lang.github.io/async-book/
* Tokio tutorial — "Hello Tokio", "Spawning", "Select":
  https://tokio.rs/tokio/tutorial
* Tokio docs — `tokio::time` (paused clock: `tokio::time::pause`),
  `tokio::select!` (cancellation safety section): https://docs.rs/tokio
* Alice Ryhl, "Async: What is blocking?": https://ryhl.io/blog/async-what-is-blocking/

## Check yourself

1. What does `let f = fetch_speed(1);` do, and when does the body run?
2. Why must a future passed to `tokio::spawn` be `'static`? Why `Send`?
3. What's the difference between `join_all(futs).await` and spawning each future?
4. In `select!`, what happens to the branch that didn't win?
5. Why is holding a `std::sync::MutexGuard` across `.await` a compile error in a spawned task?
6. How long does a `#[tokio::test(start_paused = true)]` test that sleeps 10 s take in real time?

<details><summary>Answers</summary>

1. It creates a future (a state machine value). The body runs only when the future is polled — by `.await`, `spawn`, `join_all`, etc.
2. `'static`: the task can outlive the spawning function, so it may not borrow its locals. `Send`: the multi-threaded runtime may resume the task on another thread.
3. `join_all` polls all futures inside the current task (concurrent, may borrow, one thread). Spawning creates independent tasks that may run in parallel on several threads, must be `'static + Send`, and keep running if you stop waiting.
4. Its future is dropped — cancelled at whatever `.await` it was suspended on.
5. Values alive across an `.await` are stored inside the future; `MutexGuard` is `!Send`, so the whole future is `!Send`, and `spawn` requires `Send`.
6. Essentially no time — the paused clock auto-advances when all tasks are waiting on timers.

</details>

## Where this goes next

Chapters 21–23 build the databroker's async machinery on top of this. **Chapter 24 (Async internals)** then opens the box: implementing `Future` by hand, wakers, a mini executor, `Pin`, and tower `Service`.
