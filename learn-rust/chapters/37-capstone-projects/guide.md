# 37 — Capstone projects with no scaffolding

> **Builds on:** chapters the whole course; each project lists the chapters it leans on.

**Why this matters.** Exercises with a `todo!()` in the right place teach
you the pieces. Real work starts from a blank file and a specification. You
decide the types, where state lives, which task owns what and how errors
flow. These three projects are the last step between "I can fix this
function" and "I can build and maintain a Rust service". Each is a
miniature of something the databroker does, so finishing them also means
you could have designed the databroker's core yourself.

Each exercise gives you only the public signatures, a written spec in the
header, and the tests. The hints give design direction, not code. Expect
each project to take a few hours, and plan it on paper before you type.

## How to approach a project

1. **Read every test before writing code.** The tests are the precise
   spec, and the header is the summary. Note every behaviour a test pins
   down that the header mentions only in passing.
2. **Sketch the data flow.** Who owns the state? What crosses a thread or
   task boundary (so it needs `Send`, `Arc` and channels)? Where can things
   fail, and who handles each failure?
3. **Separate the pure core from the I/O shell.** Parse and decide in plain
   functions, and keep sockets, files and tasks thin around them. This is
   the single most useful design habit in Rust, because pure functions are
   easy to test and don't fight the borrow checker.
4. **Make one test pass at a time**, in the order they appear.
5. **Then read the solution** and compare design choices, not just code.
   There are several good designs for each project.

## Project 1: a key-value server

`c37_e01_kv_server` is a TCP server with a line protocol: `SET`, `GET`,
`SUBSCRIBE`, `QUIT`, and graceful shutdown. It's the databroker in
miniature:

| Your server | The databroker |
|---|---|
| `SET` updates the store and notifies subscribers | `publish_value` → `update_entries` → `Subscriptions::notify` (`databroker/src/broker.rs:1529`) |
| One `broadcast` channel as the change feed | a `broadcast` channel per subscription, created in `subscribe` (`broker.rs:1601`) |
| One task per connection | one spawned task per provider stream (`databroker/src/grpc/kuksa_val_v2/val.rs:933`) |
| A `watch` stop signal plus a `JoinSet` | `shutdown_trigger` (`broker.rs:2504`) and tonic's `serve_with_incoming_shutdown` (`databroker/src/grpc/server.rs:95`) |

Design decisions you'll face:

* **Ordering.** If two clients SET the same key, subscribers must see
  UPDATEs in SET order. That means the store update and the publish must
  happen under one lock, and a std `Mutex` is fine because you never await
  while holding it.
* **"Only SETs after the OK".** A connection that subscribes late must not
  get stale changes still buffered in the channel. Sequence numbers solve
  this cleanly.
* **Shutdown.** Stop accepting, tell every connection to stop, then *wait*
  for them. Returning while tasks are still writing to sockets is a bug that
  tests (and production) catch.
* **Slow subscribers.** A `broadcast` receiver that falls behind gets
  `Lagged(n)`. Decide what your protocol does about it. The databroker logs
  it and sends `None` (chapter 22).

The tests talk to your server over real loopback sockets on port 0 (the OS
picks a free port), and every read has a timeout, so a bug fails the test
instead of hanging it.

## Project 2: a VSS signal CLI

`c37_e02_signal_cli` is a complete command-line program: `vss query <FILE>
<GLOB>` and `vss tree <FILE> [--depth N]`. It's what the databroker does at
startup with `--vss` files (`databroker/src/main.rs:153`, `databroker/src/vss.rs:533`),
plus the wildcard rules of `databroker/src/glob.rs:72`.

The one design decision that makes it testable: `run` takes the arguments
and two `impl Write`s and *returns* the exit code, instead of reading
`std::env::args()`, printing directly, and calling `process::exit`. The
real `main` would be three lines:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    std::process::exit(run(&args, &mut std::io::stdout(), &mut std::io::stderr()));
}
```

The spec is also strict about errors: two layers (`cannot read FILE`, then
the OS error as the cause) and conventional exit codes (0 success, 1 "no
match", 2 usage or input errors, as `grep` does). Chapter 17's error-design
rules all apply.

## Project 3: a plugin host

`c37_e03_plugin_system` covers runtime extensibility. Kinds of plugin are
registered as factories, a config file creates named instances, and each
update is delivered to every active instance. One bad plugin, whether it
returns errors or panics, must never take the host down.

This is how the databroker treats providers: they arrive at runtime as
`Arc<dyn SignalProvider>` (`broker.rs:2067`), every call to them is bounded
(`PROVIDER_UPDATE_FILTER_TIMEOUT`, `broker.rs:39`), and housekeeping removes
the ones that are gone (`broker.rs:872`). Your host adds a stronger
guarantee, **panic isolation**:

* `std::panic::catch_unwind` stops a panic at a boundary and turns it into
  a `Result`. It only works when panics unwind. Under `panic = "abort"`
  (common in embedded and some release profiles) the process dies anyway,
  so don't treat it as a general error mechanism.
* `AssertUnwindSafe` is a promise that you won't observe state a panic may
  have left half-updated. You keep that promise by disabling the plugin
  immediately.
* A panic still prints its message to stderr through the panic hook, so
  you'll see "cannot handle NaN" in the test output even though the test
  passes. That's expected.

Tokio gives you the same isolation for free at task level: a panic inside
`tokio::spawn` ends that task, and `JoinHandle::await` returns
`Err(JoinError)` with `is_panic()`. That's why a panicking request handler
in the databroker kills one request, not the server.

## Mental model

| Idea | Analogy |
|---|---|
| Pure core, I/O shell | A calculator inside a phone: the maths doesn't know about the touchscreen |
| Specification by tests | A contract signed before building starts |
| Change feed with sequence numbers | Numbered tickets at a deli counter: you only care about numbers after yours |
| Graceful shutdown | Closing a shop: lock the door, serve the people inside, then turn off the lights |
| Panic isolation | A circuit breaker for one room, not the whole building |

**What the compiler guarantees here:** no data races between connection
tasks, and no forgotten error variants in your `match`es. It does *not*
check your protocol's ordering or shutdown logic. Those are logic properties,
and the tests exist to catch them.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Start coding and the design will emerge." | In Rust, ownership decisions made late are expensive. Sketch who owns what first. |
| "One big `async fn` per connection is simplest." | Pull parsing and decisions into sync functions. The async part should only move bytes and wait. |
| "`catch_unwind` is Rust's `try/catch`." | It's a last-resort boundary for bugs, and it doesn't work with `panic = "abort"`. Expected failures are `Result`s. |
| "Dropping the listener shuts the server down." | Existing connections keep running. You must signal them and wait. |
| "If it works on my machine, the concurrency is right." | Races show up under load. Structure the code so ordering doesn't depend on timing. |

## Smells and idioms

| Smell | Idiomatic |
|---|---|
| `std::process::exit` deep inside the program | Return an exit code or `Result` from `run`, and exit only in `main` |
| `println!` inside logic functions | Take `&mut impl Write`, so output is testable |
| Holding a `MutexGuard` while writing to a socket | Copy what you need, drop the guard, then `.await` |
| A `bool` "running" flag checked in loops | A `watch`/`broadcast` channel or `CancellationToken` that tasks `select!` on |
| Stringly-typed errors all the way up | Typed errors with `#[source]`, rendered as text only at the edge |
| `unwrap()` on I/O in a server | Handle the error and close *that* connection; the server continues |

## In the databroker

| Where | What to notice |
|---|---|
| `databroker/src/broker.rs:1601` | `subscribe`: channel per subscriber, initial notification, stream returned to the caller |
| `databroker/src/broker.rs:2362` | `start_housekeeping_task`: a spawned loop that removes dead subscribers and providers |
| `databroker/src/broker.rs:2495` | `shutdown`: clear subscriptions, then signal the trigger |
| `databroker/src/grpc/server.rs:95` | `shutdown`: await the signal future, then shut the broker down |
| `databroker/src/grpc/server.rs:106` | `serve_tcp`: binding and serving, which is your `run` at production scale |
| `databroker/src/grpc/kuksa_val_v2/val.rs:933` | the per-provider task: `select!` over messages and shutdown |
| `databroker/src/main.rs:309` | the `--vss` argument; `read_metadata_file` at `:153` loads the file |
| `databroker/src/vss.rs:400` | `flatten_vss_tree`: the recursive walk your CLI also needs |
| `databroker/src/glob.rs:72` | `to_glob_string`: `.` → `/`, and "no `*` means the whole branch" |

## Exercises

| Exercise | Goal |
|---|---|
| `c37_e01_kv_server` | TCP key-value server with pub/sub and graceful shutdown |
| `c37_e02_signal_cli` | A complete, testable CLI over VSS JSON files |
| `c37_e03_plugin_system` | Config-driven trait-object plugins with failure and panic isolation |
| `c37_e04_review_kv_server` | Review: 3 planted bugs — a case-sensitive command parser, per-connection state that isn't shared, shutdown that never closes open connections |

## Further reading

* Tokio tutorial, "Shared state", "Channels", "Graceful shutdown" — <https://tokio.rs/tokio/tutorial>, <https://tokio.rs/tokio/topics/shutdown>
* *Command Line Applications in Rust* — <https://rust-cli.github.io/book/>
* `std::panic::catch_unwind` and `UnwindSafe` docs — <https://doc.rust-lang.org/std/panic/fn.catch_unwind.html>
* mini-redis, Tokio's teaching server (much like Project 1) — <https://github.com/tokio-rs/mini-redis>

## Check yourself

1. Why does the KV server's `run` take the shutdown signal as a future rather than listening for Ctrl-C itself?
2. What goes wrong if the store update and the broadcast `send` happen under two separate lock acquisitions?
3. Why does `run` in the CLI project return an `i32` instead of calling `std::process::exit`?
4. When is `catch_unwind` the wrong tool, and what should you use instead?
5. After a panic is caught in the plugin host, why must the plugin be disabled rather than retried?

<details><summary>Answers</summary>

1. Tests and embedders need to trigger shutdown themselves. The real `main`
   passes a signal-handling future, which is exactly what `serve_tcp` does
   with `shutdown_handler()`.
2. Two SETs can interleave between update and publish, so subscribers see
   UPDATEs in a different order than the store applied them, and the final
   UPDATE may not match the stored value.
3. `exit` skips destructors and would end the test process. Returning the
   code keeps `run` a normal, testable function, and only `main` exits.
4. For *expected* failures (use `Result`), under `panic = "abort"` (it can't
   catch anything), and across FFI boundaries (use `extern "C-unwind"` or
   abort). It's for isolating bugs at a boundary.
5. The panic may have interrupted the plugin halfway through updating its
   own state. Using it again would observe that broken state, which is the
   promise `AssertUnwindSafe` made.

</details>
