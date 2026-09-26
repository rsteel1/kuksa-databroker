# 36 — Capstone: drive the real databroker

> **Builds on:** chapters 35 (the mini-databroker), 30 (gRPC types), 23 (async traits).

**Why this matters for the databroker.** The `databroker` crate is a library
(`databroker/src/lib.rs`) with a thin binary (`main.rs`) on top. Everything
the binary does, from registering VSS signals to serving gRPC, goes through
public APIs you can call from a test. That is how the project's own unit tests
work, and it's the fastest way to explore behaviour, reproduce a bug, or check
a change before you touch the network. In this chapter you use the real
`DataBroker`, real `Permissions`, a real `SignalProvider` and the real
generated gRPC service trait.

The course crates depend on `databroker = { path = "../databroker",
default-features = false }`, so no TLS, VISS or OpenTelemetry is compiled in.

## Registering and reading signals

```rust
use databroker::broker::DataBroker;
use databroker::permissions;

let broker = DataBroker::default();                       // version/sha = ""
let access = broker.authorized_access(&permissions::ALLOW_ALL);
let id = access.add_entry(name, data_type, change_type, entry_type,
                          description, min, max, allowed, unit).await?;
let dp = access.get_datapoint_by_path("Vehicle.Speed").await?;
```

Almost everything you built in chapter 35 is here under the same name. The
differences are worth knowing:

- `add_entry` takes nine positional arguments (no builder). `main.rs` wraps it
  in `add_kuksa_entry` (`databroker/src/main.rs:54`).
- `allowed` is **one array value**, e.g. `DataValue::StringArray(..)`, not a
  `Vec<DataValue>`. The type must fit the data type, or registration fails.
- Reads return owned clones (`Result<Datapoint, ReadError>`), because a
  reference can't escape the lock guard that lives inside the async fn.
- `map_entries` / `filter_map_entries` / `for_each_entry` give your closure an
  `EntryReadAccess` (`databroker/src/broker.rs:1072`): either the entry, or the
  metadata plus the `ReadError` from the permission check. `metadata()` always
  works; `datapoint()` returns `Err` for unreadable entries.
- `permissions::ALLOW_ALL` is a `lazy_static!` global
  (`databroker/src/permissions.rs:21`). `&ALLOW_ALL` has type
  `&ALLOW_ALL` (a hidden wrapper type) and deref-coerces to `&Permissions`
  wherever a `&Permissions` parameter expects it. New code would use
  `std::sync::LazyLock` (see chapter 12).

## Publishing and subscribing

Publishing is `update_entries` with an `EntryUpdate` that sets only
`datapoint`:

```rust
let update = EntryUpdate {
    datapoint: Some(Datapoint { ts: SystemTime::now(), source_ts: None, value }),
    ..Default::default()
};
access.update_entries([(id, update)]).await   // Result<(), Vec<(i32, UpdateError)>>
```

The real `Datapoint` has two timestamps: `ts` (when the broker received it)
and `source_ts` (when the provider measured it, optional). The gRPC
`PublishValue` handler builds exactly this (`databroker/src/grpc/kuksa_val_v2/val.rs:822`)
via `From<&proto::Datapoint>` (`databroker/src/grpc/kuksa_val_v2/conversions.rs:24`).

`subscribe(entries, buffer_size, interval_ms)` returns
`impl Stream<Item = Option<EntryUpdates>>`. Each `ChangeNotification` wraps an
`EntryUpdate` whose `path` and `datapoint` are filled in. Adapting that stream
with `StreamExt::filter_map` is what `convert_to_proto_stream`
(`databroker/src/grpc/kuksa_val_v2/val.rs:1339`) does for gRPC. You do it into
plain Rust pairs. Tests that assert *no message arrives* use
`tokio::time::timeout` under `#[tokio::test(start_paused = true)]`, so they
finish instantly and deterministically.

## Permissions end to end

Production permissions come from a JWT. The server's interceptor
(`databroker/src/grpc/server.rs:51`) decodes the token, and
`TryFrom<Claims> for Permissions`
(`databroker/src/authorization/jwt/decoder.rs:134`) turns scopes like
`read:Vehicle.Cabin provide:Vehicle.Speed` into builder calls:

```rust
Permissions::builder()
    .add_read_permission(Permission::Glob("Vehicle.Cabin".into()))
    .add_provide_permission(Permission::Glob("Vehicle.Speed".into()))
    .expires_at(exp)
    .build()?          // globs are compiled into a RegexSet here
```

Rules worth memorizing (see `databroker/src/permissions.rs:167`–`:236`):

- Read is implied by actuate, provide and create.
- Provide = may write **datapoints** (sensor values). Actuate = may write
  **actuator targets**. Neither implies the other.
- A glob without `*` matches the path *and everything below it*.
- Expiry beats everything: an expired token fails every check with
  `PermissionExpired` (which the gRPC layer maps to `UNAUTHENTICATED`).

The builder consumes and returns `self` (`fn add_read_permission(mut self, ..)
-> Self`), which is what makes the chained style possible.

## Implementing a SignalProvider

```rust
#[async_trait::async_trait]
pub trait SignalProvider: Send + Sync + 'static {
    async fn update_filter(&self, ..) -> Result<(), (RegisterSignalError, String)>;
    fn is_available(&self) -> bool;
    async fn get_signals_values_from_provider(&self, ids: Vec<SignalId>)
        -> Result<GetValuesProviderResponse, ()>;
}
```

(`databroker/src/broker.rs:198`.) The broker keeps providers as
`Arc<dyn SignalProvider>`. Native `async fn` in traits (Rust 1.75+) isn't
object-safe, so it can't be called through `dyn`. `#[async_trait]` rewrites
each method to return `Pin<Box<dyn Future + Send + '_>>`, which *is*
object-safe. Your `impl` must use the same attribute. The supertraits
`Send + Sync + 'static` exist because providers are shared across tokio
worker threads and stored for as long as the broker likes.

`register_signals(HashMap<SignalId, TimeInterval>, Box<dyn SignalProvider +
Send + Sync>)` (`databroker/src/broker.rs:2067`) checks the provide permission
for each signal and refuses signals that already have an owner. Then
`get_values_broker` (`databroker/src/broker.rs:2200`) does the following:

1. snapshots `(provider, signals it serves)` under a short read lock, so the
   lock is never held while awaiting a provider;
2. checks read permission for every signal first (fail fast, no provider I/O);
3. queries all providers **concurrently** with `futures::future::join_all`
   (`:2262`);
4. falls back to the database per signal when a provider answered only
   partially or failed (`Err(())`);
5. returns entries in **request order**, which is why the response type uses an
   `IndexMap`.

Points 1, 3 and 4 were added in the most recent commits (e1fdfd1, 748a914,
7a210d0). Run `git show 7a210d0` and read the diff. After this chapter it
should be fully understandable. The project's own test doubles are good
models: `StaticValueProvider` (`databroker/src/broker.rs:5349`) and
`PartialResponseProvider` (`:5660`). The production implementation is
`Provider` in `databroker/src/grpc/kuksa_val_v2/val.rs:110`, which forwards
requests over the provider's bidirectional gRPC stream and matches responses
by `request_id`.

## The gRPC layer, in-process

tonic generates a trait with one async method per RPC; the databroker
implements it directly on `DataBroker`
(`databroker/src/grpc/kuksa_val_v2/val.rs:251`):

```rust
use databroker_proto::kuksa::val::v2 as proto;
use proto::val_server::Val;           // the trait must be in scope to call its methods

let mut request = tonic::Request::new(proto::GetValuesRequest { signal_ids });
request.extensions_mut().insert(permissions.clone());   // what the interceptor does
let response = broker.get_values(request).await?;       // Result<Response<_>, Status>
```

Each handler starts the same way: take `Permissions` out of the extensions
(missing → `UNAUTHENTICATED`, e.g. `:262`), resolve `SignalId`s with
`get_signal` (`:1308`), call the broker, and map domain errors to
`tonic::Status` (`to_status_with_code`,
`databroker/src/grpc/kuksa_val_v2/conversions.rs:356`). The protobuf `oneof
signal { string path; int32 id; }` becomes the Rust enum
`proto::signal_id::Signal::{Path, Id}`, wrapped in an `Option` because proto3
fields are optional on the wire.

The project's tests call handlers exactly this way. Compare
`test_get_value_id_ok` (`databroker/src/grpc/kuksa_val_v2/val.rs:1429`). For
streaming RPCs they build a `tonic::Streaming` from encoded frames with
`streaming_request` (`:1407`).

## Mental model

| Piece | Analogy | Guarantee |
|---|---|---|
| `DataBroker` | the building; cheap to hand out keys (`Clone`) | shared state is synchronized internally |
| `AuthorizedAccess` | a visitor badge scoped to one request | every call is checked against the badge |
| `Permissions` builder | filling in a badge form, stamped by `build()` | invalid globs are rejected once, up front |
| `SignalProvider` | a supplier on speed-dial | a slow supplier can't lock the building (snapshot, then await) |
| `Val` trait | the reception desk: protobuf in, status codes out | domain errors map to stable gRPC codes |

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "I need a running server to test gRPC behaviour." | Call the `Val` trait methods on `DataBroker` with a hand-built `tonic::Request`. |
| "Provide lets me set actuator targets." | Provide → datapoints; actuate → targets. |
| "`GetValues` always reads the database." | Registered providers are asked first; the database is the fallback. |
| "`async fn` in my trait is enough for `Arc<dyn Trait>`." | Not object-safe; use `#[async_trait]` (or hand-written boxed futures). |
| "`ALLOW_ALL` is a `Permissions` value." | It's a `lazy_static` wrapper that derefs to one. |

## Smells and idioms

- `add_entry`'s nine positional `Option`s are easy to mix up (min vs max vs
  allowed). Comment each `None` like the project's tests do (`None, // min`).
- Errors as `(RegisterSignalError, String)` tuples and `Result<_, ()>` carry
  little structure. A `thiserror` enum with fields would be more idiomatic
  (chapter 07).
- `get_values` in val.rs uses `proto_datapoint_opt.unwrap()` inside a handler
  (`databroker/src/grpc/kuksa_val_v2/val.rs:311` onward). It's safe today
  because the conversion always returns `Some`, but a type that can't be `None`
  would make that guarantee visible to the compiler.
- Good pattern to copy: hold locks briefly, snapshot the `Arc`s you need, then
  `.await` slow work (`get_values_broker`).

## In the databroker

- `databroker/src/broker.rs:2338` — `DataBroker::new`; `:2352` `authorized_access`.
- `databroker/src/broker.rs:1380` — `add_entry`; `:1441` `get_datapoint_by_path`; `:1503` `map_entries`; `:1514` `filter_map_entries`.
- `databroker/src/broker.rs:1529` — `update_entries`; `:1601` `subscribe`.
- `databroker/src/permissions.rs:47` — `PermissionBuilder`; `:55` `Permission`; `:22` `ALLOW_ALL`.
- `databroker/src/broker.rs:198` — `SignalProvider`; `:2067` `register_signals`; `:2200` `get_values_broker`.
- `databroker/src/grpc/kuksa_val_v2/val.rs:59` — the production `Provider` (mpsc sender + broadcast receiver + request ids).
- `databroker/src/grpc/kuksa_val_v2/val.rs:311` — `get_values`; `:822` `publish_value`; `:909` `open_provider_stream`.
- `databroker/src/grpc/server.rs:51` — the auth interceptor.
- `databroker/src/broker.rs:3127` — `helper_add_int32`, a test helper shared with val.rs tests.

## Exercises

| Exercise | Goal |
|---|---|
| `c36_e01_register_and_read` | `add_entry`, `get_datapoint_by_path`, `map_entries` with `ALLOW_ALL` |
| `c36_e02_publish_and_subscribe` | `update_entries` + adapt the real subscription stream |
| `c36_e03_permissions_denied` | build `Permissions`, observe denied/expired on every path |
| `c36_e04_implement_signal_provider` | implement the real trait, register it, watch `get_values_broker` fall back |
| `c36_e05_grpc_get_values_end_to_end` | call `Val::publish_value`/`get_values` with protobuf messages and check status codes |

## Further reading

- tonic docs: `Request::extensions_mut`, `Status`, `service::Interceptor` — https://docs.rs/tonic
- `async-trait` README (why it exists, what it expands to) — https://docs.rs/async-trait
- "Announcing async fn and return-position impl Trait in traits" (Rust blog, Dec 2023) — https://blog.rust-lang.org/
- KUKSA docs in this repo: `doc/protocol.md`, `doc/authorization.md`, `doc/wildcard_matching.md`
- proto definitions: `proto/kuksa/val/v2/val.proto` (git submodule)

## Check yourself

1. Why do the broker's read functions return owned `Datapoint`s rather than references?
2. Which permission does a provider need to publish sensor values, and which does an app need to set an actuator target?
3. What does `get_values_broker` do if a provider answers only some of the requested signals?
4. Why does the `SignalProvider` trait use `#[async_trait]`?
5. How does a gRPC handler know who is calling?
6. Which `tonic::Code` does a value outside the min/max range produce, and where is that mapping defined?

<details><summary>Answers</summary>

1. The data sits behind an async `RwLock` guard created inside the function; a
   reference into it can't outlive the guard, so the function returns a clone.
2. Provide (datapoints); actuate (actuator targets). Both imply read.
3. It uses the provider's values and falls back to the database for each
   missing signal, keeping request order (commit 7a210d0).
4. Providers are stored as `Arc<dyn SignalProvider>`, and native `async fn` in
   traits isn't object-safe; `async_trait` boxes the futures to make it so.
5. The interceptor inserted `Permissions` into the request extensions; the
   handler reads them with `request.extensions().get::<Permissions>()`.
6. `InvalidArgument`, via `UpdateError::to_status_with_code` in
   `grpc/kuksa_val_v2/conversions.rs`.
</details>
