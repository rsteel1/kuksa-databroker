# What "knowing Rust" covers, and where each part is taught

This course aims at complete, codebase-independent Rust understanding. To
check that claim instead of trusting it, the tables below map the contents
of the standard references to the chapter that teaches each topic. A topic
marked *guide* is explained in the chapter text without an exercise of its
own. A topic marked *pointer* is deliberately out of scope, with a note on
where to go next.

Use the tables two ways:

1. **Coverage.** Before starting, see that nothing important is missing.
2. **Self-assessment.** After finishing, go through the checklist at the
   bottom. Anything you can't do from memory tells you which chapter to revisit.

---

## The Rust Programming Language ("the Book")

| Book chapter | Topic | Course |
|---|---|---|
| 1–2 | Getting started, cargo, a first program | 00, 01 |
| 3 | Variables, types, functions, control flow | 01 |
| 4 | Ownership, references, slices | 02 |
| 5 | Structs and methods | 03 |
| 6 | Enums, `match`, `if let` | 04 |
| 7 | Packages, crates, modules, paths | 08, 33 |
| 8 | Vec, String, HashMap | 05, 06 |
| 9 | Error handling, `panic!` vs `Result` | 07, 17 |
| 10 | Generics, traits, lifetimes | 09, 11 |
| 11 | Writing tests | 32 (and every chapter's tests) |
| 12 | I/O project (a minigrep) | 19 (`vss-grep`), 37 |
| 13 | Closures and iterators | 10 |
| 14 | Cargo and crates.io | 08, 33 |
| 15 | Smart pointers: `Box`, `Rc`, `RefCell`, `Drop`, `Weak` and reference cycles | 12 |
| 16 | Fearless concurrency: threads, channels, `Mutex`, `Send`/`Sync` | 13, 25 |
| 17 | Async and await: futures, streams | 20–24 |
| 18 | OOP-style design, trait objects, the state pattern | 09, 14 |
| 19 | Patterns and matching | 04, 06 |
| 20 | Advanced features: unsafe, advanced traits and types, macros | 14, 15, 16, 18, 26 |
| 21 | Final project: multithreaded web server | 37 (`kv_server`) |

## Asynchronous Programming in Rust (the Async Book) and the Tokio tutorial

| Topic | Course |
|---|---|
| `async`/`.await`, futures are lazy | 20 |
| The `Future` trait, `Poll`, `Context`, `Waker` | 24 |
| Executors and building one | 24 |
| `Pin`, `Unpin`, self-referential futures | 16 (motivation), 24 |
| `join!`, `select!`, cancellation safety | 20, 24 |
| Spawning, `JoinHandle`, `'static + Send` bounds | 20 |
| Shared state: `Mutex` (std vs tokio), `RwLock`, locks across `.await` | 21 |
| Channels: mpsc, oneshot, broadcast, watch | 21 |
| Streams, `StreamExt`, implementing `Stream` | 22, 24 |
| Async traits and dyn compatibility | 23 |
| Timeouts, deadlines, graceful shutdown | 20, 21, 31 |
| I/O: `AsyncRead`/`AsyncWrite`, TCP, framing | 22 (framing exercise), 37 (`kv_server`) |
| tower `Service`/`Layer` | 24 |

## The Rustonomicon (unsafe Rust)

| Topic | Course |
|---|---|
| What `unsafe` does and doesn't allow | 26 |
| Soundness, safe abstractions, invariants | 26 |
| Raw pointers, aliasing rules | 26 |
| Data layout, `repr(C)`, alignment | 15, 26 |
| Uninitialised memory, `MaybeUninit` | 26 |
| Ownership-based resource management, `Drop`, leaking | 12, 26 |
| Variance, subtyping, `PhantomData` | 14, 16 |
| Send/Sync and atomics | 13, 25 |
| FFI (calling C, exposing Rust) | 26 |
| Undefined behaviour catalogue, Miri | 26, guide 32 |

## Rust for Rustaceans (Jon Gjengset)

| Book chapter | Course |
|---|---|
| 1 Foundations (memory, ownership, lifetimes in depth) | 02, 11, 15, 16 |
| 2 Types (layout, generics and dispatch, trait objects, coherence) | 09, 14, 15 |
| 3 Designing interfaces (unsurprising, flexible, obvious, constrained) | 33, 14 (typestate, sealed) |
| 4 Error handling (representing, propagating, enumerating errors) | 07, 17 |
| 5 Project structure (features, workspaces, versioning, MSRV) | 08, 33 |
| 6 Testing (harness, doc tests, property tests, fuzzing, Miri, loom) | 32 |
| 7 Macros (declarative and procedural) | 18 |
| 8 Asynchronous programming (futures, Pin, wakers, executors) | 20–24 |
| 9 Unsafe code | 26 |
| 10 Concurrency (models, atomics, memory ordering, lock-free) | 13, 25 |
| 11 Foreign function interfaces | 26 |
| 12 Rust without the standard library (`no_std`) | guide 26, 33 (*pointer*: the Embedded Rust Book) |
| 13 The Rust ecosystem (crates, tools, patterns) | 33, 34 |

## Rust API Guidelines (checklist)

| Area | Course |
|---|---|
| Naming (`as_`/`to_`/`into_`, no `get_`, iterator names) | 33, "Smells and idioms" in every chapter |
| Interoperability (common traits, conversions, `Send`/`Sync`, error types) | 09, 14, 17, 33 |
| Macros | 18 |
| Documentation (`///`, `//!`, examples, errors/panics/safety sections, intra-doc links) | 32 (doc tests), 33 (rustdoc) |
| Predictability (methods vs functions, no surprising `Deref`) | 14, 33 |
| Flexibility (generics, `impl Trait`, `AsRef`/`Into` parameters) | 09, 14, 33 |
| Type safety (newtypes, builders, typestate) | 03, 14 |
| Dependability (validate arguments, `Drop` doesn't fail) | 03, 12, 17 |
| Future-proofing (sealed traits, `#[non_exhaustive]`, private fields) | 14, 17, 33 |

## Beyond the books: skills for working in real codebases

| Skill | Course |
|---|---|
| Reading compiler errors, `rustc --explain` | 00 onward, every chapter's EXPECT line |
| Reading an unfamiliar codebase methodically | 34, 99 |
| Reading generated and macro-expanded code | 18, 30, 34 |
| Tracing a request through a real async server | 99 (four lifecycles) |
| Performance: allocations, layout, profiling, benchmarking | 27 |
| Service plumbing: config, logging/tracing, signals, shutdown | 31 |
| gRPC/protobuf with tonic/prost | 30 |
| Serialization with serde | 28 |
| CI hygiene: fmt, clippy, feature matrices | 32, 99 |
| Editions and MSRV | 33 |

## Deliberately out of scope (pointers)

- **WebAssembly**: the Rust and WebAssembly book, `wasm-bindgen`.
- **Embedded and bare metal in depth**: the Embedded Rust Book, `embassy`.
  Chapter 26 covers the `no_std` concepts you need to read such code.
- **GUI frameworks and game engines**: domain-specific. Everything they
  rely on is covered here.
- **Compiler internals and writing lints**: the rustc dev guide.

---

## Self-assessment checklist

You "know Rust" for practical purposes when you can do each of these without
looking anything up. The chapter numbers tell you where to revisit.

**Foundations**
- [ ] Predict whether a snippet compiles, and if not, which borrow rule it breaks (02, 11)
- [ ] Explain what physically happens in a move vs a copy vs a clone (02)
- [ ] Design a type so invalid states can't be represented (03, 04, 14)
- [ ] Choose between `&str`, `String`, `Cow<str>`, `Box<str>`, `Arc<str>` (06, 15)
- [ ] Write an iterator pipeline instead of an index loop, and implement `Iterator` (10)
- [ ] Explain why a function signature needs a named lifetime and which one (11)

**Intermediate**
- [ ] Decide between generics, `impl Trait` and `dyn Trait` and justify it (09, 14)
- [ ] Explain variance and why `&mut T` is invariant (16)
- [ ] Design error types for a library and for an application, and say what differs (17)
- [ ] Decide whether a failure should retry, fall back, fail fast or panic (17)
- [ ] Write a `macro_rules!` macro and read a derive macro's expansion (18)
- [ ] Build a CLI program whose core is testable without files or a terminal (19)

**Async**
- [ ] Explain what `.await` compiles to and what `Pin` guarantees (24)
- [ ] Know which locks can be held across `.await`, and why (21)
- [ ] Pick the right channel (mpsc, oneshot, broadcast, watch) for a job (21)
- [ ] Make a slow dependency unable to stall the whole service (21, 23)
- [ ] Explain cancellation safety in `select!` (20, 24)

**Systems**
- [ ] Choose a memory ordering for an atomic and justify it (25)
- [ ] Write a sound safe wrapper around `unsafe` code, with SAFETY comments (26)
- [ ] Find where the allocations are and remove the unnecessary ones (27)

**Real-world**
- [ ] Open any crate on docs.rs or GitHub and find its core types and one full request path within an hour (34)
- [ ] Add a feature to the databroker with tests, clippy and fmt clean (36, 99)
- [ ] Review a Rust PR for lock scoping, error handling, cloning and API design (17, 21, 33)
