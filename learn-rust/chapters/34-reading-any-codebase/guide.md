# 34 — Reading any Rust codebase

> **Builds on:** chapters everything so far, especially 14 (traits), 16 (lifetimes), 18 (macros), 24 (async internals) and 33 (Cargo).

**Why this matters.** The goal of this course isn't only to understand the
databroker. It's to open *any* Rust project (a crate you depend on, a new
job's monorepo, the standard library) and understand it in hours, not weeks.
That's a skill with a method, not a talent. This chapter teaches the method,
works through one real crate with it, and then has you practise on four
more. You "read then rebuild": read how a well-known crate does something,
write a small version yourself, and check it behaves identically.

Everything you need is already on your disk. Cargo keeps the full source of
every dependency under
`~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/`. The exact
versions are in `learn-rust/Cargo.lock`. References in this chapter use
that layout (the `*` is a hash that differs per machine).

---

## The method

Read in this order. Each step narrows what you need to read next.

1. **`Cargo.toml` (5 minutes).** What kind of crate is it (lib, bin, both,
   proc-macro)? Which edition and MSRV? What are the dependencies, and which
   are heavy (tokio, tonic, serde)? What features exist, and which are on by
   default? Is there a `build.rs`? Any `[workspace]`? Each answer tells you
   what the code *can* do. A crate with no async dependency has no async
   code; a `build.rs` means some code is generated.
2. **The crate root (`src/lib.rs` or `src/main.rs`).** The `mod` lines are the
   table of contents. `pub use` lines show the public API the author intends
   you to see. `#![...]` attributes (`forbid(unsafe_code)`, `no_std`,
   `deny(missing_docs)`) tell you the rules the author set.
3. **The public types.** Before any function body, list the main `pub struct`s,
   `pub enum`s and `pub trait`s, and what each one owns. `cargo doc --open`
   is the fastest view: it shows every public item, its trait
   implementations, and links to source.
4. **One path end to end.** Pick one realistic operation (for the
   databroker, "publish a value") and follow it from the entry point to the
   end, reading only the functions on that path. Write the chain down
   (guide 99 does this four times for the databroker).
5. **The tests.** Tests are executable documentation. They show intended
   use, edge cases, and what the author worried about. Integration tests in
   `tests/` show the public API as users see it.
6. **History, for "why".** `git log -p -- <file>` and `git blame` explain
   odd-looking code. The databroker's last three commits are a good example:
   each one explains a concurrency decision you'd otherwise have to guess.

Stop when you can answer your question. You rarely need to read everything.

### Checklist

Copy this into your notes when you start on a new codebase:

- [ ] Crate type, edition, MSRV, features, build script?
- [ ] Top-level modules and what each is for (one line each)
- [ ] The 3–7 central types and who owns what (draw it)
- [ ] Where are the entry points? (`main`, `pub fn`s, trait impls such as `Service::call`)
- [ ] One operation traced end to end
- [ ] Concurrency: which tasks/threads exist, what's shared, which locks and channels
- [ ] Errors: the error types, and where they're converted or swallowed
- [ ] `unsafe`: where it is, and the invariant each block relies on
- [ ] Generated or macro code: what generates it, and where the output is
- [ ] Tests: how to run them, what they cover

## Tools that make reading fast

| Tool | Use it for |
|---|---|
| rust-analyzer **Go to Definition** | jump from any name to its definition, including into dependencies and std |
| **Find All References** | who uses this type or function |
| **Go to Implementations** | every `impl Trait for X` of a trait, or every trait a type implements |
| **Call Hierarchy** (incoming/outgoing) | who calls this function, and what it calls |
| **Hover** and **inlay hints** | the inferred type of any expression; essential for iterator chains and closures |
| **Expand Macro Recursively** | the code a macro call expands to, in place |
| `cargo doc --open` | browsable API docs for your crate *and every dependency*, at the exact versions you use |
| `cargo tree` | the dependency graph (chapter 33) |
| `cargo expand` | the whole module after macro expansion (`cargo install cargo-expand`) |
| `git log -p`, `git blame`, `git log -S 'text'` | why code changed; `-S` finds the commit that added or removed a string |
| `rg` (ripgrep) | text search when names are built by macros and the IDE can't follow them |

## Worked example: tokio::sync::broadcast

We'll apply the method to the channel under every databroker subscription.
Paths are relative to `~/.cargo/registry/src/index.crates.io-*/tokio-1.53.1/`.

**1. Cargo.toml.** `sync = []` (line 131) is an empty feature that only
switches code on, and `full = [...]` (line 72) enables everything. So we
expect `#[cfg(feature = "sync")]` gates around the channel code.

**2. Crate root.** `src/lib.rs:564` declares `pub mod sync;`. In
`src/sync/mod.rs:452` the modules are wrapped in `cfg_sync! { ... }`, a
macro defined in `src/macros/cfg.rs:461` that expands to the
`#[cfg(feature = "sync")]` attributes we predicted. `pub mod broadcast;` is
at `src/sync/mod.rs:461`.

**3. Read the module docs first.** `src/sync/broadcast.rs` opens with about
130 lines of `//!` documentation. Lines 40–56 state the exact lag semantics
the databroker depends on: when the channel is full the oldest value is
overwritten, a receiver that missed it gets `Lagged(n)`, and its cursor jumps
to the oldest retained value. Good crates put the design in the module docs.
Always read them before the code.

**4. The public types and their owners.**

```text
Sender<T> ─┐                       (broadcast.rs:179)
           ├─► Arc<Shared<T>>      (broadcast.rs:370)
Receiver<T>┘     buffer: Box<[Mutex<Slot<T>>]>   ring of slots
  next: u64      mask: usize                     capacity - 1
(:253, :258)     tail: Mutex<Tail>               (:391) pos, rx_cnt, closed
                 Slot<T> (:406): rem, pos, val
```

Every receiver keeps its own cursor (`next`); the sender keeps one write
position (`tail.pos`). That already explains most of the behaviour.

**5. One path: `send` then `recv`.**

- `channel` (`:540`) rounds capacity up to a power of two (`:583`) so that
  `pos & mask` can replace `pos % capacity`.
- `Sender::send` (`:661`) locks `tail`, fails if `rx_cnt == 0`, writes the
  value into slot `pos & mask`, records `slot.pos = pos` and the number of
  receivers still to read it, and advances `tail.pos`.
- `Receiver::recv_ref` (`:1257`) looks at slot `next & mask`. If
  `slot.pos == next` it's the value we want. Otherwise the slot is either
  older (nothing new yet → `Empty` or `Closed`) or newer (we were lapped →
  lines 1335–1355 compute `missed` and return `Lagged`).

**6. What to skip on a first read.** Waker lists, `UnsafeCell`, and the lock-ordering
comment at `:1267`. They're about *async waiting* and performance,
not the channel's logic. Come back to them if your question is about those.

**7. Tests.** `tests/sync_broadcast.rs` has `lagging_rx` (`:234`) and three
`lagging_receiver_recovers_after_wrap_*` tests (`:417`–`:448`), each an
executable statement of the semantics from step 3.

**8. Rebuild.** `c34_e01` has you write the same logic in about 60 lines and
checks it against real tokio on the same script. If your version and
tokio's disagree, one of your beliefs about the code is wrong, which is
the fastest possible feedback on your reading.

## Reading trait-heavy and generic-heavy code

Generic signatures look intimidating, but they have a structure. Read them
in three passes:

1. **Ignore the bounds.** Look only at the shape: what goes in, what comes out.
2. **Read each bound as a requirement.** Put it in words.
3. **Ask why each bound is there.** Usually one of: "it's shared across
   threads" (`Send + Sync`), "it's stored or spawned" (`'static`), or "we
   need to call method X" (the trait that provides X).

The databroker's `serve_with_incoming_shutdown`
(`databroker/src/grpc/server.rs:190`):

```rust
pub async fn serve_with_incoming_shutdown<F, I, IO, IE>(
    incoming: I, broker: DataBroker, ..., signal: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Future<Output = ()>,
    I: Stream<Item = Result<IO, IE>>,
    IO: AsyncRead + AsyncWrite + Connected + Unpin + Send + 'static,
    IO::ConnectInfo: Clone + Send + Sync + 'static,
    IE: Into<Box<dyn std::error::Error + Send + Sync>>,
```

Pass 1: "takes a stream of incoming connections and a shutdown future, and
serves until the future completes." Pass 2: `IO` is "anything you can read
and write bytes on", which covers both TCP and Unix sockets (that's the
point of the generics). Pass 3: `Send + 'static` because each connection is
handed to a spawned task (chapter 20).

**Trait-heavy code** hides control flow: `x.foo()` could run any of a dozen
impls. Tactics:

- Find the *concrete* type first (hover in rust-analyzer), then Go to
  Implementations on the trait and pick the impl for that type.
- Blanket impls (`impl<T: Display> ToString for T`) apply without any
  `impl` for your type. If a method appears from nowhere, search for
  `impl<T` in the defining crate.
- `From`/`Into`/`?` hide conversions: `?` calls `From::from` on the error.
  anyhow's `impl<E> From<E> for Error where E: StdError + Send + Sync + 'static`
  (`anyhow-1.0.104/src/error.rs:674`) is why `?` works on every error type.
- Sealed traits (`pub trait Index: private::Sealed` at
  `serde_json-1.0.151/src/value/index.rs:37`) mean "you can call this, but
  only the listed types implement it". The impls are all in that file.
- Extension traits (`StreamExt`, `ServiceExt`, `anyhow::Context`) add methods
  to types from elsewhere. If a method isn't on the type, check which traits
  the file imports. Two imported traits with the same method name give
  E0034 ("multiple applicable items"); `c34_e02` shows how fully qualified
  syntax (`anyhow::Context::context(result, "...")`) resolves it.

`c34_e02` (anyhow) and `c34_e04` (`serde_json::Value`) practise exactly this.

## Reading data-structure code

Data structures are easiest to read through their **invariant**: the
property every public method preserves. Find it in the docs or the struct's
field comments, then check each method maintains it.

`indexmap` (`indexmap-2.14.2/src/inner.rs:29`):

```rust
pub(crate) struct Core<K, V> {
    /// indices mapping from the entry hash to its index.
    indices: Indices,
    /// entries is a dense vec maintaining entry order.
    entries: Entries<K, V>,
}
```

The invariant: "`indices` maps each key to its position in `entries`". Now
the two removal methods explain themselves. `swap_remove_finish` (`:587`)
moves the last entry into the hole, so exactly one index needs fixing.
`shift_remove_finish` (`:564`) shifts everything left, so every later index
needs decrementing. That's O(1) against O(n), and the whole API follows from
this one design choice. `c34_e03` rebuilds it and compares it against the
real crate step by step.

## Reading macro-generated and build-generated code

When "go to definition" lands on a macro, or on a name that doesn't exist in
any source file, the code was generated. There are three kinds:

| Kind | Example | How to see the output |
|---|---|---|
| `macro_rules!` | `cfg_sync!` in tokio, `vec![]` | rust-analyzer "Expand Macro Recursively", or `cargo expand` |
| proc-macro (derive/attribute) | `#[derive(Deserialize)]`, `#[async_trait]`, `#[tokio::main]` | `cargo expand --test <name>` or `cargo expand path::to::module` |
| build script output | `tonic::include_proto!("kuksa.val.v2")` | the file in `target/<profile>/build/<crate>-<hash>/out/` |

`include_proto!` (`databroker-proto/src/lib.rs:143`) is just
`include!(concat!(env!("OUT_DIR"), concat!("/", "kuksa.val.v2", ".rs")))`
(the macro is at `tonic-0.14.6/src/macros.rs:31`). The file is plain
Rust you can open:

```sh
cd learn-rust
find target -name kuksa.val.v2.rs -path '*databroker-proto*'
```

Read it with the `.proto` file side by side
(`databroker-proto/proto/kuksa/val/v2/val.proto`): each `message` became a
struct, each `oneof` an enum, and each `rpc` a method on the `Val` trait in
`pub mod val_server`. The part worth understanding is `ValServer::call`: a
`match` on the request path that wraps each route in a small struct
implementing tonic's `UnaryService` or `StreamingService`, then calls *your*
trait method. That's how `impl Val for DataBroker` in
`databroker/src/grpc/kuksa_val_v2/val.rs` gets called at all. `c34_e05`
has you answer questions about this file (checked against the file itself)
and write a miniature version.

Don't read generated code top to bottom. Find the one piece that connects to
the code you care about, as above.

## Reading async code

In async code, *where* something runs matters as much as *what* it does.

1. **Find every task.** Search for `tokio::spawn`, `spawn_blocking`, and
   `JoinSet`. Each one is a concurrent actor. In the databroker:
   the housekeeping task (`databroker/src/broker.rs:2367`) and one task per
   provider stream (`databroker/src/grpc/kuksa_val_v2/val.rs:933`).
2. **Find what connects them.** Channels (`mpsc`, `broadcast`, `oneshot`,
   `watch`) and shared state (`Arc<RwLock<...>>`). Draw the tasks as boxes
   and the channels as arrows.
3. **Find every `.await` while a lock is held.** Each one is a place where
   another task can wait on that lock. The snapshot-then-await pattern
   (chapter 21) exists because of this.
4. **Find `select!` loops.** Each branch is an event the task reacts to.
   Check what happens to the other branches' futures when one wins
   (cancellation, chapter 24).
5. **Find the shutdown path.** How does each task end? A dropped channel, a
   broadcast trigger, or never?

## Reading unsafe code

`unsafe` marks where the compiler stopped checking and the author took over.
Read it as a proof:

1. Find every `unsafe` block and `unsafe fn` (`rg -n 'unsafe'`). tokio's
   `broadcast.rs` has 17 lines mentioning `unsafe` in 1,797.
2. Each block should have a `// SAFETY:` (or `// Safety:`) comment stating the
   invariant it relies on. `broadcast.rs:1299` says "called while locked".
3. Find where that invariant is established, and check that every code path
   into the block maintains it.
4. Read the safe wrapper around it: the public API should make it
   impossible for safe callers to break the invariant. That's what makes the
   abstraction sound (chapter 26).

anyhow is a good study: `Error` is a single pointer because it builds its own
vtable (`struct ErrorVTable`, `anyhow-1.0.104/src/error.rs:722`) with
`unsafe`. `c34_e02` gets the same behaviour from a safe `Box<dyn Error>`,
which is a useful reminder that the unsafe version buys a smaller `Error`,
not different semantics.

## Mental model

| Idea | Analogy |
|---|---|
| The reading method | Reading a map before walking the city: overview, main roads, then one route |
| Module docs | The building's floor plan by the entrance |
| An invariant | The rule a librarian keeps: "every book is on the shelf its catalogue card says" |
| Generics | A form with blanks, where the bounds say what may go in each blank |
| Generated code | A printout from a machine: don't read it all, find the page you need |
| `unsafe` block | A signed statement "I checked this by hand", with the reasons written next to it |
| Read-then-rebuild | Explaining something back in your own words, and checking your version gives the same answers |

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "I have to read everything before I understand anything." | Read the manifest, root and public types, then one path. Stop when your question is answered. |
| "Library code is magic I can't read." | It's ordinary Rust on your disk; Go to Definition opens it. |
| "If Go to Definition fails, the code doesn't exist." | It's probably generated by a macro or a build script. Expand it or find the `OUT_DIR` file. |
| "A long `where` clause means complicated logic." | Bounds are requirements, not logic. Read the shape first. |
| "`unsafe` means the code is dangerous." | It means the compiler delegated a proof to the author. Check the `SAFETY` comments. |
| "A method call runs the method on that type." | With traits it runs whichever impl applies, possibly a blanket impl or an extension trait. |

## Smells and idioms

When you read a codebase, you'll also judge it. Signs of healthy code:

- module docs that explain the design; `// SAFETY:` on every unsafe block;
  small public API with private fields; tests next to the code they test;
  `#![forbid(unsafe_code)]` where unsafe isn't needed.

Warning signs (not proof of bugs, but places to look carefully):

- deeply nested `match` towers (the databroker's `open_provider_stream`,
  `val.rs:938-1068`); `.unwrap()` on values from the network; `todo!()`
  reachable from requests; locks held across `.await`; `clone()` of large
  structures inside loops; `#[allow(...)]` on whole modules.

## In the databroker and in the wild

- Apply the method to the databroker: `databroker/Cargo.toml` → `src/lib.rs`
  (modules) → `broker.rs` types (`Database`, `Subscriptions`, `DataBroker`,
  `AuthorizedAccess`) → the PublishValue path (guide 99) → `broker.rs` tests.
- `databroker/src/grpc/server.rs:190`: a generic-heavy signature, read above.
- `databroker/src/grpc/kuksa_val_v2/val.rs:933`: a spawned task with a
  `select!` loop; the concurrency map starts here.
- tokio `broadcast` (worked example above; exercise c34_e01).
- anyhow `error.rs`, `context.rs`, `fmt.rs` (exercise c34_e02).
- indexmap `inner.rs` (exercise c34_e03).
- serde_json `value/mod.rs`, `value/index.rs` (exercise c34_e04).
- the tonic-generated `kuksa.val.v2.rs` (exercise c34_e05).

## Exercises

| Exercise | Goal |
|---|---|
| `c34_e01_read_tokio_broadcast` | rebuild broadcast's ring buffer and lag rule; match real tokio on a script |
| `c34_e02_read_anyhow_error` | rebuild anyhow's `Error`, context chain and exact Display/Debug output |
| `c34_e03_read_indexmap` | rebuild an insertion-ordered map with `swap_remove`/`shift_remove`; match real indexmap step by step |
| `c34_e04_read_serde_json_value` | rebuild `Value` with sealed-trait indexing, never-panicking `[]`, and JSON Pointer |
| `c34_e05_read_tonic_generated_code` | answer questions about generated tonic code (checked against the file) and write a mini generated server |

## Further reading

- rust-analyzer manual, "Features": <https://rust-analyzer.github.io/manual.html>
- `cargo-expand`: <https://github.com/dtolnay/cargo-expand>
- *Rust for Rustaceans* (Jon Gjengset), chapter 13, "The Rust Ecosystem"
- The source of any crate: the "source" link on its docs.rs page, or
  `~/.cargo/registry/src/`
- Tokio's `broadcast` module docs:
  <https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html>

## Check yourself

1. List the six steps of the reading method in order.
2. Go to Definition on a name lands on `tonic::include_proto!`. Where is the real code?
3. In `serve_with_incoming_shutdown`, why must `IO` be `Send + 'static`?
4. What is indexmap's core invariant, and why does it make `shift_remove` O(n)?
5. A method appears on a type, but the type has no such method in its `impl` blocks. Name two places it could come from.
6. What should you look for next to every `unsafe` block?
7. In async code, what's the first thing to search for, and why?

<details><summary>Answers</summary>

1. `Cargo.toml` → crate root (`lib.rs`/`main.rs`) → public types → one path
   end to end → tests → history (`git log -p`, `blame`).
2. In the generated file under
   `target/<profile>/build/databroker-proto-<hash>/out/kuksa.val.v2.rs`, written
   by `databroker-proto/build.rs` and included via `OUT_DIR`.
3. Each accepted connection is handed to a spawned task, which may run on
   any worker thread (`Send`) and may outlive the caller's stack frame
   (`'static`).
4. `indices[key] == position of key in entries`. Removing from the middle of
   `entries` while keeping order shifts every later entry left, so every
   later index must be updated.
5. A trait implemented for it (including blanket impls like
   `impl<T: Display> ToString for T`), or an extension trait imported into
   scope (`StreamExt`, `anyhow::Context`). `Deref` to another type is a third.
6. A `// SAFETY:` comment stating the invariant, where that invariant is
   established, and whether the safe API around it prevents misuse.
7. Every `spawn` (tasks are the concurrent actors), then the channels and
   shared state that connect them.
</details>
