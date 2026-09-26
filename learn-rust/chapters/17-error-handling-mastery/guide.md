# 17 — Error handling II: mastery

**Why this matters for the databroker (and every other codebase).** Chapter 07
taught the mechanics: `Result`, `?`, custom enums, `From`, thiserror. That's
enough to make code *compile*. This chapter is about making errors *good*:
errors a caller can react to, reports an operator can understand, messages
that never leak secrets to a client, and recovery strategies that make a
system degrade gracefully instead of failing all at once. The databroker
sits between untrusted clients and hardware-facing providers, so every one
of these decisions shows up in its code, sometimes done well and sometimes not.
Reviewing error handling is one of the fastest ways to judge the quality of an
unfamiliar Rust codebase.

---

## The big decision: what kind of failure is this?

Before writing any error code, answer one question: **is this failure
expected, or is it a bug?**

```
                 Can this happen in correct operation?
                 (bad input, missing file, network, permissions)
                    │                                  │
                   YES                                 NO
                    │                                  │
        Is "nothing there" the           It's a bug or a broken invariant.
        only way it can fail?            panic! / assert! / unreachable! /
            │             │              expect("why this can't fail")
           YES            NO                           │
            │             │               Is it at a boundary where one
       Option<T>     Result<T, E>         failure mustn't kill everything
                          │               (plugin, request, task)?
            Who handles the error?                     │
           ┌──────────────┴───────────────┐   catch_unwind / tokio task
     code that must react              a human reads it   boundary, converted to an
     (library / module API)           (binary, top level)  error there
           │                                │
     typed enum (thiserror)          anyhow::Error + .context()
     one variant per reaction        report the whole chain
```

Two consequences:

- **Absence is not an error.** `HashMap::get` returns `Option`, not
  `Result`: a missing key is a normal answer. Turn it into an error only at
  the point where absence *becomes* a problem (`.ok_or_else(..)`).
- **Library vs application is about *who is downstream*.** A library can't
  know whether its caller will retry, fall back, or give up, so it must
  return something matchable. An application's `main` is the end of the
  line: a human reads the error there, so context matters more than the type.

---

## Combinators: small tools for Option and Result

A `match` for every `Option` gets noisy fast. The combinators transform
the value *inside* without unpacking it. The ones worth knowing by heart:

| You have | You want | Use |
|---|---|---|
| `Option<T>` / `Result<T,E>` | change the success value | `map(f)` |
| same | chain a step that can itself fail | `and_then(f)` (`f` returns `Option`/`Result`) |
| `Result<T,E>` | change the error | `map_err(f)` |
| `Option<T>` | `Result<T,E>` | `ok_or(e)` / **`ok_or_else(\|\| e)`** |
| `Result<T,E>` | `Option<T>` (drop the error) | `ok()` |
| either | a value with a default | `unwrap_or(v)` / `unwrap_or_else(f)` / `unwrap_or_default()` |
| either | "if present do A else B" into one value | `map_or(default, f)` / `map_or_else(g, f)` |
| `Option<Result<T,E>>` | `Result<Option<T>,E>` | `transpose()` |
| `Option<Option<T>>` | `Option<T>` | `flatten()` |
| iterator of `T` | first `f(x)` that is `Some` | `find_map(f)` |

**Eager vs lazy.** `ok_or(expensive())` calls `expensive()` *every time*,
even on success, because arguments are evaluated before the call.
`ok_or_else(|| expensive())` calls it only on `None`. The same goes for
`unwrap_or` vs `unwrap_or_else` and `map_or` vs `map_or_else`. Clippy's
`or_fun_call` lint catches many of these.

```rust
// databroker/src/grpc/kuksa_val_v2/val.rs:673 (simplified)
.ok_or(tonic::Status::not_found(format!("Invalid path in signal_id provided {path}")))?;
//     ^ builds the Status and formats the String even when the lookup succeeded.
.ok_or_else(|| tonic::Status::not_found(format!("... {path}")))?; // only on failure
```

**`?` works on `Option` too**, in a function that returns `Option`. It does
*not* mix: `?` on an `Option` inside a function returning `Result` is
error E0277. Convert explicitly with `ok_or_else` so you're forced to
say *what* error "missing" should be.

---

## Error chains: source() and context

`std::error::Error` has one method that matters beyond `Display`:

```rust
fn source(&self) -> Option<&(dyn Error + 'static)>
```

It returns the *lower-level* error that caused this one. Errors therefore form
a linked list, from what the program was trying to do down to what actually
broke:

```
LoadError::Parse  "failed to load VSS overlay \"overlay.json\""
   └─ source → ParseError::BadValue  "bad value on line 3"
         └─ source → ParseIntError   "invalid digit found in string"
```

Rules that make chains work:

1. **Each layer's message describes only its own level.** Don't write
   `"failed to load overlay: {cause}"`, because reporters print the chain and
   you'll see the cause twice.
2. **Keep the cause as a value, not as text.** `cause.to_string()` destroys
   information: the caller can no longer `downcast_ref` to it, ask for its
   `kind()`, or read its fields (a serde error's line and column).
3. **Reports walk the chain.** Printing `{}` shows only the top message.
   A reporter loops over `source()` (exercise e02), and `anyhow` does it for
   you with `{:#}` (one line, `: `-separated) and `{:?}` (multi-line with
   "Caused by:").

thiserror wires `source()` up for you when a field:

- is named `source`, or
- is marked `#[source]`, or
- is marked `#[from]` (which also generates `impl From<Cause> for YourError`, so `?` converts).

You can read thiserror's detection logic in
`~/.cargo/registry/src/index.crates.io-*/thiserror-impl-1.0.69/src/prop.rs:104`
(the field-name check).

---

## Designing a library error type

A walkthrough using exercise e03's `SignalStore` as the example.

**Step 1: list what can go wrong, from the caller's point of view.** Not
"what can go wrong inside my code" but "what might the caller want to do
differently": the path doesn't exist, the type is wrong, the value is
out of range, or the input text doesn't parse. Four reactions, four variants.

**Step 2: put data in fields, not in pre-formatted strings.**

```rust
// Bad: the caller can't get the numbers back out.
OutOfRange(String)                     // "value 251 is outside 0..=250"

// Good: matchable, testable, and Display can still format it.
OutOfRange { path: String, value: i64, min: i64, max: i64 }
```

**Step 3: keep the underlying cause** as a `#[source]` field (see above).

**Step 4: add a coarse `kind()`** if callers usually only care about the
category. `std::io::Error` does this: millions of callers match on
`e.kind() == ErrorKind::NotFound` and never look at the details.

**Step 5: future-proof it.** `#[non_exhaustive]` on a public enum forces
downstream `match`es to include a `_ =>` arm, so adding a variant later is
not a breaking change (chapter 33 covers semver).

**Step 6: make it a good citizen.**

```rust
fn assert_good_error_type<E: std::error::Error + Send + Sync + 'static>() {}
```

It should be `Send + Sync` so it can cross threads and be boxed into
`anyhow::Error` or `Box<dyn Error + Send + Sync>`, and `'static` so it owns
its data (no borrowed `&str` fields). Every error in a public API should
pass this check.

**Step 7: messages follow the API Guidelines (C-GOOD-ERR):** lowercase, no
trailing punctuation, concise, and describing *what went wrong* ("signal not
found: Vehicle.Speed"), not what the user should do.

**How many error types?** Usually one per module or crate boundary, not one
per function (too many to convert between) and not one for the whole
program (a mega-enum where every function "can" return every variant tells
the caller nothing). A good heuristic: if two functions' callers would
handle failures the same way, the functions can share an error type.

---

## Application errors with anyhow

At the top of a program you want to *explain*, not *classify*.
`anyhow::Error` is a boxed `dyn Error + Send + Sync + 'static`, plus a
backtrace (when enabled), plus a stack of context messages.

```rust
use anyhow::{bail, ensure, Context, Result};   // Result<T> = Result<T, anyhow::Error>

fn load(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config {}", path.display()))?;
    let cfg: Config = toml_like_parse(&text).context("parsing config")?;
    ensure!(cfg.port != 0, "port must not be 0");
    if cfg.name.is_empty() { bail!("name is empty"); }
    Ok(cfg)
}
```

| Tool | Meaning |
|---|---|
| `.context("msg")` / `.with_context(\|\| ..)` | wrap the error; the original becomes the source. Also works on `Option` (`None` becomes an error with that message). |
| `bail!(..)` | `return Err(anyhow!(..))` |
| `ensure!(cond, ..)` | `if !cond { bail!(..) }` |
| `err.chain()` | iterate the error and all its sources |
| `format!("{err:#}")` | whole chain on one line: `outer: middle: root` |
| `err.downcast_ref::<T>()` | find a specific typed error *anywhere* in the chain, through the context layers |
| `fn main() -> anyhow::Result<()>` | on error, prints the chain and exits with code 1 |

The `Context` trait is defined at
`~/.cargo/registry/src/index.crates.io-*/anyhow-1.0.104/src/lib.rs:616`, and
`downcast_ref` at `.../anyhow-1.0.104/src/error.rs:536`. Read them: they're
short and show how context layers wrap without losing the original.

**Don't use anyhow in a library's public API.** Callers lose the ability to
match on your errors without guessing which types to downcast to.

---

## Who is this error for? Categories and recovery

Every error has an *audience* and a *recovery strategy*:

| Category | Audience | Message | Recovery | Example (databroker) |
|---|---|---|---|---|
| User-facing | the client / end user | clear and actionable; safe to show | client fixes the request | `NotFound`, `WrongType`, `OutOfBoundsMinMax` |
| Authorization | the client | clear, but reveals nothing about *why* beyond "denied" | get a new token | `PermissionDenied`, `PermissionExpired` |
| Transient | automation | brief | retry with backoff, or fall back | provider timeout, `ProviderNotAvailable` |
| Internal | developers / operators | **generic outside, full detail in logs** | investigate, alert | lock poisoned, invariant broken |
| Permanent config | operators | detailed | fix config, restart | bad VSS file at startup |

Two rules follow:

1. **Never leak internals to clients.** An internal error's `Display`
   should be generic ("internal error"). The detail lives in its `source()`,
   which you log (exercise e05). Database connection strings, file paths,
   SQL fragments and stack traces in client-facing messages are a security
   problem as well as bad UX.
2. **Recovery must match the category.** Retry and fallback are only for
   transient errors. Retrying a `PermissionDenied` wastes effort, and
   *falling back* on one is a security bug: returning cached data to a
   client who isn't allowed to read it.

```
      error happens
           │
   is it transient? ── no ──► return it (map to the right status code)
           │
          yes
           │
   is there a safe fallback? ── yes ──► use it (log that you degraded)
           │
           no
           │
   retry with capped backoff, bounded attempts ──► still failing? return it
           │
   is the dependency failing repeatedly? ──► circuit breaker: stop calling for a while
```

The databroker's `get_values_broker` (`databroker/src/broker.rs:2200`)
follows this pattern: provider failures (transient) fall back to the last
value in the database (`broker.rs:2294`), while permission failures are
checked *before* any provider is asked (`broker.rs:2233`) and returned as-is.

Status-code mapping is where categories become visible to clients. See
`to_status_with_code` at `databroker/src/grpc/kuksa_val_v2/conversions.rs:356`:
each `UpdateError` maps to a precise gRPC code (`NotFound`,
`InvalidArgument`, `PermissionDenied`, `Unauthenticated`), which lets clients
branch on the code instead of parsing text.

---

## Retry, backoff and circuit breakers

**Retry** is right only when the error is transient *and* the operation is
safe to repeat (idempotent: setting a value, not "increment" or "send
payment"). A correct retry loop has:

- a **maximum number of attempts**, because infinite retries turn an outage
  into a self-inflicted denial of service;
- **exponential backoff** (100 ms, 200 ms, 400 ms, …) so a struggling
  dependency gets breathing room;
- a **cap** on the delay, so the tenth retry doesn't wait an hour;
- *(in production)* **jitter**: a random fraction added to each delay so
  a thousand clients that failed together don't retry in lockstep;
- **no sleep after the final attempt**;
- **overflow-safe arithmetic**: `factor.pow(attempt)` overflows quickly, so use
  `checked_pow` / `checked_mul` / `saturating_*` and `min(cap)`.

A **circuit breaker** protects the *caller* from a dependency that keeps
failing:

```
        success                   failure count reaches threshold
   ┌──────────────┐        ┌─────────────────────────────────────┐
   ▼              │        │                                     ▼
 CLOSED ──────────┴────────┘                                   OPEN
   ▲         calls flow; count failures           reject instantly, don't call
   │                                                             │
   │ trial succeeds                                  cooldown elapsed
   │                                                             ▼
   └──────────────────────────────── HALF-OPEN ◄─────────────────┘
                                 let ONE trial call through;
                                 trial fails → back to OPEN
```

**Inject time.** Exercise e06 passes `now` in and takes a `Sleeper` trait
instead of calling `std::thread::sleep`. That makes the logic:

- **deterministic to test**: tests record the requested delays instead of waiting;
- **runtime-agnostic**: the same logic works with `std::thread::sleep`, with
  `tokio::time::sleep` (in chapter 20 you'll see `#[tokio::test(start_paused = true)]`,
  which fakes time for a whole async runtime), or with a simulated clock.

The databroker bounds provider calls in time instead of retrying them: see
`PROVIDER_UPDATE_FILTER_TIMEOUT` at `databroker/src/broker.rs:39` and the
single deadline shared by every phase of a provider request in
`databroker/src/grpc/kuksa_val_v2/val.rs:158`. That's the right call on the
*server* side: the server shouldn't make a client wait through retries of
someone else's provider. Retry-with-backoff belongs in *clients* (a provider
reconnecting after the broker restarts). Production crates: `backoff`,
`tokio-retry`, `tower::retry`, and tower's rate-limit and load-shed layers.

---

## Panics, unwinding and poisoning

**What a panic does.** By default (`panic = "unwind"`) a panic walks back up
the stack, running `Drop` for every live value (unwinding), until it reaches
the top of the thread, which then dies. With `panic = "abort"` in the
`[profile]` section of `Cargo.toml`, the process aborts immediately: smaller
binaries and no unwinding cost, but nothing can catch the panic.

**When to panic** (the full rule):

- a *bug*: an invariant your own code guarantees was broken (`unreachable!()`,
  `expect("path is always set for subscription updates")`);
- *startup* preconditions without which the program can't run at all
  (`databroker/src/main.rs:42`:
  `signal(SignalKind::interrupt()).expect("failed to setup SIGINT signal handler")`);
- tests.

**Never** for anything a client, a file or the network can cause.

**`catch_unwind`** (`std::panic::catch_unwind`) stops an unwinding panic at a
boundary and turns it into a `Result<T, Box<dyn Any + Send>>`. The payload
is usually a `&'static str` (`panic!("literal")`) or a `String`
(`panic!("{x}")`), so downcast to both (exercise e07). Use it at isolation
boundaries: plugin hosts, FFI edges (panics must never unwind into C;
chapter 26), and thread pools. It's **not** a try/catch for normal control
flow. The closure must be `UnwindSafe`, a marker that warns you shared state
might be half-modified. `AssertUnwindSafe(..)` is how you say you've thought
about that.

Tokio does the same thing at every task boundary: a panicking task doesn't
bring down the runtime. Its `JoinHandle` resolves to `Err(JoinError)` with
`is_panic() == true` (chapter 24 exercise e06).

**Poisoning.** If a thread panics while holding a `std::sync::Mutex`, the
mutex is marked *poisoned*: later `lock()` calls return
`Err(PoisonError<Guard>)`. The data is still there, but it might be
half-updated. Your options are:

- `lock().unwrap()`, which propagates the panic (a reasonable default:
  "if a lock holder crashed, we can't trust the data");
- `lock().unwrap_or_else(PoisonError::into_inner)`, when you've reasoned
  that the data is still valid (a counter, a cache you can rebuild);
- `Mutex::clear_poison()` after repairing the state.

`tokio::sync::Mutex` and `parking_lot::Mutex` don't poison. That's one
reason you'll see `.lock().await` with no `unwrap` in async code.

**`#[track_caller]`** makes `Location::caller()` (and panic messages from
`unwrap`/`expect` inside the function) report the *caller's* line. Put it
on helpers that validate their arguments, so the panic points at the
bad call, not the helper.

**`#[must_use]`** on a function or type makes ignoring the result a
warning. `Result` is `#[must_use]` already; that's why `let _ = ...` is
the explicit way to say "I'm deliberately ignoring this".

---

## Errors across boundaries: threads, trait objects, iterators

**Type erasure.** When a function can fail in many unrelated ways and callers
won't match on the specifics, return a boxed trait object:

```rust
type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
```

| Bound | Why |
|---|---|
| `Send` | the error can be *moved* to another thread (returned from `thread::spawn`, sent over a channel, held across `.await` in a spawned task) |
| `Sync` | `&BoxError` can be *shared* between threads (needed to convert into `anyhow::Error`, among other things) |
| `'static` | the error owns its data and borrows nothing; required by `downcast_ref` and by storing it anywhere long-lived |

`Box<dyn Error>` without `Send + Sync` is a common mistake: it compiles
until the day the error has to cross a thread, then fails with E0277
(exercise e08). The databroker's `main.rs:232`
(`Result<(), Box<dyn std::error::Error>>`) gets away with it because `main`
is the end of the line, while `grpc/server.rs:203` correctly requires
`Into<Box<dyn std::error::Error + Send + Sync>>`.

Get the concrete type back with `err.downcast_ref::<ParseIntError>()`.
That's a runtime check, so reach for it only at the edges where you
really need to branch.

**Threads.** `JoinHandle::join()` returns `Result<T, Box<dyn Any + Send>>`,
where the outer `Err` means "the thread panicked". So a worker that returns
`Result<U, E>` gives you a nested `Result<Result<U, E>, panic payload>`.
Handle both layers.

**Iterators.** Two strategies:

- **fail fast**: `iter.map(fallible).collect::<Result<Vec<_>, _>>()` stops at
  the first `Err` and returns it;
- **collect everything**: `partition(Result::is_ok)`, or a loop that pushes
  errors into a `Vec`, reports every failure at once. The databroker's
  `update_entries` (`databroker/src/broker.rs:1529`) does this: a batch
  update returns `Err(Vec<(i32, UpdateError)>)` so one bad signal doesn't
  hide the others.

Choose based on the caller: validating a config file? Show every problem at
once. Applying a transaction? Stop at the first one.

---

## Errors in async code (preview of chapters 20–24)

Everything above applies unchanged to async code; `?` works in `async fn`
exactly as it does in sync code. Async adds three new failure shapes, and
each is covered later:

| Shape | Type | Where |
|---|---|---|
| a spawned task panicked or was cancelled | `tokio::task::JoinError` (`is_panic()`, `is_cancelled()`) | chapter 20 e02, chapter 24 e06 |
| an operation took too long | `tokio::time::timeout(d, fut)` → `Result<T, Elapsed>`, often nested as `Result<Result<T, E>, Elapsed>` | chapter 20 e03, chapter 21 e07 |
| a channel's other end is gone | `SendError<T>` (gives your value back), `RecvError`, `broadcast::error::RecvError::Lagged(n)` | chapter 21 |

The databroker matches on the nested timeout shape directly:
`Ok(Ok(())) / Ok(Err(..)) / Err(_elapsed)` in `update_filter_to_providers`
(`databroker/src/broker.rs:2477`).

One async-specific rule: an error type held across an `.await` inside a
`tokio::spawn`ed task must be `Send`, which is another reason for the
`Send + Sync` bounds above.

---

## Mental model

| Idea | Analogy |
|---|---|
| `Option` | A box that might be empty. Empty is a normal answer, not a failure. |
| `Result` with a typed error | A form with a checkbox for each reason it was rejected, so you know exactly what to fix. |
| Error chain (`source()`) | A stack of sticky notes: "couldn't start the car" → "battery dead" → "lights left on". |
| `anyhow::Context` | Adding another sticky note on top as the error travels up. |
| Transient vs permanent | "The shop is closed, try tomorrow" vs "we don't sell that". Only one is worth coming back for. |
| Circuit breaker | A fuse: after too many faults it trips, and later you try flipping it back. |
| Panic | Pulling the fire alarm: correct for real fires (bugs), wrong for burnt toast (bad input). |
| Poisoned mutex | A room sealed because someone fainted inside mid-task. You may enter, but check what they left half-done. |

**Guarantees the compiler gives you:** you can't forget that a function can
fail (`Result` must be used: `#[must_use]`); you can't use the success value
without handling the error somehow (no unchecked exceptions); and
`Send`/`Sync` bounds stop non-thread-safe errors from crossing threads.

---

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "`unwrap()` is fine, it never fails here." | Then write `expect("why it can't fail")`, or better, restructure so the type proves it. If it *can* fail on input, it's a bug waiting for a client to find it. |
| "`String` errors are simpler." | They're simpler to *create* and useless to *handle*: callers end up parsing text. Use an enum. |
| "Put the cause in the message: `format!(\"failed: {e}\")`." | Keep it as `source()` and let the reporter print the chain. Formatting it in duplicates it and destroys the type. |
| "`Box<dyn Error>` works everywhere." | Not across threads: you need `+ Send + Sync` (E0277). |
| "Retry makes things more reliable." | Only for transient errors, with bounded attempts and backoff. Otherwise it amplifies outages. |
| "`catch_unwind` is try/catch." | It's for isolation boundaries. It doesn't catch aborts, and panics aren't Rust's error mechanism. |
| "Use anyhow everywhere." | Libraries use typed errors; applications use anyhow. |
| "`?` works on anything." | `?` needs the function's return type to accept the error (via `From`). Mixing `Option` and `Result` needs an explicit conversion (E0277). |

---

## Smells and idioms

| Smell | Why it's bad | Idiomatic fix |
|---|---|---|
| `Result<T, ()>` | the caller learns *that* it failed but not why; can't be logged or mapped usefully | a small enum, even with one variant (`#[derive(Debug, thiserror::Error)] #[error("provider timed out")] struct Timeout;`) |
| `Result<T, (ErrorEnum, String)>` | two sources of truth; the String isn't matchable, and messages drift between call sites | put the data in the enum's fields and derive `Display` |
| `.map_err(\|e\| e.to_string())` | throws away the cause's type and fields | `#[source]` field / `#[from]` |
| `let _ = fallible();` without comment | a silently swallowed error | handle it, log it, or add a comment explaining why ignoring is correct |
| `todo!()` / `unimplemented!()` reachable from input | a client-triggerable panic | return an error (`TryFrom`), and `unreachable!` only when *truly* impossible |
| `ok_or(format!(..))` | allocates on the success path | `ok_or_else(\|\| format!(..))` |
| error messages like `"Error: Failed to X."` | inconsistent in a chain | lowercase, no punctuation, no "error:" prefix |
| retrying every error | wastes time on permanent errors, and falls back on security errors | classify with `is_retryable()` |

---

## In the databroker and in the wild

**The databroker's error design, read critically.** It's a working
production system; these are trade-offs to recognise, not reasons to sneer.

1. **Typed per-operation enums: good.** `UpdateError`
   (`databroker/src/broker.rs:64`) has one variant per reaction, derives
   `PartialEq` (easy to test), and maps cleanly to gRPC codes at
   `databroker/src/grpc/kuksa_val_v2/conversions.rs:356`. It doesn't
   implement `std::error::Error` or `Display`, so it can't join a chain or
   be boxed. That's fine for an internal type, but it limits reuse.

2. **`(Error, String)` tuples.** `ActuationProvider::actuate` returns
   `Result<(), (ActuationError, String)>` (`broker.rs:189-193`), and so do
   `RegisterSignalError` users. The String carries the human message, the enum
   the category, and `to_tonic_status(message)` (`conversions.rs:524`)
   combines them. It works, but nothing ties a variant to its message, so
   the same failure can be phrased differently at each call site, and tests
   end up comparing strings. The idiomatic version is a thiserror enum with
   fields (exercise e03).

3. **`Result<_, ()>`.** `SignalProvider::get_signals_values_from_provider`
   returns `Result<GetValuesProviderResponse, ()>` (`broker.rs:207`). The
   caller only needs "fall back or not", so `()` was *enough*, but timeouts,
   a closed stream, an empty response and mismatched request ids all
   collapse into one value, and nothing can log *which* happened. A small
   `ProviderError { Timeout, Disconnected, EmptyResponse }` would cost
   little and make incidents debuggable. Similarly `NotificationError {}`
   (`broker.rs:248`) and the `Err(_) => true` at `broker.rs:1575` lose why
   notification failed.

4. **A flattened cause.** `vss::Error::ParseError(String)`
   (`databroker/src/vss.rs:155`) is built with `from.to_string()`
   (`vss.rs:167`), so serde's line and column survive only as text.

5. **A reachable `todo!()`.** `impl From<proto::Value> for broker::DataValue`
   (`databroker/src/grpc/kuksa_val_v2/conversions.rs:394`) ends with
   `None => todo!()` at `conversions.rs:431`. `actuate` checks that the
   *outer* `value` is present (`databroker/src/grpc/kuksa_val_v2/val.rs:661`)
   but not that its `typed_value` is set, then calls `DataValue::from(value)`
   (`val.rs:677`). A client sending `Value { typed_value: None }` therefore
   triggers a panic. Tokio isolates it to that task (see "Panics"), so the
   server survives, but the client gets a dropped stream instead of
   `invalid_argument`. The fix is the chapter 09 tool: **`From` is for
   conversions that can't fail; use `TryFrom` when they can.**

   ```rust
   impl TryFrom<proto::Value> for broker::DataValue {
       type Error = tonic::Status;
       fn try_from(value: proto::Value) -> Result<Self, Self::Error> {
           match value.typed_value {
               Some(proto::value::TypedValue::Bool(b)) => Ok(DataValue::Bool(b)),
               // ...
               None => Err(tonic::Status::invalid_argument("Value has no typed_value")),
           }
       }
   }
   // call site:
   let value = DataValue::try_from(value)?;
   ```

   This is a good first contribution: write the failing test first (chapter 32).

6. **`unwrap`/`expect` on invariants.** `val.rs:344`
   (`proto_datapoint_opt.unwrap()`) is safe today because every arm of
   `impl From<broker::Datapoint> for Option<proto::Datapoint>`
   (`conversions.rs:48`) returns `Some`, but the types don't prove it. If the
   impl ever returned `None`, a request would panic. `val.rs:1356` at least uses
   `expect` with a message. Returning the non-optional type (or
   `filter_map`-ing) would make the invariant impossible to break.

7. **Deliberately ignored results.** `let _ = self.propagate_new_filter_to_provider(..)`
   (`broker.rs:1654`) ignores a provider notification failure during
   subscribe. That's reasonable (the subscription itself succeeded), but a
   comment or a `debug!` log would tell readers it's intentional.

8. **Startup `expect`: correct.** `main.rs:42` panics if the signal handler
   can't be installed. There's nothing sensible to do without it, and it's
   at startup, before any client depends on the process.

**In the wild** (sources are on disk under
`~/.cargo/registry/src/index.crates.io-*/`; exact versions from `learn-rust/Cargo.lock`):

- `anyhow-1.0.104/src/lib.rs:616`: the `Context` trait, sealed so only
  `Result` and `Option` get it.
- `anyhow-1.0.104/src/error.rs:536`: `downcast_ref`, which looks through
  context layers.
- `thiserror-impl-1.0.69/src/prop.rs:104`: how `#[derive(Error)]` decides
  which field is the `source`.
- `std::io::Error` + `ErrorKind`: the canonical "rich error with a coarse
  kind" design. With the `rust-src` rustup component installed, the source
  is under `$(rustc --print sysroot)/lib/rustlib/src/rust/library/std/src/io/error.rs`.
- `std::panic::catch_unwind` and `PoisonError::into_inner`: the same
  directory, `std/src/panic.rs` and `std/src/sync/poison.rs`.

---

## Exercises

| Exercise | Goal |
|---|---|
| `c17_e01_combinators` | Write each fallible transformation as one combinator chain; `ok_or` vs `ok_or_else` |
| `c17_e02_error_source_chains` | Wire up `source()` with thiserror; render a "caused by" report; find the root cause |
| `c17_e03_library_error_design` | Design a public error type: structured variants, `kind()`, `#[non_exhaustive]`, `Send + Sync + 'static` |
| `c17_e04_application_errors_anyhow` | `bail!`, `ensure!`, `with_context`, `{:#}`, and downcasting through context |
| `c17_e05_error_categories_and_recovery` | Classify errors, map them to status codes without leaking internals, fall back only on transient errors |
| `c17_e06_retry_backoff_circuit_breaker` | Capped exponential backoff, bounded retries, and a circuit breaker, with injected time |
| `c17_e07_panics_unwinding_poisoning` | `catch_unwind` at a plugin boundary, panic payloads, poisoned-mutex recovery, `#[track_caller]` |
| `c17_e08_errors_across_boundaries` | `Send + Sync` boxed errors across threads, thread panics as errors, fail-fast vs collect-all |

---

## Further reading

- *The Rust Programming Language*, ch. 9 "Error Handling":
  <https://doc.rust-lang.org/book/ch09-00-error-handling.html>
- Rust API Guidelines, C-GOOD-ERR ("Error types are meaningful and well-behaved"):
  <https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err>
- `std::error::Error` docs, especially `source()` and the provided methods:
  <https://doc.rust-lang.org/std/error/trait.Error.html>
- anyhow: <https://docs.rs/anyhow>. thiserror: <https://docs.rs/thiserror>.
- The Rustonomicon, "Unwinding" and "Poisoning":
  <https://doc.rust-lang.org/nomicon/unwinding.html>
- Andrew Gallant, "Error Handling in Rust" (a long-form classic):
  <https://blog.burntsushi.net/rust-error-handling/>
- AWS Builders' Library, "Timeouts, retries, and backoff with jitter":
  <https://aws.amazon.com/builders-library/timeouts-retries-and-backoff-with-jitter/>

---

## Check yourself

1. Why does `ok_or(format!(...))` allocate even on success, and what fixes it?
2. When should a function return `Option` rather than `Result`?
3. Why shouldn't a wrapper error's message include `{cause}`?
4. What does `#[non_exhaustive]` protect you from?
5. Why is `Box<dyn Error>` not enough to return from a thread, and which bounds fix it?
6. Why must a fallback never apply to a `PermissionDenied`?
7. Name three properties of a correct retry loop.
8. What happens when a thread panics while holding a `std::sync::Mutex`, and how can a later thread still read the data?
9. Why is `impl From<proto::Value> for DataValue` the wrong trait, and what should it be?

<details><summary>Answers</summary>

1. Arguments are evaluated before the call, so the `String` is built every
   time. `ok_or_else(|| format!(..))` builds it only on `None`.
2. When "nothing there" is a normal answer with only one way to "fail"
   (lookups, searches). Use `Result` when the caller needs to know *why*.
3. Reporters print the whole chain from `source()`. Including the cause in
   the message duplicates it, and formatting it into a string loses its type.
4. Downstream `match`es must include a wildcard arm, so you can add variants
   later without a breaking change.
5. Values returned from `thread::spawn` must be `Send`, and a `dyn Error`
   trait object says nothing about thread safety. Use
   `Box<dyn Error + Send + Sync + 'static>`.
6. Because it would hand data to a client that isn't allowed to read it.
   Fallback is for *availability* problems, never *authorization* problems.
7. Retry only transient errors; bound the number of attempts; use capped
   exponential backoff (ideally with jitter). Also: don't sleep after the
   last attempt, and use overflow-safe arithmetic.
8. The mutex becomes poisoned, so `lock()` returns `Err(PoisonError)`.
   `unwrap_or_else(PoisonError::into_inner)` gets the guard anyway, if the
   data is still valid.
9. `From` promises the conversion can't fail, but a `Value` without a
   `typed_value` has no `DataValue`, so the impl panics (`todo!()`). It should
   be `TryFrom`, returning an `invalid_argument` status.
</details>
