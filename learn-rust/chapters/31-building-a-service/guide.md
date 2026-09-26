# 31 — Building a service: config, logging, shutdown

> **Builds on:** chapters 17 (application errors), 20–21 (tasks, select!, broadcast shutdown).

**Why this matters for the databroker.** The broker core (chapters 20–23)
and the gRPC layer (chapter 30) are libraries. `databroker/src/main.rs` is
what turns them into a *service* you can deploy in a car or a container: it
parses configuration from the command line and environment, sets up logging,
builds the Tokio runtime by hand, loads VSS files, picks TLS and
authorization settings, starts listeners on TCP and optionally a Unix
socket, tells systemd it's ready, and shuts everything down cleanly on
SIGTERM. That's ~650 lines of glue that every production Rust service has
some version of. This chapter walks through it.

---

## main.rs at a glance

```text
fn main() -> Result<(), Box<dyn Error>>                         main.rs:232
 ├─ version / commit info from build-time env vars (vergen)     main.rs:233-260
 ├─ clap: Command::new("Kuksa Databroker") + ~15 Args            main.rs:262-410
 │    └─ extra args only if features tls / viss are on          main.rs:348, :382
 ├─ args = parser.get_matches()                                 main.rs:412
 ├─ tokio::runtime::Builder::new_multi_thread()...build()        main.rs:417
 └─ runtime.block_on(async {                                    main.rs:423
      init_logging()                                            main.rs:425
      DataBroker::new(..); authorized_access(&ALLOW_ALL)        main.rs:439-440
      add Kuksa.Databroker.* attributes; read --vss files       main.rs:486
      TLS config (cfg tls), Authorization (JWT public key)       main.rs:492, :544
      optional VISS websocket server (cfg viss) in tokio::spawn
      optional Unix socket server in tokio::spawn               main.rs:578-609
      sd_notify READY (cfg linux)                               main.rs:613
      serve_tcp(.., shutdown_handler()).await                   main.rs:626
      flush OpenTelemetry (cfg otel)                            main.rs:641
    })
```

Two things to notice before diving in:

* `main` returns `Result<(), Box<dyn std::error::Error>>`. Any `?` inside
  propagates up; Rust prints the error's `Debug` form and exits with code 1.
  `Box<dyn Error>` is the "I don't care about the exact type" error — right
  for a binary's top level (the `anyhow` crate is the richer version of the
  same idea). Libraries should expose typed errors instead (chapter 07).
* `main` is **not** `#[tokio::main]`. It builds the runtime manually so the
  number of worker threads can come from `--worker-threads`.

## Configuration with clap

The databroker uses clap's **builder API** — a chain of method calls:

```rust
// databroker/src/main.rs:267
Arg::new("address")
    .display_order(1)
    .long("address")
    .alias("addr")
    .help("Bind address")
    .action(ArgAction::Set)
    .value_name("IP")
    .required(false)
    .env("KUKSA_DATABROKER_ADDR")
    .default_value("127.0.0.1"),
```

and reads values back by string id and type:

```rust
let port = args.get_one::<u16>("port").expect("port should be a number");      // main.rs
let ip_addr = args.get_one::<String>("address").unwrap().parse()?;
if let Some(files) = args.get_many::<String>("vss-file") { ... }                // main.rs:486
if args.get_flag("insecure") { ... }
```

The same interface with the **derive API** (what you write in `c31_e01`):

```rust
#[derive(clap::Parser)]
struct Args {
    /// Bind address
    #[arg(long, alias = "addr", env = "KUKSA_DATABROKER_ADDR", default_value = "127.0.0.1")]
    address: String,
    /// Bind port
    #[arg(long, env = "KUKSA_DATABROKER_PORT", default_value_t = 55555)]
    port: u16,
    /// Populate data broker with VSS metadata from (comma-separated) list of files
    #[arg(long = "vss", alias = "metadata", value_delimiter = ',')]
    vss_files: Vec<String>,
}
let args = Args::parse();
```

| | Builder API | Derive API |
|---|---|---|
| Access | `get_one::<T>("id")` — string ids, type given at *use* site | `args.port` — a typed field |
| Typos | `"prot"` compiles, panics at runtime | misspelt field doesn't compile |
| Dynamic args | Easy: `if cfg!(..) { parser = parser.arg(..) }` | Needs `#[cfg]` on fields or `Command` augmentation |
| Help text | `.help("...")` | the field's `///` doc comment |

The builder fits the databroker because it adds arguments conditionally per
feature (`main.rs:348` adds `--insecure`, `--tls-cert`... only with `tls`).
The derive can do that too, with `#[cfg(feature = "tls")]` on fields.

**Environment variables** (`.env("KUKSA_DATABROKER_ADDR")`) matter for
containers and systemd units: the same image is configured per deployment
without changing the command line. Precedence is **command line > env var >
default**. `--vss` even has `.value_parser(NonEmptyStringValueParser)`, and
`--port` has `value_parser!(u16)` so `--port 70000` is rejected by clap
before your code runs (`main.rs:287`).

Other build-time info: `option_env!("VERGEN_GIT_SHA")` (`main.rs:235`) reads
variables that `databroker/build.rs` emitted via the `vergen-gix` crate
**at compile time** — git SHA, build timestamp, debug/release. `option_env!`
yields `Option<&'static str>` baked into the binary.

## Logging with tracing

`tracing` is the logging/instrumentation framework used across the Tokio
ecosystem. Emitting is separate from collecting:

```rust
use tracing::{debug, info, warn, error};

info!("Starting Kuksa Databroker {}", version);          // format-string message
debug!(path, value, "value updated");                     // structured fields + message
warn!(?request, "odd request");                           // `?x` = record with Debug, `%x` = Display
```

Structured fields (`path="Vehicle.Speed" value=10`) are what make logs
searchable in production. Levels: `error! > warn! > info! > debug! > trace!`.

A **span** is a period of time with context. `#[tracing::instrument]` wraps a
function in a span named after it, recording arguments as fields:

```rust
#[tracing::instrument(skip(value))]      // don't record `value` (large / secret / not Debug)
fn apply_update(path: &str, value: f32) -> bool { ... }
// output: WARN apply_update{path="Vehicle.Speed"}: rejected negative value
```

The databroker writes this conditionally:
`#[cfg_attr(feature = "otel", tracing::instrument(name = "...", skip(...)))]`
— spans are only created when the OpenTelemetry feature is built in, so the
default binary pays nothing for them.

### Initialising the subscriber

Nothing is printed until a **subscriber** is installed.
`init_logging` (`databroker/src/lib.rs:41`, non-otel version):

```rust
let filter = EnvFilter::try_from_default_env()         // reads RUST_LOG
    .unwrap_or_else(|err| { ...; EnvFilter::new("info") });
tracing_subscriber::fmt::Subscriber::builder()
    .with_env_filter(filter)
    .try_init()
    .expect("Unable to install global logging subscriber");
```

`RUST_LOG` syntax: `info`, `debug`, `databroker=debug,tonic=warn`,
`databroker::broker=trace`. Try it:
`RUST_LOG=debug cargo run --bin databroker -- --insecure`.

The `otel` version (`lib.rs:60`) adds an OpenTelemetry layer that exports
spans to a collector (Jaeger etc.) — see `doc/opentelemetry/README.md`.

In tests, `tracing::subscriber::with_default(subscriber, || ...)` installs a
subscriber for one closure only — that's how `c31_e02` captures output.

## The runtime, by hand

```rust
// databroker/src/main.rs:417
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(*worker_threads)   // default: std::thread::available_parallelism()
    .enable_all()                      // I/O driver + timers
    .build()
    .unwrap();

runtime.block_on(async { ... everything else ... })?;
```

`#[tokio::main]` expands to almost exactly this, with defaults. Doing it by
hand lets the thread count be configured (`--worker-threads`, env
`KUKSA_WORKER_THREADS`) — useful on small automotive ECUs where you don't
want a thread per core. `block_on` runs one future to completion on the
current thread while worker threads run spawned tasks.

## Listening: TCP and Unix sockets

* **TCP** (`serve_tcp`, `databroker/src/grpc/server.rs:106`) creates the
  socket with `socket2` to set options (`TCP_NODELAY`, accept backlog
  `MAX_ACCEPT_QUEUE_SIZE`), then hands a `TcpListenerStream` — the listener
  as a `Stream` of connections — to tonic.
* **Unix domain socket** (`serve_uds`, `server.rs:157`): a file path like
  `/run/kuksa/databroker.sock` (`DEFAULT_UNIX_SOCKET_PATH`, `main.rs:18`).
  Local clients skip TCP entirely; filesystem permissions control access.
  A crashed process leaves the socket file behind, so `main.rs:591` unlinks
  a stale socket before binding, and again after the server stops.
* Both call the generic `serve_with_incoming_shutdown<F, I, IO, IE>`
  (`server.rs:190`): "give me any stream `I` of connections `IO` that can
  read/write asynchronously, plus a shutdown future `F`". The trait bounds
  in its `where` clause are a good read-along for chapter 09.

The Unix-socket server runs in its own `tokio::spawn`ed task while the TCP
server is awaited on the main task; both get their own `shutdown_handler()`
future.

## Graceful shutdown

```rust
// databroker/src/main.rs:40
async fn shutdown_handler() {
    let mut sigint  = signal(SignalKind::interrupt()).expect("...");
    let mut sighup  = signal(SignalKind::hangup()).expect("...");
    let mut sigterm = signal(SignalKind::terminate()).expect("...");
    select! {
        _ = sigint.recv()  => info!("received SIGINT"),
        _ = sighup.recv()  => info!("received SIGHUP"),
        _ = sigterm.recv() => info!("received SIGTERM"),
    };
}
```

That future completes on the first signal. It's passed down as `signal: F`
to `serve_with_incoming_shutdown`, which wraps it (`server.rs:95`):

```rust
async fn shutdown<F: Future<Output = ()>>(databroker: DataBroker, signal: F) {
    signal.await;
    info!("Shutting down");
    databroker.shutdown().await;     // clear subscriptions + send on the broadcast trigger
}
// server.rs:264
router.serve_with_incoming_shutdown(incoming, shutdown(broker, signal)).await?;
```

tonic stops accepting connections when that future resolves and waits for
in-flight requests. Long-lived streams would keep it waiting forever — so
`DataBroker::shutdown` also **sends on a `broadcast::Sender<()>`**, and every
provider-stream task has `_ = shutdown_trigger.recv() => break` in its
`select!` loop (`grpc/kuksa_val_v2/val.rs`, `open_provider_stream`).
Subscription streams end because their senders are dropped when
`subscriptions.clear()` runs.

The pattern generalises: **one future for "stop"; a broadcast channel to fan
it out; every long-running task selects on it**. You build exactly this in
`c31_e03`, plus a grace period (`tokio::time::timeout`) for tasks that don't
cooperate.

## systemd readiness: sd-notify

```rust
// databroker/src/main.rs:613
#[cfg(target_os = "linux")]
{
    match sd_notify::booted() {
        Ok(true) => sd_notify::notify(false, &[sd_notify::NotifyState::Ready])?,
        _ => debug!("System is not using systemd, will not try to notify"),
    }
}
```

With a `Type=notify` systemd unit, services that depend on the databroker
start only after it says `READY=1` — i.e. after VSS files are loaded. The
`sd-notify` crate is a Linux-only dependency
(`[target.'cfg(target_os = "linux")'.dependencies]` in
`databroker/Cargo.toml:81`), and the block is `#[cfg]`'d out elsewhere so
macOS builds don't even see the crate.

## Conditional compilation: features and platforms

Cargo **features** (`databroker/Cargo.toml:83`):

```toml
[features]
default  = ["tls"]
tls      = ["tonic/tls-ring", "kuksa-common/tls", "kuksa/tls"]
jemalloc = ["dep:jemallocator"]
viss     = ["dep:axum", "dep:chrono"]
otel     = ["dep:chrono", "dep:opentelemetry", ...]
```

* A feature is a named flag that can switch on **optional dependencies**
  (`dep:axum`) and features of other crates (`tonic/tls-ring`).
* In code: `#[cfg(feature = "viss")]` on items, statements, blocks, struct
  fields, even function **parameters** — `serve_tcp` has
  `#[cfg(feature = "tls")] server_tls: ServerTLS` (`server.rs:106`), and every
  call site marks the matching argument with the same `#[cfg]` (`main.rs:629`).
* Build variants: `cargo build --features viss,otel`,
  `cargo build --no-default-features` (no TLS), `cargo build --all-features`.
  CI runs `cargo hack check --each-feature` to make sure *every* feature
  combination compiles.
* `#[cfg(feature = "jemalloc")] #[global_allocator]` (`main.rs:14`) swaps the
  memory allocator for the whole program behind a flag.

Platform predicates: `#[cfg(target_os = "linux")]`, `#[cfg(unix)]`,
`#[cfg(test)]`, `#[cfg(debug_assertions)]`, combined with `all/any/not`.

`#[cfg(..)]` **removes code before type checking** — the disabled branch
needn't compile. `cfg!(..)` is just a `bool`; *both* branches must compile.

## Mental model

| Concept | Analogy |
|---|---|
| clap `Command` | A form the user fills in on the command line; env vars are pre-filled answers |
| `tracing` event | A structured diary entry; the subscriber decides which ones to keep and where to file them |
| span | A folder that every diary entry written inside it is filed under |
| manual runtime | Choosing the size of the kitchen staff before opening the restaurant |
| shutdown future + broadcast | The fire alarm: one button, every room hears it, everyone walks out |
| `#[cfg]` | Scissors applied to the source *before* compiling |

**Guarantees**: a clap-typed value (`u16`) is valid when you get it; code
behind a disabled `#[cfg]` can't break the build of a configuration that
doesn't use it.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`info!` prints to stdout" | It does nothing until a subscriber is installed. |
| "`RUST_LOG` is read by Rust itself" | It's read by `EnvFilter::try_from_default_env()` in `init_logging`. |
| "`#[tokio::main]` is required" | It's a convenience; building a `Runtime` manually is equivalent and configurable. |
| "Ctrl-C just kills the process" | Only if nothing handles SIGINT; the databroker catches it and shuts down gracefully. |
| "`cfg!(feature = "x")` removes code" | It's a boolean expression; use `#[cfg]` to remove code. |
| "Features are like runtime flags" | They change what is *compiled*; changing them means rebuilding. |
| "`option_env!` reads the environment at runtime" | It reads it at **compile** time; `std::env::var` is runtime. |

## Smells and idioms

* **`unwrap()` in `main`**: `available_parallelism().unwrap()`,
  `runtime.build().unwrap()`, `get_one::<String>("address").unwrap()`. In a
  binary's startup code a panic is acceptable-ish (the process can't do
  anything useful anyway), but `.expect("why")` or `?` give better messages.
  `available_parallelism` *can* fail (e.g. restricted containers); falling
  back to 1 would be more robust.
* **A 400-line `main`**. Splitting it into `fn parse_args()`,
  `async fn load_metadata()`, `fn tls_config()`, `fn authorization()` would
  make each piece testable. The derive API would turn the argument list into
  a typed `Config` struct you can build in tests.
* **String-keyed argument access** (`get_one::<u16>("port")`) — a typo is a
  runtime panic; the derive API makes it a compile error.
* **Good idioms here**: env var for every flag; stale socket cleanup;
  readiness notification; shutdown future passed as a parameter (so tests can
  inject their own, as `databroker/tests/world/mod.rs` does); OpenTelemetry
  flushed on every exit path (`main.rs:641`).

## In the databroker

| Where | What to notice |
|---|---|
| `databroker/src/main.rs:14` | jemalloc as `#[global_allocator]` behind a feature |
| `databroker/src/main.rs:40` | `shutdown_handler`: `select!` over three OS signals |
| `databroker/src/main.rs:153` | `read_metadata_file`: file → VSS → `add_entry` |
| `databroker/src/main.rs:223` | `unlink_unix_domain_socket`: only removes *sockets* |
| `databroker/src/main.rs:262` | clap builder: `Command::new("Kuksa Databroker")` |
| `databroker/src/main.rs:338` | `--worker-threads` / `KUKSA_WORKER_THREADS` |
| `databroker/src/main.rs:348` | TLS-only arguments inside `#[cfg(feature = "tls")]` |
| `databroker/src/main.rs:417` | manual multi-thread runtime |
| `databroker/src/main.rs:544` | `(enable_authorization, jwt_public_key)` tuple match |
| `databroker/src/main.rs:578` | Unix socket: explicit path or `--enable-unix-socket` default |
| `databroker/src/main.rs:613` | `sd_notify` readiness on Linux |
| `databroker/src/lib.rs:41`, `:60` | `init_logging`, plain vs OpenTelemetry |
| `databroker/src/grpc/server.rs:95` | `shutdown()`: await signal → `databroker.shutdown()` |
| `databroker/src/grpc/server.rs:106`, `:157`, `:190` | TCP, UDS, and the generic server |
| `databroker/build.rs` | vergen: git/build info as compile-time env vars |
| `databroker/Cargo.toml:83` | feature definitions |

Run it:

```sh
cargo run --bin databroker -- --help
cargo run --bin databroker -- --insecure --vss data/vss-core/vss_release_4.0.json
KUKSA_DATABROKER_PORT=6000 RUST_LOG=debug cargo run --bin databroker -- --insecure
```

(Pick a VSS file that exists in `data/vss-core/`.) Then press Ctrl-C and
watch the `received SIGINT` / `Shutting down` log lines.

## Exercises

| Exercise | Goal |
|---|---|
| `c31_e01_clap_args_env` | A derive-API `Args` with aliases, env vars, typed defaults, delimiters |
| `c31_e02_tracing_basics` | Events with fields, `#[instrument]`, EnvFilter with fallback |
| `c31_e03_graceful_shutdown` | Signal future → broadcast trigger → `select!`ing workers + grace period |
| `c31_e04_cfg_feature_gates` | `#[cfg]` on functions and fields, `cfg!` expressions |

## Further reading

* clap docs: "Derive tutorial" and "Builder tutorial" (docs.rs/clap, `_derive::_tutorial`, `_tutorial`).
* tracing docs — "Core concepts", `#[instrument]`; tracing-subscriber `EnvFilter` directive syntax.
* Tokio tutorial — "Graceful Shutdown" (tokio.rs/tokio/topics/shutdown).
* `tokio::runtime::Builder` docs.
* Cargo book — "Features", "Build scripts", "Platform specific dependencies".
* Rust reference — "Conditional compilation".
* The Twelve-Factor App — "Config" (why env vars).

## Check yourself

1. Why does the databroker build its Tokio runtime by hand instead of using `#[tokio::main]`?
2. Where does `RUST_LOG` get interpreted, and what happens if it contains garbage?
3. What makes a long-lived provider stream end when the databroker shuts down?
4. Why is the `sd_notify` call wrapped in `#[cfg(target_os = "linux")]` rather than `if cfg!(target_os = "linux")`?
5. What's the difference between `option_env!("VERGEN_GIT_SHA")` and `std::env::var("VERGEN_GIT_SHA")`?
6. How would you run the databroker on port 6000 without changing the command line?

<details><summary>Answers</summary>

1. To set `worker_threads` from `--worker-threads` / `KUKSA_WORKER_THREADS`.
2. In `init_logging` via `EnvFilter::try_from_default_env()`; on error it
   records the error text and falls back to `"info"`.
3. `DataBroker::shutdown` sends on the broadcast shutdown trigger; the stream
   task's `select!` sees `shutdown_trigger.recv()` complete and `break`s.
4. The `sd_notify` crate isn't even a dependency on other platforms, so the
   code must be removed before compilation; `cfg!` would leave it in and fail
   to compile.
5. `option_env!` is evaluated at compile time (baked into the binary);
   `env::var` reads the process environment at runtime.
6. `KUKSA_DATABROKER_PORT=6000` in the environment.

</details>
