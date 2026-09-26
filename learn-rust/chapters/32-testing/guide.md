# 32 — Testing like the databroker

**Why this matters for the databroker.** The databroker has four layers of
tests, and any contribution you make will be judged by them: **unit tests**
inside the source files (~190 of them, mostly `#[tokio::test]`),
**Gherkin/cucumber scenarios** that start a real gRPC server
(`databroker/tests/`), **Python integration tests** against a running
container (`integration_test/`), and **CI gates** — `cargo fmt`, `cargo
clippy -D warnings`, a feature-matrix check, and coverage. The recent commits
you saw in chapter 23 (e1fdfd1, 748a914, 7a210d0) were each accompanied by
tests that use fake providers to *prove* the broker can't be blocked. This
chapter teaches you to write those tests yourself.

---

## Unit tests: the toolbox

```rust
#[cfg(test)]              // compiled only for `cargo test`
mod tests {
    use super::*;         // the private items of the parent module are visible

    #[test]
    fn capacity_for_no_buffer_is_one() {
        assert_eq!(channel_capacity(None), Ok(1));
    }
}
```

* `cargo test` builds a test binary per target and runs every `#[test]`
  function **in parallel on threads**. A test passes if it returns without
  panicking (or returns `Ok`).
* Unit tests live **next to the code** and can see private items — that's why
  `broker.rs` has its `mod tests` at the bottom (`databroker/src/broker.rs:2523`).
* The assertion macros:

| Macro | Use |
|---|---|
| `assert!(cond, "msg {x}")` | any boolean |
| `assert_eq!(left, right, "msg")` / `assert_ne!` | shows both values on failure (needs `Debug + PartialEq`) |
| `assert!(matches!(value, Pattern))` | shape checks on enums: `matches!(r, Err(UpdateError::NotFound))` |
| `debug_assert!` | only in debug builds — for invariants in *production* code |
| `panic!`, `unreachable!` | "this line must never run" inside a test double |

* **Result-returning tests**: `fn t() -> Result<(), E>` lets you use `?`;
  an `Err` fails the test. Good when setup has many fallible steps.
* **`#[should_panic(expected = "substring")]`**: passes only if the test
  panics *and* the message contains the substring. Always give `expected`,
  otherwise any panic (even an unrelated `unwrap`) counts as a pass.
* `#[ignore]` + `cargo test -- --ignored` for slow tests.

Useful `cargo test` invocations:

```sh
cargo test -p databroker                         # one package
cargo test -p databroker broker::tests::test_multi_subscribe   # filter by name substring
cargo test -p databroker -- --nocapture          # show println!/logs of passing tests
cargo test -p databroker --features viss         # tests behind a feature
cargo test -p databroker -- --test-threads=1     # serialise (for debugging interference)
```

**Test the boundaries.** `subscribe` rejects `buffer_size > 1000`; the
interesting cases are 1000 (allowed) and 1001 (rejected) —
`test_subscribe_buffersize_out_of_range` (`broker.rs:4782`). Off-by-one bugs
live exactly on the boundary.

**Break it on purpose.** A test you've never seen fail proves nothing. After
writing one, change the code under test (`>` → `>=`) and check the test
catches it. (Tools like `cargo-mutants` automate this idea.)

## Async tests and virtual time

```rust
#[tokio::test]                              // a fresh current-thread runtime per test
async fn subscribe_gets_initial_value() { ... }

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]   // when you need real parallelism
#[tokio::test(start_paused = true)]         // virtual clock (needs tokio's "test-util" feature)
```

With `start_paused = true`, Tokio's clock is frozen and **auto-advances**
only when every task is idle waiting for a timer — then it jumps straight to
the next deadline. Consequences:

* `sleep(Duration::from_secs(3600)).await` completes instantly (in real time).
* Timestamps from `tokio::time::Instant` are exact: a value sent after
  `sleep(30ms)` is observed at exactly 30 ms. No flakiness from a loaded CI
  machine.
* `std::time::Instant` / `SystemTime` do **not** follow the virtual clock —
  code under test must use `tokio::time::Instant` for this to work.
* `tokio::time::advance(d)` moves the clock manually for step-by-step tests.

**What the databroker does instead.** Its concurrency tests use *real* time,
but only as a **safety net**: synchronise with `tokio::sync::Notify` /
`Barrier` so the test knows exactly when a fake provider has been entered,
then wrap the operation that must not block in
`tokio::time::timeout(Duration::from_secs(1), ...)` and fail if it expires
(`broker.rs:5142`–`5146`). The test is fast when the code is correct and
fails (rather than hangs forever) when it isn't. Both styles are good; paused
time is better when the *timing itself* is what you're testing (sampling
intervals, timeouts, retries).

## Test doubles for trait objects

The broker talks to providers through `Arc<dyn SignalProvider>` (chapter 23).
That indirection is what makes the broker testable: a test can register a
provider that misbehaves in exactly one controlled way. The databroker's test
module defines a whole zoo (`broker.rs:5012` onwards):

| Double | Behaviour | Proves |
|---|---|---|
| `HangingSignalProvider` (`:5012`) | `std::future::pending().await` — never answers; `Notify`s when entered | a hung provider can't block subscriptions or other providers |
| `CountingSignalProvider` (`:5038`) | counts calls in an `AtomicUsize` | a provider without matching signals isn't called at all |
| `HangingUpdateFilterProvider`, `FailingUpdateFilterProvider` | never returns / errors from `update_filter` | housekeeping continues past a bad provider |
| `BarrierSignalProvider` (`:5557`) | waits on a `Barrier` shared by N providers | providers are queried *concurrently* (a sequential loop would deadlock the barrier) |
| `PartialResponseProvider` (`:5660`) | answers only some requested ids | the broker falls back to the database for the rest |

Techniques worth stealing:

* **`std::future::pending()`** — a future that never completes. The simplest
  "hung network peer".
* **Shared counters/flags via `Arc<Atomic*>`** — keep an `Arc` clone in the
  test, hand the other to the double, assert afterwards.
* **`Notify` / `Barrier`** to turn "eventually" into "now": the test awaits
  `entered.notified()` instead of sleeping and hoping.
* **`unreachable!("must not be called")`** in a method the code under test
  should skip — a wrong call fails loudly with a clear message.

`test_get_values_queries_providers_concurrently` (`broker.rs:5599`) is a
beautiful example: two providers each wait on a 2-party `Barrier`; if
`get_values_broker` called them one after another, the first would wait
forever for the second — so the test only passes with concurrent calls.

## Table-driven tests and test builders

When one behaviour has many input combinations, write the setup once and
describe each case as data. The v2 API tests do this with
`GetValuesConfigBuilder` (`databroker/src/grpc/kuksa_val_v2/val.rs:1702`):

```rust
test_get_values_combo(
    GetValuesConfigBuilder::new()
        .request_first().first_exist().auth_first()
        .request_second().second_exist()          // exists, but not authorized
        .build(),
).await;   // the helper at val.rs:1795 derives the expected result from the config
```

Each builder method is `fn x(&mut self) -> &mut Self`, and `build(&self)`
copies the flags into a config struct. A single helper,
`test_get_values_combo`, turns a config into broker setup + request +
assertions. Adding a case is one line.

A plain table works well too:

```rust
let cases = vec![
    ("missing signal",  scenario_a, Err(not_found("A"))),
    ("not authorized",  scenario_b, Err(denied("A"))),
];
for (description, scenario, expected) in cases {
    assert_eq!(scenario.run(), expected, "case: {description}");   // name the failing case!
}
```

Always put the case name in the assertion message — otherwise a failure in
case 17 of 30 just says "assertion failed".

## Integration tests: cucumber (Gherkin)

Files in a crate's `tests/` directory are **integration tests**: each is a
separate crate that can only use the library's *public* API.
`databroker/tests/read_write_values.rs` is special: it's declared with
`harness = false` (`databroker/Cargo.toml:102`–`104`), meaning it provides
its own `main` (`read_write_values.rs:442`) instead of the libtest runner,
and hands control to **cucumber**.

Behaviour is written in plain language in
`databroker/tests/features/read_write_values.feature`:

```gherkin
Scenario: Writing the current value of a Data Entry without right permissions fails
  When a client uses a token with scope read
  And a client sets the current value of Vehicle.Speed of type float to 13.4
  Then setting the value for Vehicle.Speed fails with error code 403
```

and each sentence is bound to a Rust function by a regex or cucumber
expression:

```rust
#[when(expr = "a client uses a token with scope {word}")]      // read_write_values.rs:109
async fn a_client_uses_token(w: &mut DataBrokerWorld, scope: String) { ... }
```

`DataBrokerWorld` (`databroker/tests/world/mod.rs:168`) is the per-scenario
state: it starts a real databroker on `127.0.0.1:0` (port 0 = "pick any free
port", `world/mod.rs:219`), keeps gRPC clients, and stores the last response.
It stops the server with a custom shutdown future built from `poll_fn` — the
same "shutdown is just a future" design from chapter 31 paying off. The
`libtest` feature (`Cargo.toml:88`) and cucumber's libtest writer let
`cargo test` and CI tooling consume the output.

Run: `cargo test -p databroker --test read_write_values`.

## Python integration tests

`integration_test/` treats the databroker as a black box:

* `run.sh` starts the databroker **container** with VSS 6.0 in insecure mode,
  waits until the port answers, runs `pytest`, then stops the container.
  CI runs it after building the image
  (`.github/workflows/kuksa_databroker_build.yml:356`).
* `test_databroker.py` uses the Python `kuksa-client` library — i.e. tests
  the *wire protocol* the way real apps use it.
* `integration_test/viss/` is a pytest-bdd (Gherkin again) compliance suite
  for the VISS WebSocket/HTTP interface; it needs a `--features viss` build.

## Lints, formatting and CI

The CI workflow (`.github/workflows/kuksa_databroker_build.yml`) and the
pre-commit hooks (`.pre-commit-config.yaml`) enforce:

| Gate | Command | Line |
|---|---|---|
| formatting | `cargo fmt -- --check` | `:68` |
| lints, all targets incl. tests | `cargo clippy --all-targets -- -W warnings -D warnings` | `:71` |
| lints with VISS | `cargo clippy --features viss --all-targets -- ...` | `:74` |
| every feature compiles alone | `cargo hack check --each-feature` | `:168` |
| tests + coverage | `cargo llvm-cov --all-features --workspace` | `:156` |
| black-box | `integration_test/run.sh` | `:356` |

Before pushing, run at least:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test -p databroker
```

`clippy` is not just style: it catches real bugs (`manual_checked_ops`,
`await_holding_lock`, `unwrap_or_default` misuse...). When clippy is wrong,
silence it locally with a reason:
`#[allow(clippy::too_many_arguments)]` (as `broker.rs` does on `add_entry`).

This course's own runner enforces the same discipline on the solutions:
`cargo rl check-solutions` runs clippy with `-D warnings`.

## Property-based testing with proptest

An example-based test checks one input you thought of. A **property** is a
statement that must hold for *every* input, such as "decoding what I
encoded gives back the original", "the output is sorted", or "a glob with
no wildcards matches exactly its own path". `proptest` generates hundreds
of inputs from a **strategy**. When one fails, it **shrinks** the input to
a minimal counterexample, which is usually the bug stated plainly.

```rust
use proptest::prelude::*;

fn arb_path() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Z][a-z]{0,6}", 1..5).prop_map(|parts| parts.join("."))
}

proptest! {
    #[test]
    fn exact_glob_matches_itself(path in arb_path()) {
        prop_assert!(glob_matches(&path, &path));
    }
}
```

Building blocks:

| Strategy | Generates |
|---|---|
| `any::<T>()` | any value of `T` (numbers, `bool`, `String`, tuples, `Vec`s …) |
| `"[a-z\\\n]{0,8}"` | strings matching a regex, which makes rare characters common |
| `0..100i64`, `prop::collection::vec(s, 0..5)` | ranges and collections |
| `s.prop_map(f)`, `prop_oneof![a, b]`, `Just(x)` | transform, choose, constant |
| `s.prop_filter("why", pred)` | reject some values (use sparingly; prefer generating valid ones) |

Good properties to look for:

* **Round trips.** `decode(encode(x)) == x`, as in proto ↔ `DataValue` (`conversions.rs`).
* **Invariants.** The output is sorted, has no duplicates, or stays within `min..=max`.
* **Oracles.** A slow, obviously correct version agrees with the fast one.
* **Metamorphic relations.** Adding a subscription can never make the lowest interval *higher*.

Two practical settings: `failure_persistence: None` stops proptest from writing
`proptest-regressions/` files next to your source (commit them in a real
project; they are regression tests). `rng_seed: RngSeed::Fixed(..)` makes
every run identical, which suits CI and this course. Real projects often
keep the random seed and rely on the persisted failures instead.

## Model-based testing

To test a *stateful* API (a filter manager, a cache, a subscription
registry), generate **sequences of operations**, apply each one to the
real implementation *and* to a tiny obviously-correct **model**, and
compare after every step. The model is usually a `HashMap` or `Vec` with
the naive algorithm. This is how you catch bugs in interactions nobody
wrote a scenario for, like the "stale cache after removal" bug in the
checkpoint. `proptest` can generate the sequences too (a
`Vec<Op>` strategy), and adds shrinking to the shortest failing sequence.

## Doc tests

Code blocks in `///` doc comments are compiled and run by `cargo test`
as **doc tests**. They keep examples honest:

```rust
/// Splits a VSS path into its segments.
///
/// ```
/// assert_eq!(mylib::segments("Vehicle.Speed"), vec!["Vehicle", "Speed"]);
/// ```
pub fn segments(path: &str) -> Vec<&str> { path.split('.').collect() }
```

* They only exist for **library** targets (`src/lib.rs` and its modules),
  and use the crate like an external user (`mylib::...`). That's why this
  course's exercises, which are integration-test files, contain none.
* Annotate blocks with `ignore`, `no_run` (compile only), `should_panic`
  or `compile_fail` (proves something *doesn't* compile, which is great
  for documenting type-safety guarantees). Hide setup lines with `# `.
* Run just the doc tests with `cargo test --doc`.

## Beyond unit tests: Miri, fuzzing, loom, coverage, mutation testing

| Tool | Finds | Run it |
|---|---|---|
| **Miri** | undefined behaviour in `unsafe` code (and in dependencies): out-of-bounds reads, use-after-free, invalid aliasing, data races | `rustup +nightly component add miri`, then `cargo +nightly miri test` (chapter 26) |
| **cargo-fuzz** (libFuzzer) | crashes and panics on hostile input, such as a parser fed random bytes. A fuzzer would find `conversions.rs:431`'s `todo!()` quickly | `cargo install cargo-fuzz`, `cargo fuzz init`, write a `fuzz_target!`, `cargo +nightly fuzz run target` |
| **loom** | concurrency bugs, by exhaustively exploring thread interleavings of code written against `loom::sync` types | `RUSTFLAGS="--cfg loom" cargo test` |
| **cargo-llvm-cov** | which lines and branches your tests never execute | `cargo llvm-cov --html` |
| **cargo-mutants** | tests that execute code without checking it. It mutates the code (`<` → `<=`, return `Default`) and reports mutants your suite didn't kill | `cargo mutants` |

Mutation testing asks the same question as this chapter's checkpoint:
*does the suite fail when the code is wrong?* Coverage only tells you the
code ran.

## Checkpoint

`c32_e06_checkpoint_part5` closes Part V, and it reverses the usual
roles. The implementation is given (three of them), and **you write the
tests**. A suite is only good if it passes on correct code and fails on
wrong code, so the grader runs your suite against a correct filter manager
and two subtly broken ones, modelled on `databroker/src/filter/filter_manager.rs`.

It draws on: traits and generics over `F: IntervalFilter + Default`
(chapter 09), collections (05), error values as test results (07, 17),
closures and iterators (10), and this chapter's scenario tables and
model-based testing. Read the contract, not the buggy code. For every
sentence, ask what a careless implementation would get wrong.

## Mental model

| Concept | Analogy |
|---|---|
| unit test | A spot-check of one machine part on the workbench |
| integration test | Assembling the car and driving it round the car park |
| test double | A crash-test dummy: behaves in a controlled, reproducible way |
| paused time | A film you can fast-forward exactly to the next interesting frame |
| `Notify`/`Barrier` in tests | A starting pistol, instead of "wait a bit and hope everyone's ready" |
| table-driven test | A checklist: one procedure, many rows |
| CI gates | The inspection line nothing leaves without passing |

**Guarantee**: `cargo test` runs tests in isolation *from each other's
memory* only in the sense that each test has its own stack — they share the
process, globals and environment variables. Anything global (env vars,
`static`s, the current directory, a fixed port) can make tests interfere.

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "Tests run one after another" | In parallel threads by default; shared globals (env vars, fixed ports) cause flakiness. |
| "`#[should_panic]` is enough" | Without `expected = "..."` any panic passes — including a bug in the test. |
| "Integration tests can use private functions" | Files in `tests/` only see the public API. |
| "`sleep` in a test makes it reliable" | It makes it slow *and* flaky; synchronise (`Notify`, channels) or use paused time. |
| "Paused time affects `std::time::Instant`" | Only `tokio::time` follows the virtual clock. |
| "Green tests mean correct code" | Only if the tests can fail. Break the code once to check. |

## Smells and idioms

* **Sleep-based synchronisation** — replace with `Notify`, a oneshot, or
  paused time.
* **`unwrap()` in tests** is fine (a panic *is* a failure), but
  `expect("what we were doing")` makes failures readable.
* **Asserting only `is_err()`** — check *which* error:
  `assert!(matches!(r, Err(UpdateError::OutOfBoundsMinMax)))`.
* **Copy-pasted setup** — `broker.rs` has `helper_add_int8/16/32/64/...`
  near-duplicates (`broker.rs:3127` and neighbours); a generic helper or a
  builder would cut hundreds of lines.
* **Hard-coded ports** — use port `0` and ask the listener for its address, as
  `world/mod.rs:219` does.
* **Testing through the network when a trait call would do** — call the `Val`
  trait method directly (chapter 30) for handler logic; save real sockets for
  the integration layer.

## In the databroker

| Where | What to notice |
|---|---|
| `databroker/src/broker.rs:2523` | `#[cfg(test)] pub mod tests` — public so other modules' tests reuse helpers |
| `databroker/src/broker.rs:3127` | `helper_add_int32` — shared setup helper |
| `databroker/src/broker.rs:4782` | boundary test for `buffer_size` |
| `databroker/src/broker.rs:4988` | a test pinning down tokio's `Lagged(n)` behaviour |
| `databroker/src/broker.rs:5012`–`5110` | test doubles for `SignalProvider` / `ActuationProvider` |
| `databroker/src/broker.rs:5112` | hung provider must not block subscriptions (Notify + timeout) |
| `databroker/src/broker.rs:5599` | Barrier proves concurrency |
| `databroker/src/grpc/kuksa_val_v2/val.rs:1407` | `streaming_request`: fake a client stream from a `Vec` |
| `databroker/src/grpc/kuksa_val_v2/val.rs:1702` | `GetValuesConfigBuilder` |
| `databroker/src/glob.rs:207` | ~60 table-like tests for wildcard rules |
| `databroker/Cargo.toml:102` | `[[test]] harness = false` for cucumber |
| `databroker/tests/features/read_write_values.feature` | Gherkin scenarios |
| `databroker/tests/world/mod.rs:195` | `start_databroker`: real server on port 0 |
| `integration_test/run.sh` | container + pytest black-box tests |
| `.github/workflows/kuksa_databroker_build.yml:66`–`74` | fmt + clippy gates |

## Exercises

| Exercise | Goal |
|---|---|
| `c32_e01_unit_tests_and_should_panic` | You write the tests: `assert_eq!`, `matches!`, `?`, `should_panic(expected)` |
| `c32_e02_async_tests_paused_time` | Implement interval sampling with `tokio::time::Instant`; write a paused-time test |
| `c32_e03_test_doubles_for_traits` | Build Static / Failing / Hanging / Counting / Unavailable providers |
| `c32_e04_test_builders` | A `&mut Self` scenario builder driving a table of cases |
| `c32_e05_property_testing` | Write a proptest strategy that reaches the corners; let it find two codec bugs |
| `c32_e06_checkpoint_part5` | **Checkpoint:** write a test suite that catches two subtly broken filter managers |

## Further reading

* Rust Book ch. 11 "Writing Automated Tests".
* Tokio docs — "Testing" topic (tokio.rs/tokio/topics/testing), `tokio::time::pause`, `advance`.
* cucumber-rs book — <https://cucumber-rs.github.io/cucumber/main/>.
* Clippy lint list — <https://rust-lang.github.io/rust-clippy/>.
* `cargo-hack`, `cargo-llvm-cov`, `cargo-mutants` READMEs.
* The proptest book — <https://proptest-rs.github.io/proptest/>.
* Rust Fuzz Book — <https://rust-fuzz.github.io/book/>.
* Miri — <https://github.com/rust-lang/miri>; loom — <https://docs.rs/loom>.
* rustdoc book, "Documentation tests" — <https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html>.

## Check yourself

1. Why can the unit tests in `broker.rs` call private functions, while `databroker/tests/read_write_values.rs` can't?
2. What does `harness = false` change?
3. A test uses `start_paused = true` but the code under test measures time with `std::time::Instant`. What happens?
4. How does `test_get_values_queries_providers_concurrently` prove concurrency without measuring time?
5. Why is `#[should_panic]` without `expected` risky?
6. Name the four CI gates your change must pass besides tests.
7. Your round-trip property passes 512 cases, yet a user hits a bug. Name two likely reasons.
8. Coverage is 100%, but a mutation tool reports surviving mutants. What does that tell you?
9. Why can't the exercise files in this course contain doc tests?

<details><summary>Answers</summary>

1. Unit tests are a child module (`mod tests`) of the code, so they see its
   private items; `tests/*.rs` are separate crates using only the public API.
2. The file supplies its own `main` instead of the libtest runner — here, to
   run cucumber.
3. `std::time::Instant` ignores the virtual clock; with auto-advance the code
   sees ~0 real time elapsed, so time-based logic misbehaves.
4. Each provider waits on a shared 2-party `Barrier`; sequential calls would
   deadlock, so the test only completes if both calls run concurrently.
5. Any panic passes it — including an unrelated `unwrap()` failing during setup.
6. `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (with and
   without `viss`), `cargo hack check --each-feature`, plus the integration
   tests / coverage run.
7. The strategy never generates the troublesome inputs (for example, strings
   without backslashes), or the property is too weak to notice the error.
   Check what your generator actually produces, as `strategy_covers_the_tricky_cases` does.
8. The tests execute that code but don't assert on its result, so a wrong
   answer still passes.
9. Doc tests only run for library targets (`src/lib.rs`), and the exercises
   are integration-test files under `tests/`.

</details>

## Where this goes next

**Chapter 34 (Reading any Rust codebase)** makes a project's tests one of your first reading stops: they're executable documentation of how the code is meant to be used. Chapter 37's projects give you only tests to work from.
