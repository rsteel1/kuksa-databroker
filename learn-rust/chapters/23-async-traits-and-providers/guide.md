# 23 — Async traits & the provider pattern

**Why this matters for the databroker.** A *provider* is the component that
owns a signal's real value — e.g. a CAN bus reader feeding `Vehicle.Speed`,
or an ECU that can actually move a seat when `Vehicle.Cabin.Seat...Position`
is actuated. The broker doesn't care how a provider works; it talks to all of
them through two traits, `ActuationProvider` and `SignalProvider`
(`databroker/src/broker.rs:189` and `:198`). Their methods are async, the
broker stores providers of different types in one table, and it must survive
providers that are slow, lie, or disappear. This chapter ties together
everything from chapters 09 and 20–22.

## async fn in traits

Since Rust 1.75 you can write `async fn` directly in a trait:

```rust
trait SignalSource {
    async fn read(&self, id: i32) -> Option<f64>;
}

async fn read_all<S: SignalSource>(source: &S, ids: &[i32]) -> Vec<Option<f64>> { ... }
```

With a generic `S: SignalSource`, the compiler knows the concrete type at each
call site and generates specialised code (monomorphization — chapter 09). Each
impl's `read` returns its own anonymous future type. No boxing, no dynamic
dispatch: zero cost.

Two limits matter for the databroker:

1. **Not dyn compatible.** Because every impl's future has a different,
   unnameable type, `dyn SignalSource` has no single signature to put in a
   vtable (E0038 — see next section).
2. **`Send` is not promised.** A generic caller can't assume the returned
   future is `Send`, so spawning it may fail. (For public traits rustc warns
   with the `async_fn_in_trait` lint for this reason; the workaround is
   `fn read(&self) -> impl Future<Output = ...> + Send`.)

## Why #[async_trait] for dyn

The broker keeps *all* providers in one collection —
`HashMap<Uuid, SignalProviderSubscription>` where each entry holds an
`Arc<dyn SignalProvider>`. That requires a trait object, and trait objects
require every method to have one concrete signature.

The `async-trait` crate rewrites

```rust
#[async_trait::async_trait]
pub trait SignalProvider: Send + Sync + 'static {
    async fn get_signals_values_from_provider(&self, ids: Vec<SignalId>)
        -> Result<GetValuesProviderResponse, ()>;
}
```

into roughly

```rust
pub trait SignalProvider: Send + Sync + 'static {
    fn get_signals_values_from_provider<'a>(&'a self, ids: Vec<SignalId>)
        -> Pin<Box<dyn Future<Output = Result<GetValuesProviderResponse, ()>> + Send + 'a>>;
}
```

Now every impl returns the *same* type — a boxed, pinned, `Send` future — so
`dyn SignalProvider` works. Cost: one heap allocation per call, negligible
next to a network round trip. Both the trait **and every impl** need the
attribute. (tonic uses the same mechanism: `#[tonic::async_trait] impl
proto::val_server::Val for broker::DataBroker` at
`databroker/src/grpc/kuksa_val_v2/val.rs:250`.)

**The supertraits.** `SignalProvider: Send + Sync + 'static` is not decoration:

* `Arc<T>` is `Send` only if `T: Send + Sync`. Providers are shared between
  the connection task, the housekeeping task and request handlers on
  different threads.
* `'static`: the provider is stored in a long-lived table, so it can't borrow
  anything temporary.
* `#[async_trait]` futures are `Send` by default, which requires `&self` to be
  `Send`, i.e. `Self: Sync`.

`ActuationProvider` declares no supertraits but the broker always stores it as
`Arc<dyn ActuationProvider + Send + Sync + 'static>` (`databroker/src/broker.rs:218`)
— same requirement, spelled at the use site.

Rust is working on native dyn-compatible async traits; until that lands,
`#[async_trait]` is the standard answer.

## The provider registry

When a provider connects (`OpenProviderStream`) and sends
`ProvideSignalRequest`, the connection task:

1. builds a `Provider` value holding an `mpsc::Sender` back to the client
   (`databroker/src/grpc/kuksa_val_v2/val.rs:59`);
2. calls `register_signals` (`databroker/src/broker.rs:2067`), which checks
   permissions, rejects signals that already have a provider
   (`SignalAlreadyRegistered`), wraps the provider with
   `Arc::from(Box<dyn SignalProvider>)` and stores it under a fresh `Uuid`;
3. keeps the `Uuid` in `local_provider_uuid` for later messages.

**Liveness.** `Provider::is_available` is `!self.sender.is_closed()`
(`databroker/src/grpc/kuksa_val_v2/val.rs:150`): when the client disconnects,
tonic drops the response stream, the receiver is gone, the sender reports
closed. Every second the housekeeping task calls
`cleanup_signal_providers_subscriptions` (`databroker/src/broker.rs:872`),
which uses `HashMap::retain` to drop dead providers (or ones whose JWT
expired) and returns their signals, so subscribers of those signals can be
told (`send_none_to_receiver`).

## GetValues: concurrent, bounded, with fallback

`AuthorizedAccess::get_values_broker` (`databroker/src/broker.rs:2200`)
answers a `GetValues` request. Its current shape is the result of three
recent commits — read them with `git show <hash> -- databroker/src/broker.rs`:

| Commit | Problem | Fix |
|---|---|---|
| e1fdfd1 | `subscriptions` lock held while awaiting providers → a hung provider froze the broker | snapshot `(Arc<dyn SignalProvider>, Vec<SignalId>)` pairs, release the lock, then call |
| 748a914 | providers queried one by one → latency added up | build one future per provider, `futures::future::join_all` |
| 7a210d0 | a provider that answered only *some* signals made the others vanish from the response | per-signal fallback to the database for anything missing |

The algorithm:

1. **Snapshot** providers whose signals intersect the request (`:2210-2231`).
2. **Validate** every provider-served signal against permissions *before*
   any provider I/O (`:2235-2249`) — fail fast, and never ask a provider
   about something the caller may not read.
3. **Query concurrently** (`:2254-2262`).
4. **Merge with fallback**: provider value if present, otherwise the
   database value; provider error → database for all its signals
   (`:2264-2306`).
5. Signals **no provider covers** come from the database (`:2309-2319`).
6. **Preserve request order** with an `IndexMap` (`:2323-2329`), because the
   gRPC contract promises it.

Where's the timeout? Not in `get_values_broker` — it trusts each
`SignalProvider` impl to bound itself, and the gRPC `Provider` impl does
(1 s deadline, next section). In exercise `c23_e04` you put the timeout at
the call site instead; both are valid. Bounding at the call site protects
you from *any* misbehaving impl (including test doubles); bounding in the
impl lets each provider type pick its own budget.

## Talking to a provider over its stream

A gRPC provider has exactly one bidirectional stream. To ask it for current
values, `Provider::get_signals_values_from_provider`
(`databroker/src/grpc/kuksa_val_v2/val.rs:154-247`):

1. sets a **1 s deadline** for the whole exchange (`:158`);
2. locks `self.receiver` — a `tokio::sync::Mutex` around the response
   stream — so only one request/response exchange per provider runs at a
   time (the lock is per provider, never shared with the broker) (`:164`);
3. takes a request id: `self.next_request_id.fetch_add(1, Ordering::Relaxed)`
   (`:174`). `Relaxed` is enough — we need unique numbers, not ordering
   guarantees with other memory;
4. sends `GetProviderValueRequest { request_id, signal_ids }` down the stream;
5. reads responses until the deadline, **skipping any whose `request_id`
   doesn't match** (`:216`) — late answers to an earlier timed-out request;
6. keeps only entries for signals that were actually requested;
7. treats an empty answer as failure so the broker falls back (`:233`).

Responses arrive on the connection task (the big `select!` loop in
`open_provider_stream`) and are forwarded through a `broadcast` channel
(`get_value_sender`, `:930` and `:1035`) to whoever is waiting in step 5.

## Mental model

| Concept | Analogy |
|---|---|
| Trait | A job description: "can answer value requests", "can actuate" |
| Generic `S: Trait` | Hiring for one known person — the contract is compiled in |
| `dyn Trait` | A switchboard: any employee who fits the job description can be plugged in |
| `#[async_trait]` | Gives every employee the same kind of envelope (a boxed future) so the switchboard can handle them uniformly |
| `Arc<dyn Provider>` in a registry | Name badges in a shared directory; many desks can call the same person |
| `is_available` | "Is their phone line still connected?" |
| request id | A ticket number so you don't accept the answer to someone else's question |
| deadline | "If I don't hear back by 10:00 I'll use yesterday's numbers" (the database) |

**Guarantee:** the compiler ensures every provider type implements every
method with the right signature, and that providers shared across threads are
`Send + Sync`. **Not guaranteed:** that providers answer, answer on time, or
answer what you asked — the broker's code must defend against all of that.

## Common misconceptions

| Wrong model | Correct model | You'll see |
|---|---|---|
| "Native async fn in traits makes `dyn Trait` work" | Not (yet) dyn compatible | E0038 |
| "`#[async_trait]` on the trait is enough" | Every impl needs it too | "lifetime parameters or bounds do not match" / E0195 |
| "`Arc<dyn Trait>` is always Send" | Only if the trait object is `Send + Sync` | E0277 at `tokio::spawn` |
| "Concurrent = spawn a task per provider" | `join_all` gives concurrency without spawning | — |
| "A provider response contains what I asked for" | It may be partial, empty, stale or contain extra ids | the 7a210d0 bug |
| "A check under a read lock still holds after it" | Another task can change state before you take the write lock | races (see below) |

## Smells and idioms

* `Box<dyn Trait>` in a table you need to snapshot → `Arc<dyn Trait>`
  (exactly the e1fdfd1 change).
* Error type `()` (`Result<GetValuesProviderResponse, ()>`) throws away *why*
  a provider failed. Fine for "fall back to the database", poor for
  diagnostics — a small error enum would be more idiomatic.
* Errors as `(ActuationError, String)` tuples: workable, but a `thiserror`
  enum with the message inside would compose better with `?` (chapters 07 and 17).
* **Check-then-act across two lock acquisitions.** `register_signals` checks
  for already-registered signals under a *read* lock, releases it, then
  inserts under a *write* lock (`databroker/src/broker.rs:2076-2107`). Two
  providers registering the same signal at the same moment could both pass
  the check. Doing check and insert under one write lock closes that gap.
  Spotting this kind of thing is what reviewing async code is about.
* `return Ok(());` at the end of a function (`databroker/src/grpc/kuksa_val_v2/val.rs:101` and `:147`) — clippy's
  `needless_return`; idiomatic Rust ends with the expression `Ok(())`.

## In the databroker

* `databroker/src/broker.rs:188-208` — the two provider traits.
* `databroker/src/broker.rs:216-237` — `ActuationSubscription` /
  `SignalProviderSubscription` hold `Arc<dyn ...>`.
* `databroker/src/broker.rs:872-889` — `cleanup_signal_providers_subscriptions` (`retain` + `is_available`).
* `databroker/src/broker.rs:2067-2111` — `register_signals`.
* `databroker/src/broker.rs:2200-2334` — `get_values_broker`.
* `databroker/src/broker.rs:2457-2493` — `update_filter_to_providers`: sequential, but each call bounded by `PROVIDER_UPDATE_FILTER_TIMEOUT`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:59-248` — `Provider` and both trait impls.
* `databroker/src/grpc/kuksa_val_v2/val.rs:1130-1160` — `provide_actuation` builds a `Provider` and calls `broker.provide_actuation(.., Box::new(provider))`.
* Tests with hand-written providers (`HangingSignalProvider`,
  `PartialResponseProvider`, `BarrierSignalProvider`) start at
  `databroker/src/broker.rs:5012` — great examples of test doubles for async traits.

## Exercises

| Exercise | Goal |
|---|---|
| `c23_e01_async_fn_in_trait` | Native async trait + generic function |
| `c23_e02_async_trait_for_dyn` | Fix E0038 with `#[async_trait]`; add `Send + Sync` |
| `c23_e03_provider_registry` | Register, look up and clean up `Arc<dyn Provider>`s |
| `c23_e04_bounded_provider_calls` | A simplified `get_values_broker`: concurrent, bounded, fallback, ordered |
| `c23_e05_request_id_correlation` | The provider request/response exchange with ids and a deadline |

## Further reading

* Rust blog — "Announcing `async fn` and return-position `impl Trait` in traits" (Dec 2023): https://blog.rust-lang.org/
* `async-trait` docs (what it expands to, `?Send`): https://docs.rs/async-trait
* Rust Reference — "Dyn compatibility" (formerly object safety): https://doc.rust-lang.org/reference/items/traits.html
* `futures::future::join_all` and `FuturesUnordered`: https://docs.rs/futures
* `git show e1fdfd1`, `git show 748a914`, `git show 7a210d0` — read the tests they added.

## Check yourself

1. Why can't `Vec<Box<dyn Trait>>` hold a trait with a native `async fn`?
2. What does `#[async_trait]` change in the method signature, and what does it cost?
3. Why does `SignalProvider` require `Send + Sync + 'static`?
4. How does the broker notice that a provider disconnected?
5. List the ways a provider's GetValues answer can be "wrong", and how the broker handles each.
6. Why skip responses with a different `request_id` instead of treating them as errors?
7. What race exists in `register_signals`, and how would you fix it?

<details><summary>Answers</summary>

1. Each impl's async method returns a different anonymous future type, so there's no single signature for the vtable — the trait isn't dyn compatible.
2. It returns `Pin<Box<dyn Future<Output = T> + Send + 'a>>` instead of an anonymous future; one heap allocation per call.
3. Providers live in a long-lived shared table (`'static`) and are used from several tasks/threads via `Arc` (`Send + Sync`); async-trait's `Send` futures also need `Self: Sync`.
4. `is_available()` returns `false` once the response channel's receiver is dropped (`sender.is_closed()`); housekeeping removes it within ~1 s.
5. Timeout/no answer, error, empty answer → database fallback for all its signals; partial answer → fallback per missing signal (7a210d0); extra ids → filtered out; stale answer for an older request → skipped by request id.
6. They're answers to an earlier request that timed out; the current request's answer may still come within the deadline.
7. Check (read lock) and insert (write lock) are separate, so two concurrent registrations can both pass the check. Take the write lock once and do both under it.

</details>

## Where this goes next

**Chapter 24 (Async internals)** explains what `#[async_trait]`'s `Pin<Box<dyn Future + Send>>` really is, and how cancellation and task panics behave. Chapter 36 implements the real `SignalProvider` against the real broker.
