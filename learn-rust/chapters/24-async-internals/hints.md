# Chapter 24 hints

## c24_e01_future_by_hand
`poll` gets `self: Pin<&mut Self>`. For types that are `Unpin` (both of these
are: they only hold plain data), `self.get_mut()` gives you an ordinary
`&mut Self` to work with.

`Ready`: the value lives in an `Option` so you can hand it out exactly once.
`Option::take` is the tool for that.

`Countdown`: always bump `polls`. If `remaining == 0` you're done. Otherwise
decrement, call `cx.waker().wake_by_ref()` (or nobody will poll you again),
and return `Poll::Pending`.

`poll_by_hand`: build a context once with
`Context::from_waker(Waker::noop())`, then loop. Count each call to
`Pin::new(&mut fut).poll(&mut cx)` and return when you see `Poll::Ready(out)`.

## c24_e02_waker_and_timer
Two sides share `Arc<Mutex<Shared>>`. The future's side is: lock, and if
`value` is `Some`, return it. Otherwise store a clone of `cx.waker()` and
return `Pending`.

Store the waker **while still holding the lock** you used to check `value`.
Otherwise `fire` could run between your check and your store, find no waker,
and the wake-up is lost for ever.

The handle's side (`fire`): lock, set `value`, `take()` the waker, **unlock**,
then call `waker.wake()`. Waking while holding the lock risks a deadlock if
the waker re-enters.

To pass `the_latest_waker_is_the_one_woken`, replace the stored waker on every
poll, unless `existing.will_wake(cx.waker())` says it's the same one.

## c24_e03_mini_executor
`block_on`: `let mut fut = std::pin::pin!(fut);`, create
`Waker::from(Arc::new(ThreadWaker { thread: thread::current() }))`, then loop
over `fut.as_mut().poll(&mut cx)`. On `Pending`, call `thread::park()`.

`Task::wake`: push `self` (an `Arc<Task>`) onto `self.queue`. Clone the queue
`Arc` first, because you're moving `self` into it.

`spawn`: `Arc::new(Task { future: Mutex::new(Some(Box::pin(fut))), queue: self.queue.clone() })`,
then push it.

`run`: pop the next task in its own statement, so the queue lock is released
before you poll. The task's waker needs to lock that same queue, and
`std::sync::Mutex` is not re-entrant. Then take the future out of the task's
slot and poll it with `Waker::from(task.clone())`. If it's still `Pending`,
put it back in the slot.

## c24_e04_pin_and_unpin
The error `F cannot be unpinned` means `Pin::new` only accepts `Unpin`
types, and an `async` block holding a reference across an `.await` is not
one. Run `rustc --explain E0277`.

You need `Pin<&mut F>` for the field from `Pin<&mut Self>`, which is pin
projection. Wrap the struct definition in `pin_project! { ... }` and put
`#[pin]` on `inner`. In `poll`, call `let this = self.project();`. Now
`this.inner` is a `Pin<&mut F>` and `this.polls` is a `&mut u32`.

`poll_once`: `let fut = std::pin::pin!(fut);` pins it on the stack, and
`fut.poll(&mut Context::from_waker(Waker::noop()))` does the rest.

## c24_e05_stream_by_hand
`Counter::poll_next`: if `next > end`, return `Ready(None)`. Otherwise
return `Ready(Some(next))` and increment. It never needs `Pending`.
`size_hint` returns `(n, Some(n))`, where `n = end - next + 1` (or 0).

`LagToNone`: `let this = self.project();`, then match on
`this.inner.poll_next(cx)`. There are four cases: `Ready(Some(Ok(t)))`,
`Ready(Some(Err(Lagged(n))))` (add to `*this.missed`), `Ready(None)` and
`Pending`. Pass `Pending` up unchanged. The inner stream already arranged
the wake-up.

## c24_e06_cancellation_and_drop
`InFlight::start` increments and keeps a clone of the `Arc`. `Drop`
decrements. Nothing else is needed: Rust calls `drop` when a cancelled
future is destroyed.

`next_frame`: the bug to avoid is keeping the header only in a local
variable. If the future is dropped while awaiting the body, that local is
gone. Store the header in `self.pending_len` as soon as it arrives. At the
start of the call, only wait for a header if `pending_len` is `None`. Use
`self.rx.recv().await?` so a closed channel returns `None`.

`supervise`: `handle.await` gives `Result<T, JoinError>`. Use
`err.is_cancelled()` for aborts. For panics, `err.into_panic()` gives
`Box<dyn Any + Send>`: try `downcast_ref::<&str>()`, then
`downcast_ref::<String>()`.

## c24_e07_tower_service
`SignalStore`: `poll_ready` returns `Poll::Ready(Ok(()))`. `call` computes a
`Result` and wraps it with `std::future::ready(...)`.

`AuthLayer::layer` builds `Auth { inner, tokens: self.tokens.clone() }`.

`Auth::poll_ready` should return `self.inner.poll_ready(cx)`. You can only
accept work when the inner service can.

`Auth::call`: `req.token.as_deref()`, then `.and_then(|h| h.strip_prefix("Bearer "))`,
then `.and_then(|t| self.tokens.get(t))`. On success, set `req.user` and
return `Box::pin(self.inner.call(req))`. On failure, return
`Box::pin(ready(Err(Error::Unauthenticated(..))))`. Both arms become the same
`BoxFuture` type.

## c24_e08_checkpoint_part3
Strategy, not code:

- Compute one `deadline` up front. `tokio::time::timeout_at(deadline, fut)`
  bounds each call by the same overall budget.
- Pre-fill a results vector with `NotStarted`, one slot per request, and write
  each outcome at its index. Request order then comes for free.
- `workers` spawned tasks pulling from a shared queue give you the concurrency
  cap. Pull in order, so starting order is preserved.
- A broadcast `Receiver` can't be shared by workers. Turn the shutdown
  *event* into *state* that everyone can check before starting new work (for
  example a `watch` channel set by one small watcher task).
- Before each pull, a worker stops if shutdown happened or the deadline has
  passed.
- Don't hold a `std::sync::Mutex` guard across an `.await`.

## c24_e09_review_waker_storage
Two bugs, both about the contract every `Future` impl must keep: if you
return `Pending`, something must be holding a `Waker` that will be woken
once you'd make progress.

Bug 1: `poll` checks `guard.open` and returns `Pending` without ever
storing `cx.waker()` anywhere. Nothing the opener can call later will wake
this task. Add a `waker: Option<Waker>` field to `Shared`, and in the
`else` branch store `cx.waker().clone()` before returning `Pending`.

Bug 2: `open()` sets the flag but never calls `.wake()` on anything, so
even with the waker stored, the waiting task is never told to poll again.
`take()` the stored waker and call `waker.wake()` if it was `Some`, after
setting `open = true`.
