# 24 — Async internals

**Why this matters for the databroker.** Chapters 20–23 taught you to *use*
async Rust: `.await`, `tokio::spawn`, channels, streams, `#[async_trait]`.
That's enough to write most application code. It isn't enough to read the
libraries underneath: tonic's `InterceptedService`, tokio's `Sleep`, tower
middleware, `tokio-stream` adapters, or the generated gRPC server code. Those
libraries implement `Future`, `Stream` and `tower::Service` by hand, deal with
`Pin`, and store `Waker`s. In this chapter you build each piece yourself: a
future, a waker-driven leaf future, an executor using only std, pin
projection, a hand-written stream, cancellation-safe state, and tower
middleware. After it, nothing in an async Rust codebase should look like
magic.

---

## The Future trait, by hand

Here is the whole trait (from `core::future`):

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub enum Poll<T> { Ready(T), Pending }
```

Chapter 20 said the compiler turns every `async fn` into a state machine
that implements this trait. Writing one yourself shows the contract that
those state machines, and every library future, must follow:

1. **`Ready(value)` exactly once.** After that the future is finished.
   Polling it again is a bug; many futures panic if you do.
2. **`Pending` comes with a promise.** Before returning `Pending`, the future
   must make sure that `cx.waker()` will be woken when it's worth polling
   again. If nobody wakes it, the task hangs silently for ever. This is the
   classic "lost wake-up" bug.
3. **`poll` must be cheap and non-blocking.** It does whatever work is
   possible *right now* and returns.

```rust
impl Future for Countdown {
    type Output = &'static str;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        let this = self.get_mut();          // OK because Countdown: Unpin
        if this.remaining == 0 { return Poll::Ready("liftoff"); }
        this.remaining -= 1;
        cx.waker().wake_by_ref();           // "poll me again soon"
        Poll::Pending
    }
}
```

A future that wakes itself immediately, like this one, is a *yield*: it gives
other tasks a turn. That's exactly what `tokio::task::yield_now()` does.

To poll by hand outside a runtime you need a `Context`. The simplest one
comes from a waker that does nothing: `Context::from_waker(Waker::noop())`.
That only works for futures that never wait on anything external.

## Wakers: how Pending ends

A **leaf future** waits on something outside the program's control: a
socket, a timer, a message from another task. Its `poll` checks whether the
thing is ready. If not, it **stores the waker where the producer can find
it** and returns `Pending`. Later the producer (the OS event loop, the timer
wheel, a channel sender, another thread) calls `waker.wake()`, and the
executor puts the task back in its run queue.

```
 task polls Signal ──► value missing ──► store cx.waker() ──► Pending
                                                     │
 other thread: fire(7) ─► set value ─► take waker ───┘─► waker.wake()
                                                           │
 executor: task is runnable again ─► poll ─► value present ─► Ready(7)
```

Three rules that library code always follows:

- **Check and store under the same lock.** If `poll` checks "no value",
  unlocks, and only then stores the waker, the producer can fire in between,
  find no waker, and the wake-up is lost.
- **Always keep the latest waker.** A task can move between worker threads,
  and each poll may come with a different waker. `Waker::will_wake` lets you
  skip the clone when it's the same one.
- **Don't wake while holding your own lock.** Waking can run arbitrary code
  (some executors poll right away). Take the waker out, release the lock,
  then call `wake()`.

`std::task::Wake` is the easy way to make a waker from your own type: implement
`fn wake(self: Arc<Self>)`, then `Waker::from(Arc::new(my_waker))`.

Tokio's `Sleep` (the future behind `tokio::time::sleep` and the interval in
the databroker's housekeeping task) is this pattern at industrial scale. It
registers with a timer wheel, and the timer driver wakes it when the deadline
passes.

## Build an executor

An executor needs only three ingredients:

1. **A task** holds a boxed, pinned future (`Pin<Box<dyn Future<Output = ()> + Send>>`).
2. **A queue** holds runnable tasks.
3. **A waker per task** pushes that task back onto the queue when woken.

```rust
impl Wake for Task {
    fn wake(self: Arc<Self>) {
        let queue = self.queue.clone();
        queue.lock().unwrap().push_back(self);   // runnable again
    }
}

fn run(&self) {
    while let Some(task) = self.pop() {          // pop in its own statement!
        let waker = Waker::from(task.clone());
        let mut cx = Context::from_waker(&waker);
        // poll once; if still Pending, keep the future in the task
    }
}
```

`block_on` is the one-future version, and it's what `#[tokio::main]` and
`runtime.block_on` do at the top of `main`. Poll the future. If it's
`Pending`, `thread::park()` until the waker calls `thread.unpark()`, then
poll again. `park`/`unpark` is safe against races, because an `unpark` that
arrives before the `park` is remembered.

Exercise `c24_e03` shows the defining property of async concurrency: two tasks
that `.await` in a loop take turns (`A1 B1 A2 B2 …`) on **one** thread. Each
`.await` that returns `Pending` is where the executor switches tasks.

What Tokio adds on top: multiple worker threads, per-thread queues with work
stealing, an I/O driver (epoll on Linux) and a timer driver that turn
external events into `wake()` calls, cooperative budgeting so one busy task
can't starve others, and `spawn_blocking` threads.

## Pin and Unpin

Consider an async block like this:

```rust
async {
    let readings = String::from("abc");
    let first = &readings;          // points into the future's own storage
    something().await;              // ...and stays alive across the await
    first.len()
}
```

The compiler's state machine stores both `readings` and `first` *inside the
future*, so `first` points into the future itself. If the future were moved
to another address after the first poll, `first` would dangle. Rust solves
this with a type-level promise:

- **`Pin<&mut T>`** is a mutable reference that promises the `T` behind it
  **will never move again**. `Future::poll` takes `self: Pin<&mut Self>`, so
  every future has been pinned before it is first polled.
- **`Unpin`** is an auto trait meaning "moving me is harmless, I have no
  self-references". Almost every ordinary type is `Unpin` (integers,
  `String`, `Vec`, `Box<T>`). Compiler-generated async state machines usually
  are **not**.
- For `Unpin` types, pinning costs nothing: `Pin::new(&mut x)` works, and
  `pin.get_mut()` gives back `&mut T`.
- For `!Unpin` types you pin in one of two ways:
  - **on the heap:** `Box::pin(fut)` gives `Pin<Box<F>>`. The box can move
    around, but the future inside can't. This is why tonic services return
    `Pin<Box<dyn Stream + Send>>`: see `type SubscribeStream` at
    `databroker/src/grpc/kuksa_val_v2/val.rs:369`.
  - **on the stack:** `std::pin::pin!(fut)` gives `Pin<&mut F>` for the rest
    of the scope. `tokio::pin!` is the older version of the same thing.

**Pin projection** is the problem every wrapper future hits. A `Timeout<F>`
or `Map<F>` holds an inner future and has to poll it, but it only has
`Pin<&mut Self>`. It needs `Pin<&mut F>` for the inner field and ordinary
`&mut` for plain fields such as counters. `pin_project_lite::pin_project!`
generates that projection safely:

```rust
pin_project! {
    struct WithPollCount<F> {
        #[pin] inner: F,   // projected as Pin<&mut F>
        polls: u32,        // projected as &mut u32
    }
}
// in poll:
let this = self.project();
*this.polls += 1;
this.inner.poll(cx)
```

This is exactly how tokio's `Sleep` is declared (`pin_project!` at
`tokio-1.53.1/src/time/sleep.rs:132`). When you see `#[pin]` in library
code, now you know why it's there.

Common errors:
- **E0277** "`F` cannot be unpinned" means you tried `Pin::new` or
  `get_mut` on a type that isn't `Unpin`. Pin it, or use projection.
- **E0599** "no method named `poll` found" means you're calling `poll` on a
  plain `&mut F`, not a `Pin<&mut F>`.

## Streams by hand

`Stream` is the async equivalent of `Iterator`:

```rust
fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
```

| Returns | Meaning |
|---|---|
| `Ready(Some(item))` | the next item |
| `Ready(None)` | finished; don't poll again |
| `Pending` | nothing yet; a wake-up is arranged |

Every adapter in `tokio_stream::StreamExt` (`map`, `filter_map`, `timeout`,
`take`) is a small pin-projected struct whose `poll_next` polls the inner
stream and transforms the result. That's exactly what you write in
`c24_e05` for the databroker's "`Lagged` → `None`" mapping
(`databroker/src/broker.rs:1665`). The most common bug in a hand-written
adapter is turning an inner `Pending` into `Ready(None)`, which ends the
stream early. Pass `Pending` up unchanged: the inner stream has already
registered the waker.

The trait itself is in `futures-core` (`futures-core-0.3.34/src/stream.rs:76`),
which is why `tokio-stream`, `futures` and `async-stream` all interoperate.

## Cancellation, Drop and task failure

In async Rust, **cancelling a future means dropping it**. That happens when:

- `select!` finishes one branch and drops the others;
- `tokio::time::timeout` expires;
- a `JoinHandle` is `abort()`ed, or the runtime shuts down;
- the gRPC client disconnects and tonic drops your response future or stream.

The dropped future stops at the `.await` where it was parked. No more of its
code runs, except the `Drop` implementations of values it owns. Two
consequences:

1. **Cleanup belongs in `Drop`.** An in-flight gauge, a "release the permit"
   step or a metric decrement written *after* an `.await` silently doesn't
   happen on cancellation. Write it as an RAII guard (`c24_e06` part 1).
2. **State that must survive a cancelled attempt lives outside the future.**
   A reader that pulls a header and then awaits the body loses the header if
   it's dropped in between, *unless* the header is stored in `self` right
   away. That's what "cancellation safe" means in tokio's documentation, and
   why the databroker's provider loop can safely `select!` on
   `stream.message()` (`databroker/src/grpc/kuksa_val_v2/val.rs:938`).

**Task failure.** A panic inside a spawned task does *not* crash the process
(unless the runtime is configured to). It unwinds that task only, and
awaiting its `JoinHandle` gives `Err(JoinError)`:

- `err.is_cancelled()`: the task was aborted;
- `err.is_panic()` / `err.into_panic()`: the panic payload, a
  `Box<dyn Any + Send>` that you `downcast_ref::<&str>()` or `::<String>()`.

(`JoinError` is at `tokio-1.53.1/src/runtime/task/error.rs:9`.) This is also
why the `todo!()` panic reachable in the databroker's `Actuate` path (see
guide 99) takes down one request, not the server.

## tower: Service and Layer

`tower::Service` is the abstraction under tonic, axum, hyper and most Rust
network middleware (`tower-service-0.3.3/src/lib.rs:311`):

```rust
pub trait Service<Request> {
    type Response;
    type Error;
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;
    fn call(&mut self, req: Request) -> Self::Future;
}
```

Think of it as an async function `Request -> Result<Response, Error>`, plus
**back-pressure**: callers must wait for `poll_ready` to return `Ready(Ok(()))`
before calling `call`. A rate limiter or a connection pool says "not yet"
here. `ServiceExt::oneshot(req)` does both steps for you.

A **`Layer<S>`** turns a service into a bigger one
(`tower-layer-0.3.3/src/lib.rs:95`). `ServiceBuilder::new().layer(a).layer(b).service(s)`
builds `a(b(s))`, so the **first layer listed is the outermost** and sees the
request first. That's why layer order matters for auth vs logging vs metrics
(`c24_e07`).

**How this maps to the databroker.** `ValServer::with_interceptor(broker, authorization)`
(`databroker/src/grpc/server.rs:256`) wraps the generated gRPC service in
tonic's `InterceptedService` (`tonic-0.14.6/src/service/interceptor.rs:87`),
which implements `Service<http::Request<_>>` (line 115). Its `call`
(line 129) runs your `Interceptor::call` (the trait is at line 41) before
forwarding to the inner service. The databroker's
`impl tonic::service::Interceptor for Authorization`
(`databroker/src/grpc/server.rs:51`) is that hook: it validates the bearer
token and inserts `Permissions` into the request extensions, just like the
`Auth` layer you write in `c24_e07`.

## Checkpoint

`c24_e08_checkpoint_part3` ends Part III (async). It gives you signatures and
tests only, and uses:

- tasks and `JoinHandle`s (chapter 20);
- channels, and turning a broadcast event into shared state (chapter 21);
- deadlines shared across several operations (`timeout_at`, chapter 21);
- concurrency limits and result ordering (chapters 21 and 23; compare
  `get_values_broker`);
- cancellation semantics (this chapter).

Design it on paper first. What does each worker do in a loop? Where does
each result go? What exactly happens at the deadline?

---

## Mental model

| Concept | Analogy |
|---|---|
| `Future` | A recipe card: nothing happens until a cook works through it |
| `poll` | The cook doing as many steps as possible without waiting |
| `Pending` + waker | "Ring this bell when the oven timer goes off" |
| Executor | The head chef: works only on cards whose bell has rung |
| `Pin` | A dish that's been plated: you may add to it, never move it |
| `Unpin` | Food in a box: moving it doesn't matter |
| Cancellation | The card is thrown away mid-recipe; only the cleanup crew (`Drop`) runs |
| `tower::Layer` | Airport security checkpoints wrapped around the gate |

**Guarantees:** the compiler ensures a `!Unpin` future is never moved after
it's pinned, and `Send` bounds ensure a future crossing threads holds only
thread-safe data. The *wake-up* contract is **not** checked by the compiler:
a missing wake-up compiles fine and hangs at runtime.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`Pending` means the runtime will poll me again later." | Only if someone calls your waker. No wake, no poll: the task hangs. |
| "Being polled means my data is ready." | Wake-ups can be spurious. Re-check your state on every poll. |
| "`Pin` makes data immutable." | `Pin<&mut T>` still allows mutation. It only forbids *moving*. |
| "`Box::pin` is slow magic." | It's one heap allocation, the same as `Box::new`, plus a promise. |
| "`Pin::new` works for any future." | Only for `Unpin` ones. Otherwise E0277 "cannot be unpinned". |
| "Code after `.await` always runs." | Not if the future is dropped (cancelled) at that `.await`. Use `Drop` guards. |
| "A panic in a task crashes the server." | It ends that task; `JoinHandle` reports `JoinError::is_panic()`. |
| "Middleware is special framework machinery." | It's a `Service` that holds another `Service`. |

## Smells and idioms

- **Storing the waker once and never updating it.** Refresh it on every
  `Pending` (skip the clone with `will_wake`).
- **Calling `wake()` while holding the lock the future also takes.** Take
  the waker out, release the lock, then wake.
- **`unsafe` pin projection.** Use `pin-project-lite` (or `pin-project`)
  instead. Unsafe `Pin::map_unchecked_mut` is almost never needed in
  application code.
- **Busy-polling** with a no-op waker in production. It's fine in tests and
  exercises; a real executor parks.
- **Cleanup after `.await` instead of in `Drop`.** It's wrong under
  `select!`, `timeout` and client disconnects.
- **Converting inner `Pending` to `Ready(None)` in a stream adapter** ends
  the stream early.
- **`BoxFuture` everywhere.** It's needed when branches return different
  future types, as in `Auth::call`, but it costs an allocation per call. A
  concrete future type (or `std::future::Ready`) is better when possible.
- *Databroker note:* the service methods return `Pin<Box<dyn Stream …>>`
  (`val.rs:369`) because a trait's associated type must name one type. With
  Rust 1.75+ return-position `impl Trait` in traits, some of this boxing
  could go away, but tonic's generated traits still use associated types.

## In the databroker and in the wild

Databroker:
- `databroker/src/main.rs:417` and `:423`: `Builder::new_multi_thread()` and
  `runtime.block_on(...)`, the production versions of `c24_e03`.
- `databroker/src/broker.rs:2368`: `tokio::time::interval` in housekeeping, a
  timer-driven leaf future.
- `databroker/src/broker.rs:1665`: `BroadcastStream::new(receiver).map(...)`,
  the combinator form of `c24_e05`'s `LagToNone`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:369`:
  `type SubscribeStream = Pin<Box<dyn Stream<…> + Send + Sync + 'static>>`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:938`: the `select!` loop that
  depends on `Streaming::message()` being cancellation safe.
- `databroker/src/grpc/server.rs:51` and `:256`: the `Interceptor` impl and
  `ValServer::with_interceptor`, which is tower middleware.

In the wild (crate sources are under `~/.cargo/registry/src/index.crates.io-*/`):
- `tokio-1.53.1/src/time/sleep.rs:132` (`pin_project!`), `:225`
  (`struct Sleep`) and `:452` (`impl Future for Sleep`): a real leaf future.
- `tokio-1.53.1/src/runtime/park.rs:24` and `:59`: the thread parker (a
  `Mutex` + `Condvar`) that `block_on` sleeps on. Compare your
  `thread::park()`.
- `tokio-1.53.1/src/runtime/task/error.rs:63`: `JoinError::is_panic`.
- `futures-core-0.3.34/src/stream.rs:76`: `Stream::poll_next`.
- `tower-service-0.3.3/src/lib.rs:311`, `tower-layer-0.3.3/src/lib.rs:95` and
  `tower-0.5.3/src/builder/mod.rs:132` (`ServiceBuilder::layer`).
- `tonic-0.14.6/src/service/interceptor.rs:41`, `:87`, `:115` and `:129`:
  `Interceptor`, `InterceptedService`, and its `Service` impl and `call`.
- The standard library's `std::task::Wake` (`library/alloc/src/task.rs`,
  readable with `rustup component add rust-src`).

## Exercises

| Exercise | Goal |
|---|---|
| `c24_e01_future_by_hand` | Implement `Future` for two types and drive them with a hand-written poll loop |
| `c24_e02_waker_and_timer` | A leaf future woken from another thread; avoid lost wake-ups |
| `c24_e03_mini_executor` | `block_on` and a task-queue executor using only std |
| `c24_e04_pin_and_unpin` | Pin projection with `pin-project-lite`; `pin!` vs `Box::pin` |
| `c24_e05_stream_by_hand` | `Stream::poll_next` for a source and a lag-mapping adapter |
| `c24_e06_cancellation_and_drop` | RAII under cancellation, cancellation-safe reads, `JoinError` |
| `c24_e07_tower_service` | A tower `Service` and an auth `Layer`; layer order |
| `c24_e08_checkpoint_part3` | Checkpoint: a bounded, deadline-limited, shutdown-aware fan-out |

## Further reading

- *Asynchronous Programming in Rust* (the Async Book), chapters "Under the
  Hood: Executing Futures and Tasks" and "Pinning":
  <https://rust-lang.github.io/async-book/>
- Tokio tutorial, "Async in depth": <https://tokio.rs/tokio/tutorial/async>
- `std::pin` module docs: <https://doc.rust-lang.org/std/pin/index.html>
- `pin-project-lite`: <https://docs.rs/pin-project-lite>
- "Inventing the Service trait" (Tokio blog): <https://tokio.rs/blog/2021-05-14-inventing-the-service-trait>
- tower docs: <https://docs.rs/tower>
- Tokio docs on cancellation safety in `select!`: <https://docs.rs/tokio/latest/tokio/macro.select.html>
- *Rust for Rustaceans*, chapter 8, "Asynchronous Programming"

## Check yourself

1. What must a future do before returning `Poll::Pending`, and what happens if it doesn't?
2. Why can an `async` block that holds a reference across an `.await` not be `Unpin`?
3. `Pin::new(&mut fut)` fails with E0277. Name two ways to get a pinned future instead.
4. In a hand-written executor, why must the queue lock be released before a task is polled?
5. A `select!` loop drops the losing future. What does "cancellation safe" mean for the future in the other branch?
6. Given `ServiceBuilder::new().layer(A).layer(B).service(S)`, which layer sees the request first?
7. How does the databroker's `Authorization` interceptor relate to tower's `Service` trait?

<details><summary>Answers</summary>

1. It must arrange for `cx.waker()` to be woken later, typically by storing a
   clone where the producer will find it. If it doesn't, nothing polls the
   task again and it hangs for ever, with no error.
2. The reference points into the future's own storage (the state machine),
   so moving the future would leave it dangling. The compiler therefore makes
   the state machine `!Unpin`.
3. `Box::pin(fut)` (on the heap) or `std::pin::pin!(fut)` / `tokio::pin!(fut)`
   (on the stack). Wrapper futures use pin projection instead.
4. Polling may call the task's waker, and `Task::wake` locks the same queue to
   re-queue itself. `std::sync::Mutex` isn't re-entrant, so holding it would
   deadlock.
5. Dropping it partway through loses nothing: any progress (such as a
   received header or message) is either not taken yet or stored outside the
   future, so the next call continues correctly.
6. `A`. The first layer listed is the outermost.
7. `ValServer::with_interceptor` wraps the generated service in tonic's
   `InterceptedService`, which implements `tower::Service`. Its `call` runs
   `Authorization::call` (token check, `Permissions` into extensions) before
   forwarding to the inner service. It's a tower middleware layer.
</details>
