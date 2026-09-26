# 00 — Setup and the big picture

**Why this matters.** Before learning any Rust, you need to know what you're
working towards. This chapter covers what the databroker is, how its code is
organised, which tools you'll use every day, and how to read the compiler,
which will be your main teacher throughout the course.

---

## What the databroker is

Picture a modern car. Hundreds of values change constantly: vehicle speed,
battery charge, whether a door is open, the target cabin temperature. Many
programs want those values: a dashboard app, a charging planner, a cloud
uploader. Many other programs *know* those values because they read them from
the vehicle's buses (CAN, SOME/IP, and so on).

The **KUKSA databroker** sits between them as a *signal server*:

```
            VSS CONSUMERS (northbound)                "apps"
     ┌──────────────┐  ┌───────────────┐  ┌──────────────┐
     │ dashboard    │  │ charge planner│  │ cloud bridge │
     └──────┬───────┘  └──────┬────────┘  └──────┬───────┘
            │ get / subscribe │ actuate          │
            ▼                 ▼                  ▼
     ╔══════════════════════════════════════════════════╗
     ║                KUKSA DATABROKER                  ║
     ║  • in-memory DB of signals (Vehicle.Speed, …)    ║
     ║  • checks permissions (JWT)                      ║
     ║  • fans updates out to subscribers               ║
     ║  • routes actuation requests to providers        ║
     ╚══════════════════════════════════════════════════╝
            ▲                 ▲                  ▲
            │ publish values  │ receive actuation│
     ┌──────┴───────┐  ┌──────┴────────┐  ┌──────┴───────┐
     │ CAN feeder   │  │ HVAC service  │  │ GPS provider │
     └──────────────┘  └───────────────┘  └──────────────┘
            VSS PROVIDERS (southbound)          "hardware side"
```

Vocabulary you'll see everywhere (see `doc/terminology.md` for the full list):

| Term | Meaning | In the code |
|---|---|---|
| **VSS** | *Vehicle Signal Specification*, a standard tree of signal names such as `Vehicle.Cabin.Door.Row1.DriverSide.IsOpen` | loaded from `data/vss-core/*.json` by `databroker/src/vss.rs` |
| **Signal / entry** | One node in that tree that holds a value | `Entry` in `databroker/src/broker.rs:113` |
| **Sensor** | Value measured by the car (speed) | `EntryType::Sensor` |
| **Actuator** | Something that can be *commanded* (a window). It has a current value and a *target* | `EntryType::Actuator`, `actuator_target` |
| **Attribute** | Value that rarely changes (VIN, number of doors) | `EntryType::Attribute` |
| **Datapoint** | A value plus timestamps | `Datapoint` in `broker.rs:106` |
| **Metadata** | Type, unit, min/max, allowed values, description | `Metadata` in `broker.rs:90` |
| **Provider** | Client that supplies values or carries out actuations | `SignalProvider` / `ActuationProvider` traits, `broker.rs:188-208` |
| **Consumer** | Client that reads, subscribes or requests actuation | the gRPC `Val` service methods |

The databroker exposes these operations over **gRPC** (an RPC protocol on
HTTP/2 whose messages are defined in `.proto` files). There are two API
versions, `kuksa.val.v1` and the newer `kuksa.val.v2`, and an optional
WebSocket JSON protocol called **VISS**.

## How the code is organised

```
kuksa-databroker/                 ← a Cargo *workspace* (several crates)
├── Cargo.toml                    members: databroker, databroker-proto, databroker-cli
├── proto/                        git submodule: the .proto API definitions
├── databroker-proto/             crate: generates Rust types from proto/ at build time
│   └── build.rs                  runs protoc + tonic-prost-build
├── databroker/                   crate: the server (library + binary)
│   ├── src/main.rs               the binary: CLI args, logging, start the server
│   ├── src/lib.rs                the library root: declares the modules below
│   ├── src/broker.rs             ★ the core: database, subscriptions, providers
│   ├── src/types.rs              DataType, DataValue, comparisons
│   ├── src/permissions.rs        who may read/write what
│   ├── src/glob.rs               VSS wildcard matching (Vehicle.*.IsOpen)
│   ├── src/vss.rs                parse VSS JSON files
│   ├── src/filter/               per-signal sampling intervals for providers
│   ├── src/authorization/        JWT decoding → Permissions
│   ├── src/grpc/                 gRPC server + kuksa.val v1/v2 services
│   ├── src/viss/                 optional WebSocket VISS server (feature "viss")
│   └── tests/                    cucumber (BDD) integration tests
├── databroker-cli/               crate: interactive command-line client
└── lib/                          client libraries (kuksa, common), not in the workspace
```

A request comes in over gRPC and flows like this:

```
network ─► tonic server (grpc/server.rs)
             │  interceptor: JWT → Permissions   (grpc/server.rs:51)
             ▼
           Val trait impl (grpc/kuksa_val_v2/val.rs)
             │  convert proto types → broker types (conversions.rs)
             ▼
           DataBroker / AuthorizedAccess (broker.rs)
             │  Arc<RwLock<Database>>, Arc<RwLock<Subscriptions>>
             ▼
           subscribers (broadcast channels → streams back to clients)
           providers   (mpsc channels → bidirectional gRPC streams)
```

Right now, most of that picture is just words. By chapter 36 you'll be able
to explain every arrow, including which lock is held, what gets cloned and
why, and which task runs it.

## Why Rust (and why it feels hard at first)

The databroker runs inside vehicles. It must not crash, leak memory, or
corrupt data when dozens of clients use it at the same time. Rust enforces
three guarantees **at compile time**:

1. **Memory safety.** No use-after-free, no dangling pointers, no buffer
   overflows. Ownership and borrowing (chapters 02 and 11) make this possible.
2. **No data races.** Two threads can't touch the same data unsafely.
   The `Send` and `Sync` traits (chapters 13, 20–21 and 25) enforce this.
3. **No forgotten cases.** `match` must handle every variant, errors are
   values (`Result`), and there is no `null` (chapters 04 and 07).

The price is that the compiler rejects programs other languages would
accept. Treat each rejection as the compiler pointing to a real bug
that it found early. Most of this course teaches you to predict what the
compiler will say.

| If you come from… | The main shift |
|---|---|
| Python / JavaScript / Go | No garbage collector. Every value has exactly one owner, and memory is freed when that owner goes out of scope. There is no `null`: absence is `Option`. |
| Java / C# | Values are *owned*, not references by default. Assigning a `String` *moves* it. There are no exceptions: errors are return values. |
| C / C++ | Same control over memory, but the compiler checks the rules you'd otherwise keep in your head. Rust has no implicit copies of heap data. |

## Your tools

| Tool | What it does | You'll type |
|---|---|---|
| `rustup` | installs/updates Rust toolchains | `rustup update` |
| `cargo` | build tool + package manager | `cargo build`, `cargo test`, `cargo run` |
| `rustc` | the compiler (cargo calls it) | `rustc --explain E0382` |
| `rust-analyzer` | editor plugin: types on hover, go-to-definition, inline errors | install in your editor |
| `clippy` | linter that suggests more idiomatic code | `cargo clippy` |
| `rustfmt` | automatic formatter | `cargo fmt` |
| `cargo doc` | builds HTML docs for your code *and all dependencies* | `cargo doc --open` |

Two habits to build early:

- **Go-to-definition is your friend.** In the databroker, Ctrl/Cmd-click
  on any type (such as `DataBroker`) to jump to it. Do the same on library
  types (such as `broadcast::channel`) to read their docs and source.
- **`cargo doc --open -p tokio`** (run in `learn-rust/`) opens offline
  docs for the exact tokio version you're using.

## Reading a compiler error

Here's the kind of error you'll see in chapter 02:

```
error[E0382]: borrow of moved value: `path`
 --> chapters/02-ownership/exercises/c02_e01_move.rs:20:20
  |
17 |     let path = String::from("Vehicle.Speed");
   |         ---- move occurs because `path` has type `String`,
   |              which does not implement the `Copy` trait
18 |     register(path);
   |              ---- value moved here
19 |
20 |     println!("{}", path);
   |                    ^^^^ value borrowed here after move
   |
help: consider cloning the value if the performance cost is acceptable
   |
18 |     register(path.clone());
   |                  ++++++++
```

How to read it:

1. **The code (`E0382`) and title** tell you which rule was broken. Run
   `rustc --explain E0382` for a full explanation with examples.
2. **The labelled spans tell a story in order:** where the value was created,
   where it was moved, and where you tried to use it afterwards.
3. **`help:` is a suggestion, not the answer.** Here, `.clone()` would
   compile, but the better fix might be to lend (`&path`) instead of giving
   the value away. The course teaches you to tell these apart.
4. **Fix the *first* error first.** Later errors are often knock-on effects.

## How this course works

- Read `chapters/NN-*/guide.md`, then run `cargo rl` in `learn-rust/`.
- Each exercise is a small Rust file with tests at the bottom. You make the
  tests pass without editing them, then delete `// I AM NOT DONE`.
- Early exercises have one focused compile error. Later ones are `todo!()`
  bodies where you write real logic. The last exercise in each chapter is
  "databroker-shaped": a simplified copy of real code from this repository.
- The goal is Rust you can use in **any** codebase, not just this one. The
  general-Rust chapters (advanced traits, types and lifetimes, errors,
  macros, I/O, async internals, concurrency, unsafe, performance, Cargo, and
  how to read unfamiliar code) end with "In the databroker and in the wild",
  pointing at real crates such as tokio, serde and anyhow as well as the
  databroker.
- The course has six parts: Foundations (01–13), Intermediate (14–19),
  Async (20–24), Systems (25–27), Real-world domains (28–34) and Capstones
  (35–37). The last exercise of chapters 13, 19, 24, 27 and 32 is a
  **checkpoint**: a mixed-topic problem with almost no scaffolding.
- The capstones build a **mini-databroker** from scratch (chapter 35), use
  the **real** `databroker` crate as a library (chapter 36), and finish with
  projects where only the tests are given (chapter 37).
- `MASTERY.md` maps the standard Rust references to chapters and ends with a
  self-assessment checklist.

## Building and running the real databroker

You don't need to understand this yet, but seeing the finished system
running helps. From the repository root:

```sh
# one-time: fetch the .proto definitions (the dev container does this for you)
git submodule update --init

# terminal 1: run the server with the VSS 4.0 signal tree, without TLS/auth
cargo run --bin databroker -- --insecure --vss data/vss-core/vss_release_4.0.json

# terminal 2: the interactive client
cargo run --bin databroker-cli
```

In the CLI, try:

```
get Vehicle.Speed
publish Vehicle.Speed 42
get Vehicle.Speed
subscribe Vehicle.Speed        # then publish from a third terminal
```

Add `RUST_LOG=debug` in front of the server command to see the `debug!`
lines from the code. You'll learn where they come from in chapter 31.

## Mental model

| Rust idea | Analogy | Chapter |
|---|---|---|
| Ownership | Exactly one person holds the house key | 02 |
| Move | Handing your key to someone else | 02 |
| `&T` borrow | Lending a book: many readers, no scribbling | 02 |
| `&mut T` borrow | The one editor of a shared document | 02 |
| Lifetime `'a` | "This ticket is valid until…" | 11 |
| `Arc<T>` | Several remotes for one TV; the last one out turns it off | 12 |
| `RwLock<T>` | A library room: many readers, or one writer | 12, 21 |
| `Future` | A recipe card that's only cooked when someone `.await`s it | 20 |
| channel | A pneumatic tube between tasks | 21 |
| Trait | A job description; many types can apply | 09, 14 |
| `dyn Trait` | Calling "whoever holds the role" through a phone directory (vtable) | 09 |
| `Result` / `?` | A parcel that's either the goods or a damage report, passed back up | 07, 17 |
| Macro | A code template filled in before compilation | 18 |
| `Pin` | "Nailed to the floor": this value must not move again | 24 |
| Atomic | A counter you can bump without a lock, with ordering rules | 12, 25 |
| `unsafe` | "I've checked this myself": you take over the compiler's job | 26 |

## Common misconceptions

| Wrong model | Correct model |
|---|---|
| "The borrow checker is something to fight." | It encodes real bugs. When it refuses, redesign who owns what. |
| "`.clone()` fixes everything." | Clone is sometimes right (such as a cheap `Arc`) but often hides a design problem. |
| "Lifetimes are like garbage collection." | Lifetimes are compile-time *checks*. They don't change when anything is freed. |
| "`async` makes code run in parallel." | `async` makes code *pausable*. Parallelism comes from the runtime's threads and from `spawn`. |
| "I need `unsafe` to do real things." | The databroker contains no `unsafe` code. When you do need it, chapter 26 shows how to keep it small and sound. |
| "Errors are exceptions I'll deal with later." | Errors are values in the type signature. Chapters 07 and 17 teach designing them. |
| "Macros are magic I can't read." | They expand to ordinary Rust. Chapter 18 shows how to see and write that expansion. |

## Check yourself

1. What's the difference between a provider and a consumer?
2. Which file holds the in-memory signal database?
3. Which crate turns `.proto` files into Rust types, and when does that happen?
4. What are the three compile-time guarantees Rust gives the databroker?
5. What do you run to get the long explanation of error `E0502`?

<details><summary>Answers</summary>

1. Providers supply values or carry out actuations (southbound, hardware
   side). Consumers read, subscribe, or request actuations (northbound, apps).
2. `databroker/src/broker.rs` (the `Database` struct).
3. `databroker-proto`, whose `build.rs` runs at compile time.
4. Memory safety, freedom from data races, and exhaustive handling (no null, errors as values).
5. `rustc --explain E0502`.
</details>

## Further reading

- *The Rust Programming Language* ("the Book"), chapters 1–2: <https://doc.rust-lang.org/book/>
- Rust by Example: <https://doc.rust-lang.org/rust-by-example/>
- `doc/terminology.md` and `doc/user_guide.md` in this repository
- VSS: <https://covesa.github.io/vehicle_signal_specification/>
