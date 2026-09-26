# 99 — Reading and contributing to the databroker

**Why this matters.** You now know every Rust feature the databroker uses.
This chapter puts them together. It traces four real requests through the
code, line by line, and ends with a workflow and ideas for your first
contributions.

Open the files side by side while you read. Every reference is a real
`path:line` in this repository (line numbers were accurate when this was
written, so `grep` for the function name if the code has moved).

---

## Startup: from `main` to a listening server

| Step | Where | Concepts (chapter) |
|---|---|---|
| Parse CLI flags and env vars with the clap *builder* API | `databroker/src/main.rs:232` onward | clap, `env` (31) |
| Build a multi-thread Tokio runtime manually (not `#[tokio::main]`) so the worker count is configurable | `main.rs:417` | runtime (20) |
| `runtime.block_on(async { ... })`: everything after this is async | `main.rs:423` | futures (20) |
| Create the broker and a full-access handle | `main.rs:439-440` | `Clone` handles of `Arc`s (12), `lazy_static` `ALLOW_ALL` (12) |
| Load VSS JSON files into the database | `read_metadata_file`, `main.rs:153` | serde (28), `?` (07) |
| Choose TLS or insecure | `main.rs` `#[cfg(feature = "tls")]` blocks | cfg features (08, 31) |
| Serve gRPC; pass `shutdown_handler()` as a *future* | `main.rs:626`, `main.rs:40` | `select!` over signals (20, 31) |
| Start housekeeping, add services, attach the JWT interceptor, serve until the shutdown future completes | `grpc/server.rs:190-268` (`start_housekeeping_task` at `:205`) | generics with `where` bounds (09), tonic (30) |

Look closely at `serve_with_incoming_shutdown`'s signature
(`grpc/server.rs:190`). Its generic parameters (`F: Future`,
`I: Stream<Item = Result<IO, IE>>`, `IO: AsyncRead + AsyncWrite + ... + 'static`)
let the same function serve both TCP and Unix sockets. That's static
dispatch (chapter 09) in a real design.

---

## Lifecycle 1: `PublishValue` → every subscriber is notified

A provider calls `PublishValue(Vehicle.Speed = 42)`.

1. **Interceptor** — `grpc/server.rs:51`. It reads the `authorization`
   metadata header, decodes the JWT, turns the claims into `Permissions`
   (via `TryFrom`), and stores them in `request.extensions_mut()`. When
   authorization is disabled, it inserts `ALLOW_ALL` instead.
   *Concepts: traits (`Interceptor`), `match` on nested `Result`s, type-keyed
   extension map.*
2. **Service method** — `grpc/kuksa_val_v2/val.rs:822` `publish_value`.
   It takes the `Permissions` back out of the extensions (returning
   `unauthenticated` if they're missing) and builds an `AuthorizedAccess`
   with `self.authorized_access(&permissions)`. That's a *borrowing view*
   that can't outlive the broker or the permissions (chapter 11).
3. **Resolve the signal** — `get_signal`, `val.rs:1308`: path or id →
   `i32` id, including the permission check.
4. **Convert the value** — `broker::Datapoint::from(&data_point)` uses the
   `From` impl in `grpc/kuksa_val_v2/conversions.rs:24` (chapters 09, 30).
5. **Build an `EntryUpdate`** that sets only `datapoint` and leaves every
   other field `None`, meaning "don't change it". See the tri-state
   `Option<Option<_>>` note at `broker.rs:256-268` (chapter 04).
6. **`update_entries`** — `broker.rs:1529`. This is the heart of the path:
   - `self.broker.database.write().await` takes the **write lock**.
   - For each update, `DatabaseWriteAccess::update` (`broker.rs:1202`):
     permission checks → `Entry::diff` (`:276`) drops unchanged values for
     `OnChange` signals → `Entry::validate` (`:302`) checks the type,
     min/max and allowed values → `Entry::apply` (`:758`) mutates and
     returns the set of changed `Field`s.
   - `db.downgrade()` (`broker.rs:1559`) turns the write guard into a
     **read guard atomically**. Other readers can proceed, but no writer can
     sneak in before subscribers see a consistent snapshot (chapter 21).
   - `subscriptions.read().await.notify(Some(&changed), &db)` (`:824`)
     loops over every `ChangeSubscription`.
7. **Per-subscription notify** — `broker.rs:923`. It applies the
   `interval_duration` rate limit, checks whether the subscriber's field set
   overlaps the changed set (`is_disjoint`, chapter 05), builds an
   `EntryUpdates`, and calls `self.sender.send(...)` on a **`broadcast`**
   channel (`:995`).
8. **Back out to the client** — each subscriber holds the other end as a
   `BroadcastStream` created in `subscribe` (`broker.rs:1665`). A lagging
   subscriber gets `Lagged(n)`, which is mapped to `None` and logged. The
   gRPC layer maps it again with `convert_to_proto_stream` (`val.rs:1339`)
   and tonic streams it to the client (chapter 22).
9. **Errors** — `update_entries` returns `Err(Vec<(i32, UpdateError)>)`.
   `publish_value` turns the first one into a `tonic::Status` with
   `to_status_with_code` (`conversions.rs:356`).

Questions to test yourself:

- Why is the write lock downgraded instead of released and then re-acquired as a read lock?
- What happens to a subscriber whose client disconnected? (Follow `receiver_count()` into housekeeping below.)
- Why does `notify` take `&Database` rather than locking the database itself?

---

## Lifecycle 2: `Subscribe`

1. `subscribe` — `val.rs:383`. It resolves every path to an id with the
   field set `{Datapoint}`, and reads the optional `min_sample_interval` filter.
2. `AuthorizedAccess::subscribe` — `broker.rs:1601`:
   - validates `buffer_size` (at most `MAX_SUBSCRIBE_BUFFER_SIZE`), where
     the channel capacity is `buffer_size + 1`;
   - creates `broadcast::channel(capacity)`;
   - **sends an initial notification** with the current values of every
     requested signal (`notify(None, ...)`, `:1642`) while holding only a read lock;
   - registers the subscription under a fresh `Uuid`;
   - tells interested providers about the new sampling interval
     (`propagate_new_filter_to_provider`, `:2011`, using
     `filter/filter_manager.rs`);
   - returns `impl Stream<Item = Option<EntryUpdates>>`.
3. The gRPC layer boxes it as `Pin<Box<dyn Stream + Send + Sync + 'static>>`
   (`type SubscribeStream`, `val.rs:369`), because a trait's associated
   type must be one concrete type (chapters 09, 22).

---

## Lifecycle 3: a provider connects with `OpenProviderStream`

This is the most complex async code in the repository. Read it after
chapters 20–23.

1. `open_provider_stream` — `val.rs:909`. It receives a
   `tonic::Streaming<OpenProviderStreamRequest>` (the client→server half)
   and must return a `Stream` (the server→client half).
2. It creates an **mpsc** channel (`:929`). The receiver becomes the
   response stream (`ReceiverStream`, `:1072`), and the sender is cloned
   wherever the broker needs to talk to this provider.
3. It creates a **broadcast** channel for `GetProviderValueResponse`s
   (`:930`), then `tokio::spawn`s a task (`:933`) that owns everything it
   needs (`move` closure plus a cloned `DataBroker`, chapter 20) and loops on:

   ```rust
   select! {
       message = stream.message() => { /* dispatch on req.action */ }
       _ = shutdown_trigger.recv() => break,
   }
   ```

4. Dispatch per `oneof` action (chapter 30):
   - `ProvideSignalRequest` → `register_provided_signals` (`val.rs:1162`).
     It builds a `Provider { sender, receiver, next_request_id }` and passes
     it as `Box<dyn SignalProvider>` to `register_signals` (`broker.rs:2067`),
     which stores it as `Arc<dyn SignalProvider>`. That's why the trait uses
     `#[async_trait]` (chapter 23).
   - `PublishValuesRequest` → `publish_values` (`val.rs:1252`). It checks
     that this provider owns the signals (`valid_provider_publish_signals`,
     `broker.rs:2185`), then calls the same `update_entries` as Lifecycle 1.
   - `GetProviderValueResponse` → forwarded into the broadcast channel
     (`val.rs:1034`), where a pending `get_signals_values_from_provider`
     call is waiting for it (Lifecycle 4).
   - `ProvideActuationRequest` → `provide_actuation` (`val.rs:1090`), which
     makes this provider the actuator owner for those signals.
5. When the client disconnects, `stream.message()` returns `Ok(None)` or
   `Err`, and the loop breaks. The task ends and drops the channel ends,
   so `Provider::is_available()` (`sender.is_closed()`, `val.rs:150`)
   becomes false. Within a second, housekeeping removes the provider
   (`cleanup_signal_providers_subscriptions`, `broker.rs:872`) and sends
   `None` to affected subscribers.

---

## Lifecycle 4: `GetValues` when providers are involved

This path was hardened in the three most recent commits (`git log -3`):
e1fdfd1, 748a914 and 7a210d0. Read them with `git show <sha>`.

1. `get_values` — `val.rs:311` resolves ids and calls
   `get_values_broker` (`broker.rs:2200`).
2. **Snapshot, then await.** The code clones the `Arc<dyn SignalProvider>`s
   and their signal subsets *while holding the subscriptions read lock*,
   then drops the lock before calling any provider (`broker.rs:2210-2231`).
   Holding it across a slow provider would block every subscribe and publish
   (commit e1fdfd1, chapter 21).
3. **Validate before any I/O**: missing or unauthorized signals fail fast (`:2233`).
4. **Query providers concurrently** with `futures::future::join_all`
   (`:2254-2262`, commit 748a914), so latency equals the slowest provider,
   not the sum of all of them (chapter 20).
5. Inside each provider: `Provider::get_signals_values_from_provider`
   (`val.rs:154`) uses **one deadline** for every phase (lock, send, receive)
   via `saturating_duration_since`, allocates a `request_id` with
   `AtomicU32::fetch_add(Relaxed)`, and skips responses whose id doesn't
   match (chapter 23).
6. **Partial responses fall back to the database** per signal (`broker.rs:2274-2292`,
   commit 7a210d0).
7. **Order is preserved** with `IndexMap` (`:2321-2329`), because the API
   promises results in request order (chapter 05).

---

## Housekeeping

`DataBroker::start_housekeeping_task` (`broker.rs:2362`) is a spawned loop
driven by `tokio::time::interval(1s)`. Each tick it:

- removes actuation providers that are gone or expired (`Subscriptions::cleanup`, `:857`);
- removes signal providers that are gone or expired and tells affected
  subscribers by sending `None`;
- removes change subscriptions whose stream was dropped
  (`sender.receiver_count() == 0`, `:895`). This is RAII at work: dropping
  the client's stream drops the `Receiver` (chapter 12);
- recomputes provider sampling filters and sends them to providers, with each
  call bounded by `PROVIDER_UPDATE_FILTER_TIMEOUT` (`:2477`).

Notice that each step takes and releases the `subscriptions` lock
separately. That's lock scoping (chapter 12): short critical sections and no
guard held across an unrelated `.await`.

---

## File-by-file tour (suggested reading order)

| # | File | Read it for | Size |
|---|---|---|---|
| 1 | `databroker/src/types.rs` | `DataType`, `DataValue`, cross-type comparisons | ~1.1k lines, mostly tests |
| 2 | `databroker/src/permissions.rs` | builder, `PathMatcher`, `can_*` checks | 300 |
| 3 | `databroker/src/glob.rs` | VSS wildcard → glob, validation | 3k, mostly tests |
| 4 | `databroker/src/vss.rs` | serde parsing and tree flattening | 700 |
| 5 | `databroker/src/broker.rs` 1–1380 | data model, validation, lifetimed access views | |
| 6 | `databroker/src/broker.rs` 1373–2521 | `AuthorizedAccess`, subscribe, providers, housekeeping | |
| 7 | `databroker/src/broker.rs` tests (2523+) | test doubles such as `HangingSignalProvider` | |
| 8 | `databroker/src/filter/filter_manager.rs` | lowest sampling interval per signal | 250 |
| 9 | `databroker/src/grpc/kuksa_val_v2/conversions.rs` | proto ↔ broker conversions | 550 |
| 10 | `databroker/src/grpc/kuksa_val_v2/val.rs` | the v2 service (skim v1 afterwards) | 4k, half tests |
| 11 | `databroker/src/grpc/server.rs` | interceptor, TCP/UDS serving, shutdown | 270 |
| 12 | `databroker/src/authorization/jwt/*` | token decoding, scope parsing | 600 |
| 13 | `databroker/src/main.rs` | wiring it all together | 650 |
| 14 | `databroker/src/viss/` | the optional WebSocket protocol (axum) | 1.9k |
| 15 | `databroker-cli/src/` | a client, useful for seeing the API from outside | 1.7k |

While reading, keep a notebook with three columns: *pattern*,
*where it appears*, and *why it's needed here*.

---

## Development workflow

From the repository root, inside the dev container:

```sh
cargo fmt -- --check                                   # CI checks this
cargo clippy --all-targets -- -W warnings -D warnings  # CI checks this
cargo clippy --features viss --all-targets -- -D warnings
cargo test -p databroker                               # unit + cucumber tests
cargo test -p databroker broker::tests::test_subscribe # one test by name filter
RUST_LOG=debug cargo run --bin databroker -- --insecure --vss data/vss-core/vss_release_4.0.json
```

The CI definitions are in `.github/workflows/kuksa_databroker_build.yml`. The
pre-commit hooks (`.pre-commit-config.yaml`) run fmt, check and clippy.
`CONTRIBUTING.md` describes the Eclipse contribution process, which includes
signing the Eclipse Contributor Agreement.

A good loop for any change:

1. Write a failing test next to similar ones (for example in `broker.rs`'s `mod tests`).
2. Make it pass with the smallest change.
3. Run fmt and clippy, then the full test suite.
4. Read your diff as a reviewer would: which locks are held, what gets cloned, which errors are ignored?

---

## First contribution ideas (real TODOs in the code)

Start small and ask on the project's issue tracker before larger work.

| Idea | Where | Skills |
|---|---|---|
| Better floating-point equality in `DataValue` comparisons | `types.rs:440-470` TODOs | 04, 09, 32 |
| Decide whether `diff` should compare timestamps, and add tests either way | `broker.rs:279` | 03, 32 |
| Make the JWT `aud` claim configurable | `authorization/jwt/decoder.rs:120`, plus a clap flag in `main.rs` | 07, 31 |
| Set a disconnected provider's signals to "not available" and notify subscribers | `broker.rs:2406` TODO | 21, 35 |
| Reduce the nesting in `open_provider_stream` by extracting one function per action | `val.rs:938-1068` | 20, 22; a pure refactor, so keep behaviour identical and rely on the existing tests |
| Drop `lazy_static!` for `ALLOW_ALL`: its fields are all `None`/plain enum variants, so a plain `static` compiles (otherwise `std::sync::LazyLock`) | `permissions.rs:16-21` | 12 (see guide 12) |
| Replace `(ErrorEnum, String)` error tuples with `thiserror` types | `ActuationError`, `RegisterSignalError` users | 07, 17; larger change, discuss first |

## Issues found while writing this course

These came from reading the code carefully while building the exercises.
Each one has been confirmed by reading the source, but **none has been
reproduced against a running server**. Writing the failing test is the first
step of the contribution, and it's great practice (chapter 32).

| Issue | Where | Why it matters | Related exercise |
|---|---|---|---|
| `None => todo!()` in `From<proto::Value> for DataValue`. `Actuate` with `value: Some(Value { typed_value: None })` passes the `ok_or` check and reaches it | `grpc/kuksa_val_v2/conversions.rs:431`, reached from `grpc/kuksa_val_v2/val.rs:677` | A client-controlled panic in a request handler. Tokio contains it to that task, but it should be `invalid_argument`. Consider `TryFrom` (chapter 09) | c17, c30 |
| Check-then-act race in `register_signals`: the "already registered?" check runs under a **read** lock and the insert under a separate **write** lock | `broker.rs:2076-2107` | Two providers registering the same signal at the same time could both succeed. Fix by checking and inserting under one write guard | c21, c23 |
| `subscribe` releases the database lock between the initial notification and registering the subscription | `broker.rs:1639-1652` | An update published in that gap reaches neither the initial snapshot nor the stream | c35_e05 (optional part) |
| Copy-pasted error message: the sensor and attribute branches both say "data_type required for actuator" | `vss.rs:460`, `vss.rs:489` | Misleading error for users writing custom VSS files. Trivial first PR | c28 |
| The scope regex is recompiled on every token parse | `authorization/jwt/scope.rs:33` | Per-request cost. Use `LazyLock<Regex>` (chapters 12, 29) | c29_e02 |
| `last_emitted` is a Tokio `RwLock<Instant>` that is never held across `.await` | `broker.rs:244` | A `std::sync::Mutex` (or an atomic timestamp) is simpler and cheaper (chapter 21) | c21 |
| README still mentions `sdv.databroker.v1`, which is no longer in the code | `README.md` | Documentation drift | — |

## Check yourself

1. Which lock does `update_entries` hold while notifying subscribers, and why that one?
2. How does a subscriber that disconnects get cleaned up? Name the method and the condition.
3. Why is `SignalProvider` stored as `Arc<dyn SignalProvider>` and not a generic `T`?
4. What would break if `get_values_broker` awaited providers while holding the subscriptions lock?
5. In `OpenProviderStream`, which channel carries messages *to* the provider, and which carries `GetProviderValueResponse`s back into the broker?

<details><summary>Answers</summary>

1. A **read** lock on the database, obtained by `downgrade()`ing the write
   guard. Updates are finished, but no writer may interleave while
   subscribers build their notifications from the database.
2. Housekeeping's `cleanup_change_subscriptions` removes subscriptions
   whose `sender.receiver_count() == 0`, which happens after the client's
   stream (and therefore its `Receiver`) has been dropped. Subscriptions
   with expired permissions are removed at the same time.
3. Many providers with different concrete types live in one collection, and
   they arrive at runtime, so this needs dynamic dispatch. `Arc` lets
   the broker clone a handle out of the lock and use it after releasing the lock.
4. A slow or hung provider would block every operation that needs the
   subscriptions write lock (subscribe, provider registration,
   housekeeping), and because publishing needs the read lock to notify, a
   queued writer could stall everything.
5. The mpsc `response_stream_sender` sends to the provider (it becomes the
   response stream). The broadcast `get_value_sender` carries
   `GetProviderValueResponse`s to the waiting `get_signals_values_from_provider`.
</details>
