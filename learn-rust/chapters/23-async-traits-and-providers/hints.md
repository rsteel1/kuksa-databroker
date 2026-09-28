# Chapter 23 hints

## c23_e01_async_fn_in_trait
`Constant::read`: `Some(self.value)`.
`Table::read`: `tokio::time::sleep(self.latency).await;` then look the id up:
`self.values.get(&id).copied()` (`get` gives `Option<&f64>`; `copied` makes it `Option<f64>`).
`read_all`: loop over `ids`, `push(source.read(id).await)`.

## c23_e02_async_trait_for_dyn
Put `#[async_trait::async_trait]` on the line above `trait Provider` and above
BOTH `impl Provider for ...` blocks — the macro must rewrite the trait and every
impl the same way.

Once that compiles you'll get E0277: `dyn Provider` cannot be sent between
threads safely. `Arc<T>` is `Send` only if `T: Send + Sync`. Make those
supertraits: `trait Provider: Send + Sync { ... }` — exactly like the
databroker's `SignalProvider: Send + Sync + 'static`.

## c23_e03_provider_registry
`register`: first check every existing registration:
`registration.signals.intersection(&signals)` gives the clashing ids —
return the smallest (`.min()`) as `AlreadyProvided`. Only if nothing clashes,
create `Uuid::new_v4()` and insert.

`provider_for`: `self.providers.values().find(|r| r.signals.contains(&signal))`
then `.map(|r| r.provider.clone())` — cloning an `Arc` just bumps a counter.

`cleanup`: `HashMap::retain(|uuid, registration| ...)` keeps entries for which the
closure returns `true`. Before returning `false`, record
`(*uuid, registration.signals.clone())` in a result map declared outside the closure.

## c23_e04_bounded_provider_calls
Suggested steps:

1. For each `(provider, provided)` compute the requested ids it serves, keeping
   request order: `requested.iter().copied().filter(|id| provided.contains(id))`.
   Skip providers where that list is empty (`filter_map` + `bool::then`).
2. For each remaining provider create (don't await!) a future:
   `async move { (ids.clone(), timeout(PROVIDER_TIMEOUT, provider.values(ids)).await) }`
   — clone the `Arc` into it. Then `join_all(...).await`.
3. For each `(ids, answer)`: `Ok(Ok(map))` is a (maybe partial) answer; `Ok(Err(()))`
   (provider failed) and `Err(_)` (timed out) both mean "no values". For every id
   in `ids`, take the provider value or fall back to `database` (`?` on
   `NotFound`). Store in a `HashMap<i32, f64>`.
4. Finally map over `requested` in order: take the collected value, or fall
   back to `database`. `collect::<Result<Vec<_>, _>>()` stops at the first error.

Compare with `get_values_broker` in `databroker/src/broker.rs` — same shape.

## c23_e05_request_id_correlation
Set up `let deadline = Instant::now() + DEADLINE;` and a helper closure
`let remaining = || deadline.saturating_duration_since(Instant::now());`.

1. `let mut inbox = timeout(remaining(), self.inbox.lock()).await.map_err(|_| ())?;`
2. `let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);`
   (`fetch_add` returns the OLD value — the first id is 1).
3. Send with `timeout(remaining(), self.outbox.send(..))` — two layers of
   `Result`, so `.map_err(|_| ())?` twice.
4. `loop`: `timeout(remaining(), inbox.recv())` → two layers again (timeout;
   then `RecvError::Closed`/`Lagged`). `continue` on a wrong `request_id`;
   otherwise filter `entries` to the requested ids and return `Err(())` if empty.

## c23_e06_review_concurrent_and_bounded
Bug 1: `for provider in providers { results.push(provider.value().await); }`
awaits each future to completion before creating the next one, so nothing
ever overlaps. Build every provider's future first (`providers.iter().map(|p| p.value())`)
and hand the whole collection to `join_all` — that's what starts them all
running at once.

Bug 2: a `SemaphorePermit` only limits concurrency for as long as it's
alive; dropping it right after `acquire().await` (before the guarded work
even starts) means the "limit" governs nothing. Keep the `permit` binding
alive across the `provider.value().await` call — drop it (explicitly, or
just by letting it fall out of scope) only after that call finishes.
