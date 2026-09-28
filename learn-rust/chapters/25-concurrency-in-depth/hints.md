# Chapter 25 hints

## c25_e01_scoped_threads
`values.chunks_mut(size)` gives non-overlapping `&mut [f64]` pieces. Inside
`thread::scope(|s| { ... })`, call `s.spawn(move || ...)` once per chunk. The
`move` moves the chunk *reference* into the thread, not the data.

A scoped thread can return a value: `s.spawn(...)` gives a
`ScopedJoinHandle`, and `handle.join()` returns what the closure returned.
`normalize` has two scopes, one after the other: the first computes the
per-chunk maxima (with `chunks`, read-only), and the second scales (reuse
`scale_in_place`). The shared borrow from the first scope has ended before
the second needs `&mut`.

`histogram`: each thread builds a local `[usize; 256]` and returns it. Add them
together after `join`. There are no shared counters, so there's nothing to
contend on.

## c25_e02_condvar
Always follow this shape: lock, then `condvar.wait_while(guard, |state| still_must_wait)`.
`wait_while` releases the lock while sleeping, re-locks on wake-up, and loops
over spurious wake-ups for you.

- `push` waits on `not_full` while `!closed && len >= capacity`. It then
  either returns `Err(item)` (closed) or pushes and calls
  `not_empty.notify_one()`.
- `pop` waits on `not_empty` while `!closed && items.is_empty()`, then
  `pop_front()`. If that returned something, `not_full.notify_one()`.
- `pop_timeout` is the same, using `wait_timeout_while(guard, timeout, ..)`,
  which returns `(guard, WaitTimeoutResult)`.
- `close` sets `closed` and calls `notify_all()` on **both** condvars. Every
  sleeper must wake and re-check its condition.

## c25_e03_atomics_cas_loop
`record_max`: load the current value. Loop: if `value <= current`, return
`current`. Otherwise
`compare_exchange_weak(current, value, Relaxed, Relaxed)`. `Ok(prev)` means
you won, so return `prev`. `Err(actual)` means someone changed it, so set
`current = actual` and go round again.

`try_acquire`: `permits.fetch_update(AcqRel, Acquire, |n| n.checked_sub(1)).is_ok()`.
`checked_sub` returns `None` at zero, which makes `fetch_update` give up.

`set_once`: a single `compare_exchange(0, value, ..)`, with no loop. You
either win or learn the winner's value from the `Err`. `.map(|_| ())`
converts the `Ok` side.

## c25_e04_memory_ordering
The payload and the flag are different variables. The **flag** carries the
synchronisation, so the payload stores and loads can be `Relaxed`.

Writer: store the four values (`Relaxed`), then `ready.store(true, Release)`.
Reader: `ready.load(Acquire)`. Only if that's `true`, load the values
(`Relaxed`). `std::array::from_fn(|i| ...)` builds the `[u64; 4]`.

`choose_ordering`:
- Counters that nothing else depends on use `Relaxed`.
- A flag that publishes data uses `Release` on the store and `Acquire` on
  the load.
- The two-flag mutual-exclusion case needs `SeqCst`. Acquire/Release doesn't
  order a store to one variable against a later load of a *different* one.

## c25_e05_deadlock_and_lock_ordering
`transfer`: if the ids are equal, return `Ok(())` straight away (locking the
same `Mutex` twice deadlocks on its own). Otherwise pick
`(first, second)` so that `first.id < second.id` and lock them in that order.
Then work out which guard is `from` and which is `to`. Check the funds before
changing anything.

`try_transfer`: loop `max_attempts` times. `lock()` `from`, then
`to.balance.try_lock()`. On `Err(TryLockError::WouldBlock)`, `drop` the
`from` guard, call `thread::yield_now()` and continue. When the loop ends,
return `Err(Busy)`.

## c25_e06_data_parallelism_by_hand
Precompute `let chunks: Vec<&[T]> = items.chunks(chunk_size).collect();`
and a shared `AtomicUsize` counter at 0.

Spawn `threads` scoped workers. Each loops:
`let i = next.fetch_add(1, Relaxed); let Some(chunk) = chunks.get(i) else { break };`
and pushes `(i, chunk.iter().map(&f).collect::<Vec<R>>())` into its own Vec,
which it returns.

After joining, sort all `(index, results)` pairs by index and `flat_map`
them into one `Vec<R>`.

## c25_e07_rwlock_vs_mutex_vs_channels
- **Mutex / RwLock:** `get` locks (for `RwLock`, `read()`) and returns
  `.get(path).copied()`. `set` and `add` need exclusive access (`lock()` /
  `write()`). In `add`, do the read-modify-write under **one** guard with
  `entry(path.to_string()).or_insert(0)`.
- **Actor:** `mpsc::channel::<Command>()`. `thread::spawn(move || { let mut map = HashMap::new(); for cmd in rx { match cmd { ... } } })`.
  `Get` and `Add` carry a reply `Sender`: the actor sends the answer back, and
  the caller creates the reply channel, sends the command and then `recv()`s.
  Store `Mutex::new(Some(tx))` and `Some(handle)` in the struct.

## c25_e08_review_ordering_and_lock_order
Two independent bugs.

Bug 1: `ready_flag_orderings` returns `(Relaxed, Relaxed)`. A `Relaxed`
store puts no constraint on the writes made just before it, so a reader
that sees `ready == true` via a `Relaxed` load has no guarantee it also
sees the `calls`/`errors` values written earlier in the writer's program
order. Change the tuple to `(Ordering::Release, Ordering::Acquire)` — that
pair is exactly what "publish then observe" needs; `calls` and `errors`
themselves can stay `Relaxed` because the flag's Release/Acquire already
covers them.

Bug 2: `transfer` locks `from.balance` then `to.balance` — literally the
order the caller passed them in. A concurrent `transfer(&b, &a, ..)` locks
the same two mutexes in the opposite order, which is the textbook AB-BA
deadlock (see `c25_e05_deadlock_and_lock_ordering`). Compare `from.id` and
`to.id` and always lock the lower id first, then work out which guard is
`from` and which is `to` before checking funds.
