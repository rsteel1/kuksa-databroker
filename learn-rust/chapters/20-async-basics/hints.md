# Chapter 20 hints

## c20_e01_async_await
The error says you cannot add two `impl Future<Output = u32>` values.
`fetch_speed(1)` is not a number yet — it's a *description* of work that
will produce a number. Which keyword turns a future into its output?

Put `.await` after each `fetch_speed(...)` call.

## c20_e02_tokio_spawn_joinhandle
`tokio::spawn` requires `F: Future + Send + 'static`. `'static` here means
"doesn't borrow anything that could disappear". `for name in &names` gives
you `&String` references into `names`, which is dropped when `greet_all`
returns — possibly before the task runs.

Two changes: iterate `names` by value (`for name in names`) so each loop
iteration owns a `String`, and write `async move { ... }` so the block takes
ownership of `name` instead of borrowing it.

## c20_e03_sleep_and_timeout
`ask_provider`: `sleep(delay).await;` then return `value`.

`ask_provider_with_limit`: `timeout(limit, some_future)` gives back
`Result<T, Elapsed>`. You want `Result<i32, AskError>` — convert the error
type with `.map_err(|_| AskError::TimedOut)`.

## c20_e04_join_all_concurrency
Build an iterator of futures: `delays_ms.iter().map(|&d| query_provider(d))`.
Creating them does not start them (futures are lazy). Pass that iterator to
`join_all(...)` and `.await` the result — it is already a `Vec<u64>` in input order.

## c20_e05_select
Shape:

```rust
let mut collected = Vec::new();
loop {
    select! {
        message = messages.recv() => match message {
            Some(m) => collected.push(m),
            None => break,
        },
        _ = shutdown.recv() => break,
    }
}
collected
```
`recv()` on an mpsc receiver returns `None` once every sender is dropped.

## c20_e06_blocking_and_send
Part 1: the compiler points at the `.await` and says the guard "is held across
this await point". Put the lock in its own block that evaluates to the new
total: `let new_total = { let mut g = total.lock()...; *g += reading; *g };`
The guard is dropped at the closing `}` — before the `.await`.

Part 2: `tokio::task::spawn_blocking(move || expensive_checksum(&data))`
returns a `JoinHandle<u64>`; `.await` it and `.expect(...)` the `JoinError`.

## c20_e07_review_blocking_and_concurrency
Bug 1: `slow_confirm` calls `std::thread::sleep`, not `tokio::time::sleep`.
`std::thread::sleep` blocks the actual OS thread the task is running on —
under `start_paused = true` it also completely ignores the virtual clock,
which is exactly what the test's `start.elapsed() < 10ms` and
`!handle.is_finished()` assertions catch. Swap it for
`tokio::time::sleep(delay).await`.

Bug 2: `poll_three_sensors` awaits `read_sensor` three times in a row —
each `.await` fully finishes before the next one even starts, so the
delays add up. `tokio::join!(read_sensor(..), read_sensor(..), read_sensor(..))`
starts all three at once and waits for the slowest, so under a paused
clock the whole call only advances virtual time by one delay, which is
what the second test measures.
