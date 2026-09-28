# Chapter 21 hints

## c21_e01_tokio_mpsc_backpressure
`publish`: `tx.try_send(value)` returns `Result<(), TrySendError<i32>>`.
`TrySendError` is an enum with `Full(value)` and `Closed(value)` — match on it.

`drain`: `while let Some(v) = rx.recv().await { ... }` — `recv` yields `None`
once all senders are gone and the buffer is empty.

`consumer_gone`: look for a method on `Sender` that tells whether the
receiver was dropped (`is_closed`). The databroker uses it in `is_available`.

## c21_e02_oneshot_request_reply
`run_store`: own a `HashMap<String, i32>` as a local variable, loop with
`while let Some(command) = commands.recv().await`, and `match` on the command.
For `Get`, answer with `reply.send(values.get(&path).copied())`; ignore the
result (`let _ =`) — the caller may have gone away.

`get`: create `let (reply, response) = oneshot::channel();`, send
`Command::Get { path: path.to_string(), reply }`, then `response.await`.
Both steps can fail (store gone): `.ok()?` turns a `Result` into an early
`None`. `response.await` is `Result<Option<i32>, RecvError>` —
`.ok().flatten()` turns it into `Option<i32>`.

## c21_e03_broadcast_and_lag
Loop on `rx.recv().await` and match three cases:
`Ok(v)` → push; `Err(RecvError::Lagged(n))` → add `n` to the missed count and
KEEP GOING (the receiver is still usable); `Err(RecvError::Closed)` → break.

## c21_e04_shutdown_trigger
`new`: `let (shutdown_trigger, _) = broadcast::channel(1);` — just like
`DataBroker::new`.
`shutdown_trigger`: `self.shutdown_trigger.subscribe()`.
`shutdown`: `send(())` returns `Err` when there are no receivers; ignore it.

`spawn_worker`: call `broker.shutdown_trigger()` BEFORE `tokio::spawn`, move
the receiver into an `async move` block, and loop on
`select! { _ = sleep(1s) => ticks += 1, _ = shutdown.recv() => break }`.
If you subscribe inside the task, `immediate_shutdown_is_not_missed` fails:
the shutdown can be sent before the task ever runs.

## c21_e05_rwlock_downgrade
Steps: `let mut w = self.data.write().await;` → `w.insert(...)` →
`let r = RwLockWriteGuard::downgrade(w);` → `sleep(notify_time).await` →
return `r.len()`. `downgrade` consumes the write guard (takes it by value).

## c21_e06_snapshot_then_await
The read guard lives as long as the variable `providers`, i.e. across every
`.await` in the loop. You need a copy of the list that doesn't borrow the lock.
`Vec<Arc<SlowProvider>>` is `Clone`, and cloning an `Arc` is cheap.

`let snapshot = self.providers.read().await.clone();` — the guard is a
temporary and is dropped at the `;`. Then loop over `&snapshot`.

## c21_e07_deadline_budget
Compute `let deadline = Instant::now() + budget;` once. Before each step:
`let remaining = deadline.saturating_duration_since(Instant::now());` and wrap
the step in `timeout(remaining, ...)`.

`timeout(remaining, tx.send(request)).await` is
`Result<Result<(), SendError<u32>>, Elapsed>` — two layers: map the outer
error to `DeadlineExceeded`, the inner to `Disconnected`, with `?` after each.
`rx.recv()` yields `Option<u32>`: `None` means disconnected (`ok_or`).

## c21_e08_watch_latest_value
A `watch` channel is one slot, not a queue. Methods on `watch::Receiver`:
`borrow()` (peek, doesn't mark seen), `changed().await` (wait for a value
newer than the last one *this receiver* marked seen; `Err` once the sender
is gone), `borrow_and_update()` (read and mark seen), and
`wait_for(predicate).await` (returns immediately if the current value
already matches).

`latest`: `*rx.borrow()`. `next_change`: `rx.changed().await.ok()?;` then
`Some(*rx.borrow_and_update())`. `wait_until_above`:
`rx.wait_for(|v| *v > limit).await.ok().map(|v| *v)`.

## c21_e09_review_shutdown_and_joinhandle
Bug 1: `tokio::spawn(...)` returns a `JoinHandle`, and the exercise's
`spawn_worker` just lets that value fall off the end of the expression —
dropping it detaches the task. A dropped handle keeps running the task in
the background, but nothing can ever `.await` it to know when it's done.
Give `Worker` a `handle: JoinHandle<()>` field, keep what `tokio::spawn`
returns, and `.await` it inside `shutdown`.

Bug 2: `select!` can see the shutdown signal become ready before it
happens to poll the `recv()` branch again, so items already sitting in
the channel never get pulled out through the normal path. Once you're
about to break out on shutdown, drain whatever's left with a
`while let Ok(item) = rx.try_recv() { ... }` loop first.
