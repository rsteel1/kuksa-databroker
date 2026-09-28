# 07 — Error handling

**Why this matters for the databroker.** A broker sits between untrusted
clients, flaky providers and a car's worth of signals. Paths don't exist,
values have the wrong type, tokens expire, providers hang up. None of that may
crash the process. Rust makes every fallible operation *say so in its type*,
and the databroker turns those typed errors into gRPC status codes at the edge.
Reading `broker.rs` is mostly reading `Result`s.

## Result: errors are values

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

There are no exceptions in Rust. A function that can fail returns a
`Result`, and the caller has to look inside to get the value:

```rust
match "120".parse::<u16>() {
    Ok(speed) => println!("speed {speed}"),
    Err(e) => println!("bad input: {e}"),
}
```

`Result` is marked `#[must_use]`: silently dropping one produces a warning.
If you really want to ignore an error, say so: `let _ = sender.send(msg);`
(the databroker does exactly this in `send_none_to_receiver`).

`Option<T>` is the sibling for *absence* that isn't an error: `HashMap::get`
returns `Option`, not `Result`, because a missing key is normal.

Useful methods (all avoid writing a `match`):

| Method | Does |
|---|---|
| `map(f)` | transform the `Ok` value |
| `map_err(f)` | transform the error |
| `and_then(f)` | chain another fallible step |
| `ok()` | `Result<T,E>` → `Option<T>` (drops the error) |
| `ok_or(e)` / `ok_or_else(f)` | `Option<T>` → `Result<T,E>` |
| `unwrap_or(v)` / `unwrap_or_default()` | fall back to a value |
| `is_ok()` / `is_err()` | just check |

## The ? operator

```rust
fn parse_assignment(input: &str) -> Result<(String, i32), String> {
    let (path, value) = input.split_once('=').ok_or("missing '='")?;
    let number = value.trim().parse::<i32>().map_err(|_| "bad value")?;
    Ok((path.trim().to_string(), number))
}
```

`expr?` is shorthand for:

```rust
match expr {
    Ok(v) => v,
    Err(e) => return Err(From::from(e)),
}
```

Two consequences:

1. `?` only compiles inside a function that returns `Result` (or `Option`,
   for `?` on options). Using it in a `fn -> ()` gives **E0277**. That's why
   `main` in the databroker returns `Result<(), Box<dyn std::error::Error>>`.
2. The error is passed through `From::from` — which is how one function can
   use `?` on several different error types (next sections).

`Option`s turn into `Result`s with `ok_or`: `.ok_or(MyError::Missing)?`.

## Designing an error type

`String` errors are fine for scripts, but callers can't `match` on text. A
library defines an **enum** of the things that can go wrong:

```rust
#[derive(Debug, PartialEq)]
pub enum UpdateError {
    NotFound,
    WrongType,
    OutOfBoundsMinMax,
    PermissionDenied,
    // ...
}
```

That's (a trimmed) `databroker/src/broker.rs:64`. The gRPC layer then maps
each variant to a status code (`NotFound` → `NOT_FOUND`, `PermissionDenied` →
`PERMISSION_DENIED`...). Variants can carry data:
`OutOfBounds { value: i64, min: i64, max: i64 }`.

To be a well-behaved error, a type implements:

- `Debug` (derive it) — for `{:?}` and `unwrap` messages,
- `Display` — the human-readable message (`to_string()` comes from this),
- `std::error::Error` — the marker trait that lets it be boxed as
  `Box<dyn Error>` and chained via `source()`.

```rust
impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::NotFound => write!(f, "signal not found"),
            // ...
        }
    }
}
impl std::error::Error for UpdateError {}
```

Granularity is a design decision: one variant per *different reaction the
caller might have*, not one per line of code that can fail.

## How ? converts errors: From

If `vss.rs` parses JSON with `serde_json::from_reader(reader)?` in a function
returning `Result<_, vss::Error>`, the `?` needs to turn a
`serde_json::Error` into a `vss::Error`. It does so by calling
`vss::Error::from(json_err)`, which exists because of:

```rust
impl From<serde_json::Error> for Error {
    fn from(from: serde_json::Error) -> Self {
        Error::ParseError(from.to_string())
    }
}
```

(`databroker/src/vss.rs:167`.) Without that impl you get
**E0277: `?` couldn't convert the error to `Error`**. The fix is either a
`From` impl (conversion happens everywhere automatically) or an explicit
`.map_err(|e| ...)` at the call site (when the conversion depends on context).

`Box<dyn std::error::Error>` is the catch-all: *any* type implementing
`Error` converts into it via `?`. (`dyn Error` is a *trait object*, meaning "some
type that implements `Error`". Chapter 09 explains trait objects.) Great for `main` and tests; poor for a
library API because callers lose the ability to match on variants.

## thiserror: the same, derived

Writing `Display` + `Error` + `From` by hand is boilerplate. The `thiserror`
crate derives it:

```rust
#[derive(Debug, thiserror::Error)]
enum ProviderError {
    #[error("signal already provided: {id}")]
    AlreadyProvided { id: i32 },
    #[error("invalid signal id `{1}`")]
    InvalidId(#[source] std::num::ParseIntError, String),
    #[error("provider stream closed")]
    Closed(#[from] std::io::Error),
}
```

- `#[error("...")]` generates `Display`; fields are interpolated by name or
  position.
- `#[source]` makes `.source()` return the inner error (for error chains).
- `#[from]` does that *and* generates `impl From<io::Error>`, so `?` works.

The generated code is exactly what you'd write by hand — there's no runtime
cost. The databroker uses it in `databroker/src/authorization/mod.rs:26`.

## Panic, expect, or Result?

`panic!` unwinds the current thread and (in a server, usually) kills it. Use
it for **bugs** — states your code guarantees can't happen — and for
**startup preconditions** without which the program can't sensibly run.

| Situation | Use |
|---|---|
| Expected failure the caller can handle (bad input, missing signal, I/O) | `Result` |
| Absence is normal (lookup) | `Option` |
| Broken invariant = a bug in *our* code | `panic!`, `assert!`, `unreachable!`, `expect` |
| Startup can't continue (can't install signal handler, logger) | `expect("why")` |
| Tests, prototypes | `unwrap` is fine |

`unwrap()` panics with a generic message; `expect("...")` panics with yours.
Always prefer `expect` in non-test code, and phrase the message as the
invariant: `expect("KUKSA_PORT must be a valid port")`.

**Library vs application.** Libraries expose typed errors (`thiserror`) so
callers can react. Applications usually just need to *report* failures
well: `anyhow::Result<T>` accepts any error, and `.context("what we were
doing")` wraps it in a readable chain:

```rust
let port: u16 = text.parse().with_context(|| format!("invalid port `{text}`"))?;
// "{:#}" prints: invalid port `http`: invalid digit found in string
```

`anyhow::bail!("msg")` is `return Err(anyhow!("msg"))`. The databroker's
`build.rs` uses `anyhow`.

## Mental model

| Concept | Analogy |
|---|---|
| `Result<T, E>` | a parcel that is either the goods or a signed delivery-failure note — you must open it |
| `?` | "if it's a failure note, forward it to my caller (translated), otherwise give me the goods" |
| `From` impl | a translation table between failure-note formats |
| `panic!` | pulling the fire alarm: correct for a real fire, terrible as a way to say "item out of stock" |
| `expect("...")` | pulling the fire alarm but leaving a note explaining which fire |
| `anyhow::Context` | stapling "while doing X" onto the note on the way up |

**Guarantee the compiler gives you:** you cannot use a value from a fallible
operation without acknowledging that it might have failed. Errors can't be
forgotten silently (`#[must_use]`), and error conversions are checked.

## Common misconceptions

| Wrong model | Correct model | Error |
|---|---|---|
| "`?` works anywhere" | Only in functions returning `Result`/`Option` | E0277 |
| "`?` converts any error to any error" | Only if `From<Their> for Mine` exists | E0277 |
| "Errors are exceptions; they propagate on their own" | Nothing propagates unless you write `?` or `return Err` | — |
| "`unwrap` is how you get the value" | `unwrap` is "crash if wrong"; use `?`, `match`, `if let`, combinators | runtime panic |
| "`Box<dyn Error>` is the best error type" | Great for `main`/tests; libraries should expose matchable enums | — |
| "`Display` is derived" | Only `Debug` is derivable from std; `Display` you write (or thiserror) | E0277/E0599 |

## Smells and idioms

- **`unwrap()` in library/server code.** A panic in a spawned task kills that
  task (e.g. a client's stream) — at best. Prefer `?` or `expect` with the invariant.
- **Stringly-typed errors** (`Result<T, String>`) in APIs: callers can't match.
- **Swallowing errors**: `if let Ok(x) = ... {}` with no `else`, or
  `let _ =` on something that really matters. Log at least (`debug!`/`warn!`).
- **Losing the cause**: `.map_err(|_| MyError::Bad)` throws the original error
  away. Keep it as a `#[source]` when it helps debugging.
- **In the databroker:**
  - `SignalProvider::get_signals_values_from_provider` returns
    `Result<GetValuesProviderResponse, ()>` (`databroker/src/broker.rs:207`).
    A unit error carries no information — callers can only "fall back to the
    database". It's a pragmatic choice (every failure is handled the same way),
    but it makes debugging harder.
  - Provider APIs return `Result<(), (ActuationError, String)>` — an enum
    *plus* a free-text message in a tuple. It works, but a struct error or an
    enum with data (or `thiserror`) would be more self-describing.
  - `authorization/mod.rs:34` does `.map_err(|_| Error::InvalidPublicKey)?`,
    discarding *why* the key failed to parse (the decoder's error has a message).
  - `valid_provider_publish_signals` uses `.unwrap()` (`broker.rs:2195`) —
    relies on an invariant that the provider uuid exists.

## In the databroker

- `databroker/src/broker.rs:42`–`87` — the domain error enums: `ActuationError`,
  `RegisterSignalError`, `UpdateError`, `ReadError`, `RegistrationError`.
  Note what they derive (`Debug`, sometimes `PartialEq`, `Clone`).
- `databroker/src/permissions.rs:167` — `can_read(&self, path) -> Result<(), PermissionError>`:
  `Result<(), E>` is the idiom for "a check that can fail".
- `databroker/src/broker.rs:1276` — `.map_err(|err| match err { PermissionError::Denied => RegistrationError::PermissionDenied, ... })?`:
  converting between two error vocabularies at a layer boundary, explicitly.
- `databroker/src/vss.rs:155`–`173` — a complete hand-written error type:
  enum, `Display`, `From<serde_json::Error>`, `impl std::error::Error`.
- `databroker/src/authorization/mod.rs:26` — the same with `thiserror`.
- `databroker/src/grpc/kuksa_val_v2/conversions.rs:356` — `UpdateError::to_status_with_code`
  maps each variant to a `tonic::Status` (NOT_FOUND, INVALID_ARGUMENT,
  PERMISSION_DENIED, UNAUTHENTICATED). This is where errors become the wire protocol.
- `databroker/src/main.rs:42` — `.expect("failed to setup SIGINT signal handler")`:
  startup precondition → panic is right.
- `databroker/src/main.rs:156` — `read_metadata_file(...) -> Result<(), Box<dyn std::error::Error>>`
  uses `?` for both `std::io::Error` (open) and `vss::Error` (parse): both
  convert into the box.
- `databroker/src/lib.rs:54` — `.expect("Unable to install global logging subscriber")`.

## Exercises

| Exercise | Goal |
|---|---|
| `c07_e01_result_basics` | produce and match on `Result`s; match guards |
| `c07_e02_question_mark` | fix E0277 by returning `Result`; `ok_or` + `?` |
| `c07_e03_custom_error_enum` | a proper error enum with `Display` + `Error` |
| `c07_e04_from_conversions` | make `?` convert serde/parse errors via `From` |
| `c07_e05_thiserror` | derive all of the above with `thiserror` |
| `c07_e06_panic_vs_result` | choose panic vs Result; `anyhow` + `context` |
| `c07_e07_build_update_pipeline` | Build: a `thiserror` error enum + fail-fast/lenient parsing pipeline from a spec |
| `c07_e08_review_batch_apply` | Review: find 3 planted bugs (swallowed error, off-by-one, lost error context) |

## Further reading

- The Rust Book, ch. 9 "Error Handling" — https://doc.rust-lang.org/book/ch09-00-error-handling.html
  (especially 9.3 "To panic! or Not to panic!")
- Rust by Example, "Error handling" — https://doc.rust-lang.org/rust-by-example/error.html
- `thiserror` — https://docs.rs/thiserror ; `anyhow` — https://docs.rs/anyhow
- `std::error::Error` and `source()` — https://doc.rust-lang.org/std/error/trait.Error.html

## Check yourself

1. What exactly does `x?` expand to?
2. Why does `?` on a `serde_json::Error` compile inside `vss.rs` functions?
3. When is `expect` better than returning a `Result`? Give a databroker example.
4. Why might a library prefer an error enum over `Box<dyn Error>`?
5. What does `#[from]` generate that `#[source]` doesn't?
6. What information is lost with `Result<T, ()>`?

<details><summary>Answers</summary>

1. `match x { Ok(v) => v, Err(e) => return Err(From::from(e)) }` (for `Result`).
2. Because `impl From<serde_json::Error> for vss::Error` exists (vss.rs:167).
3. When failure means a bug or an unrecoverable startup condition, e.g.
   installing the SIGINT handler in `main.rs:42`.
4. Callers can `match` on variants and react differently (e.g. map to
   different gRPC status codes); with `Box<dyn Error>` they can only print it.
5. An `impl From<Inner> for MyError`, which lets `?` convert automatically.
   Both make `source()` return the inner error.
6. Everything: *why* it failed. The caller can only know *that* it failed.
</details>

## Where this goes next

This chapter covers the mechanics. **Chapter 17 (Error handling II)** covers the design: error chains and context, library vs application errors, who an error is for, retry/backoff/circuit breakers, panics and poisoning, and a critique of the databroker's own error types.
