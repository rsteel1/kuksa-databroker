# Curriculum map

The goal is a complete understanding of Rust, deep enough to read, debug and
contribute to **any** Rust codebase. The KUKSA databroker is the running
example and the capstone: every chapter shows where it uses the concept, and
general-Rust chapters also point at well-known crates ("In the wild").

Each chapter has one guide file and a run of exercises. Chapters marked
**NEW** were added in the general-Rust expansion. "DB" means where the
concept appears in `kuksa-databroker`. `MASTERY.md` maps every topic in the
standard references (the Book, the Async Book, the Nomicon, Rust for
Rustaceans, the API Guidelines) to the chapter that covers it.

## Part I — Foundations

### 00 Setup & the big picture (guide only)
Toolchain, cargo, rust-analyzer, clippy, rustfmt, how to read compiler errors,
`rustc --explain`, how this course works, what the databroker is (VSS, signals,
providers, consumers, gRPC APIs), building and running it, workspace tour.

### 01 Basics — `c01`
e01_variables (let/mut) · e02_types_and_casts (integer widths, `as`, overflow,
`try_from` — the Int8/Uint8 range checks) · e03_functions (expressions vs
statements, return values) · e04_control_flow (if/loop/while/for, `loop` with
break value) · e05_tuples_arrays_shadowing (shadowing for transformation).
DB: `types.rs` numeric variants, `broker.rs` validate min/max.

### 02 Ownership & borrowing — `c02`
e01_move (E0382) · e02_clone_vs_move · e03_borrow (&T) · e04_mut_borrow (&mut,
E0499/E0502) · e05_string_vs_str (paths as `&str`) · e06_slices_and_returning_refs
(E0106/E0515). DB: `get_entry_by_path(&self, path: impl AsRef<str>)`, clone of
Datapoint when leaving a lock.

### 03 Structs, methods & newtypes — `c03`
e01_define_struct (Datapoint) · e02_impl_methods (`&self`/`&mut self`/`self`) ·
e03_associated_fns_new_default (`new`, `#[derive(Default)]`, `Default` impl) ·
e04_derives (Debug/Clone/PartialEq/Eq/Hash — why `Field` derives Hash) ·
e05_struct_update_syntax (`EntryUpdate { datapoint, ..Default::default() }`) ·
e06_validated_newtype (SignalId-style newtype + a `VssPath` that can only be
constructed valid). DB: `broker.rs` Metadata/Datapoint/Entry/EntryUpdate, `types.rs` SignalId.

### 04 Enums & pattern matching — `c04`
e01_simple_enum (EntryType/ChangeType) · e02_enum_with_data (DataValue) ·
e03_exhaustive_match (DataType Display, `_` pitfalls) · e04_option (if let,
let-else, `?` on Option) · e05_option_option_tristate (EntryUpdate "leave
unchanged / set to None / set to value") · e06_match_on_tuples (DataValue
`greater_than` with type widening). DB: `types.rs`, `broker.rs` `Entry::apply`.

### 05 Collections — `c05`
e01_vec · e02_hashmap_path_to_id (+ entry API) · e03_hashset_ops (is_disjoint,
intersection, subset — subscription matching) · e04_btreemap_sorted (VSS
flattened tree) · e05_indexmap_order (GetValues must preserve request order) ·
e06_retain_cleanup (Subscriptions::cleanup). DB: `Database`, `Subscriptions`,
`get_values_broker`.

### 06 Strings, text & formatting — `c06` (NEW)
e01_utf8_bytes_chars (`len()` is bytes, `chars()`, `char_indices`, why
`s[0]` doesn't compile, byte-slicing panics on a char boundary) ·
e02_string_building (`String::with_capacity`, `push_str`, `format!`, `write!`
into a `String`, `join`, and why `+` moves the left operand) · e03_parse_fromstr
(`str::parse`, implementing `FromStr`, `Err` associated type) ·
e04_formatting_traits (`Display` vs `Debug`, `{:>8.2}`, `{:#?}`, `{:x}`,
`Formatter` flags, implementing `Display` with width support) ·
e05_advanced_patterns (match ergonomics, binding modes, `ref`/`ref mut`,
`@` bindings, guards, slice patterns `[first, .., last]`, nested
destructuring, `matches!`) · e06_cow_str (return borrowed-or-owned text
without allocating in the common case). DB: VSS path handling, `Display`
impls in `types.rs`, `databroker-proto` `FromStr for DataType`.

### 07 Error handling I: the basics — `c07`
e01_result_basics · e02_question_mark · e03_custom_error_enum (+Display +
std::error::Error) · e04_from_conversions (`?` auto-converts; `impl From<serde_json::Error>`
as in `vss.rs`) · e05_thiserror (derive the same thing) · e06_panic_vs_result
(`expect` vs `unwrap` vs Result; `anyhow` in binaries). DB: `UpdateError`,
`vss::Error`, `(ActuationError, String)` tuples, `main.rs` `Box<dyn Error>`.

### 08 Modules, crates & Cargo basics — `c08`
e01_inline_modules_and_pub · e02_use_paths_and_reexports (`pub use crate::types`) ·
e03_visibility_pub_crate · e04_cfg_attributes (`#[cfg(test)]`, `cfg!`,
`#[cfg_attr(...)]`; guide covers features, workspaces, build.rs, OUT_DIR).
DB: `lib.rs`, `Cargo.toml` features `tls`/`viss`/`otel`, `databroker-proto/build.rs`.

### 09 Traits & generics — `c09`
e01_define_and_impl_trait · e02_default_methods · e03_display_impl ·
e04_from_into_tryfrom (`impl From<proto::Value> for DataValue` shape) ·
e05_generic_bounds_and_where · e06_impl_trait_args (`impl Into<String>`,
`impl AsRef<str>`, `impl IntoIterator<Item=...>` — `DataBroker::new`,
`update_entries`) · e07_trait_objects_dyn (static vs dynamic dispatch,
`Box<dyn Trait>`, object safety E0038). DB: `conversions.rs`, `ActuationProvider`.

### 10 Closures & iterators — `c10`
e01_closure_capture · e02_fn_fnmut_fnonce (`with_read_lock(f: impl FnOnce)`) ·
e03_iterator_adaptors (map/filter/filter_map/collect into HashMap/HashSet) ·
e04_collect_results (`collect::<Result<Vec<_>,_>>`) · e05_custom_iterator
(EntryReadIterator wrapping `hash_map::Values` + permission check) ·
e06_subscription_matching (fold/any/all/chain over the subscription-matching loop). DB: `AuthorizedAccess::map_entries`, `EntryReadIterator`.

### 11 Lifetimes — `c11`
e01_elision · e02_precise_lifetimes (E0106, tying the output to the right input) · e03_struct_holding_refs
(`DatabaseReadAccess<'a,'b>`) · e04_enum_holding_refs (`EntryReadAccess<'a>`) ·
e05_scoped_authorized_view (Database → authorized_read_access → view that
can't outlive the DB or the permissions). DB: `broker.rs` 1062-1180, 1341-1380.

### 12 Smart pointers, Drop & interior mutability — `c12`
e01_box_recursive_tree (VSS tree) · e02_drop_raii (guard that runs cleanup) ·
e03_rc_refcell · e04_arc_shared · e05_mutex_rwlock (std) · e06_atomics
(AtomicI32 `next_id`, `fetch_add`, Ordering) · e07_oncelock_lazylock
(`ALLOW_ALL` without lazy_static) · e08_weak_parent_links (`Weak` for
parent pointers, breaking `Rc` cycles). DB: `Database::next_id`, `permissions::ALLOW_ALL`,
`Provider::next_request_id`.

### 13 Threads, Send & Sync — `c13`
e01_spawn_join_move · e02_arc_mutex_counter · e03_std_mpsc_channel ·
e04_send_sync_providers (Rc across threads → E0277; fix with Arc). DB: why
`SignalProvider: Send + Sync + 'static`.

## Part II — Async Rust (the databroker's core)
Plus **e05_checkpoint_part1** (NEW): a mixed-topic problem with no guide
pointer and minimal scaffolding. Parse a small signal-update log (text → typed
records with errors), keep the latest value per path in a thread-safe store
updated by several worker threads, and answer queries through an iterator
API with lifetimes. It uses chapters 01–13 together.

## Part II — Intermediate Rust

### 14 Advanced traits — `c14` (NEW)
e01_associated_types_vs_generics (why `Iterator` has `type Item` and why
`From<T>` is generic) · e02_supertraits_and_blanket_impls
(`impl<T: Display> Describe for T`) · e03_orphan_rule_newtype (E0117 and the
newtype workaround) · e04_operator_overloading (`Add`, `Index`, `Neg`,
`PartialOrd` by hand) · e05_deref_asref_borrow (when to implement `Deref`,
the "`Deref` for inheritance" anti-pattern, `Borrow` for `HashMap` lookups by
`&str`) · e06_any_downcast (`dyn Any`, `downcast_ref`; DB uses
`std::any::Any` in `databroker-proto`) · e07_sealed_and_marker_traits (sealed
trait pattern, marker traits, auto traits) · e08_typestate_phantomdata
(builder whose `build()` exists only once every required field is set).
In the wild: std `Iterator`, `serde::Serialize` blanket impls, tower `Service`.

### 15 Advanced types & memory layout — `c15` (NEW)
e01_size_and_layout (`size_of`, alignment, padding, field reordering,
`#[repr(C)]`) · e02_niche_optimization (`Option<&T>`, `Option<Box<T>>` and
`Option<NonZeroU32>` cost nothing extra; tests assert sizes) ·
e03_dsts_and_fat_pointers (`str`, `[T]`, `dyn Trait`; `&[T]` is two words;
`?Sized` bounds) · e04_const_generics (`[T; N]` generic code, a fixed-capacity
ring buffer) · e05_function_pointers_vs_closures (`fn` vs `Fn`,
`Box<dyn Fn>`, returning closures) · e06_never_type_and_aliases (`!`,
`Infallible`, `type` aliases such as the databroker's `SubscribeStream`) ·
e07_box_str_rc_str (`Box<str>`, `Arc<str>` for shared immutable strings,
interning signal paths).

### 16 Advanced lifetimes — `c16` (NEW)
e01_static_bound_vs_static_ref (`T: 'static` means "owns its data", not
"lives forever"; why `tokio::spawn` needs it) · e02_lifetime_bounds (`T: 'a`,
`dyn Trait + 'a`, default object lifetime) · e03_hrtb (`for<'a> Fn(&'a str) -> &'a str`,
where closures need HRTB) · e04_variance (covariance of `&'a T`, invariance
of `&'a mut T` and `Cell<T>`, the classic `&mut Vec<&'static str>` bug) ·
e05_reborrowing_and_nll (implicit reborrows, two-phase borrows, and why
NLL accepts code that looks wrong) · e06_self_referential_problem (why a
struct can't hold a reference into itself; fix with indices, `Arc`, or split
ownership, which leads into `Pin` in chapter 24).

### 17 Error handling II: mastery — `c17` (NEW)
e01_combinators (`map`, `map_err`, `and_then`, `ok_or_else`, `unwrap_or_else`,
`transpose`, `flatten`, `?` on `Option` vs `Result`, `Option<Result>` ↔
`Result<Option>`) · e02_error_source_chains (`Error::source`, walking a
chain, rendering "caused by:" output, `#[source]`/`#[from]` in thiserror) ·
e03_library_error_design (one error type per module/crate boundary,
`#[non_exhaustive]`, avoiding `String` errors, keeping the underlying cause,
matching on `io::ErrorKind`-style kinds) · e04_application_errors_anyhow
(`anyhow::Context`, `bail!`, `ensure!`, downcasting an `anyhow::Error` back
to a typed error, `main() -> anyhow::Result<()>`) ·
e05_error_categories_and_recovery (user-facing vs internal vs transient vs
permanent; an `is_retryable()` method; mapping to gRPC status codes like
`conversions.rs` does; never leaking internals to clients) ·
e06_retry_backoff_circuit_breaker (retry only transient errors with capped
exponential backoff and a maximum attempt count, plus a small circuit
breaker. The clock and sleep are injected, so tests are deterministic and
the code isn't tied to one runtime) · e07_panics_unwinding_poisoning
(when to panic, `catch_unwind` at a boundary, `panic = "abort"`, `Mutex`
poisoning and `PoisonError::into_inner`, `#[must_use]`, `#[track_caller]`) ·
e08_errors_across_boundaries (`Box<dyn Error + Send + Sync + 'static>` and
why each bound is needed, errors crossing threads, errors in iterator
pipelines, fail-fast vs collect-all). DB: `UpdateError`, `(ActuationError,
String)` tuples, the provider-fallback strategy in `get_values_broker`,
`to_status_with_code`, and the `todo!()` panic reachable from `Actuate`
(`conversions.rs:431`), fixed properly with `TryFrom`.

### 18 Macros — `c18` (NEW)
e01_macro_rules_basics (fragments `expr`/`ident`/`ty`/`tt`, hygiene) ·
e02_repetition (`$(...),*`, building a `HashMap` literal macro) ·
e03_recursive_and_tt_munching (a small DSL, like declaring VSS signals) ·
e04_macros_that_generate_items (generating `impl` blocks for many types, the
way `types.rs` repeats per-numeric-type code) · e05_reading_derive_output
(what `#[derive(Debug, Clone)]` and `#[derive(Deserialize)]` expand to, and
`cargo expand` usage; tests pin the behaviour of hand-written equivalents) ·
e06_write_a_derive_macro (proc macro in the `exercise-macros` crate: a
`#[derive(SignalNames)]` that lists a struct's field names, using
`syn`/`quote` and `compile_error!` for bad input). In the wild:
`tokio::main`, `async_trait`, `serde`, `clap::Parser`, `tonic::include_proto!`.

### 19 std I/O, files, processes & CLI programs — `c19` (NEW)
e01_read_write_traits (`Read`/`Write`/`BufRead`, generic over readers, which
is how `vss::parse_vss_from_reader` works) · e02_files_and_paths
(`Path`/`PathBuf`, `OsStr`, `fs::read_to_string`, a temporary directory in
tests, why paths aren't `String`s) · e03_line_processing (streaming a big
input with `lines()` without loading it all) · e04_process_and_env
(`std::process::Command`, exit codes, `env::var`, `std::process::exit` vs
returning from `main`) · e05_cli_program_end_to_end (a small `vss-grep`
tool: args → read file → filter → output, fully testable because the core
is a function over `impl BufRead` and `impl Write`) ·
e06_checkpoint_part2 (mixed-topic, minimal scaffolding: a config-file
loader with layered errors (chapter 17), a declarative macro for defaults
(18), `FromStr` parsing (06) and a typestate builder (14)).

## Part III — Async Rust (the databroker's core)

### 20 Async basics with Tokio — `c20`
e01_async_await · e02_tokio_spawn_joinhandle · e03_sleep_and_timeout
(`start_paused`) · e04_join_all_concurrency (sequential vs concurrent —
commit 748a914) · e05_select (first of message / shutdown) ·
e06_blocking_and_send (spawn_blocking; "future is not Send" from a std
MutexGuard held across await). DB: `start_housekeeping_task`, `open_provider_stream`.

### 21 Async locks & channels — `c21`
e01_tokio_mpsc_backpressure · e02_oneshot_request_reply · e03_broadcast_and_lag
(`RecvError::Lagged`) · e04_shutdown_trigger (broadcast `()` + select) ·
e05_rwlock_downgrade (write → downgrade → notify under read) ·
e06_snapshot_then_await (don't hold the subscriptions lock while awaiting a
provider — commit e1fdfd1) · e07_deadline_budget (`saturating_duration_since`,
one deadline across phases). DB: `update_entries`, `get_values_broker`,
`Provider::get_signals_values_from_provider`.
Plus e08_watch_latest_value (`tokio::sync::watch` for "latest value wins" state).

### 22 Streams — `c22`
e01_stream_next (StreamExt) · e02_receiver_stream (mpsc → Stream) ·
e03_broadcast_stream_lagged_to_none (exact `subscribe` pattern) ·
e04_async_stream_macro · e05_boxed_stream_type_alias
(`Pin<Box<dyn Stream<Item=..> + Send>>`, `type SubscribeStream`). DB:
`AuthorizedAccess::subscribe`, `convert_to_proto_stream`.
Plus e06_async_io_framing (`AsyncRead`/`AsyncWrite`, `AsyncBufReadExt::lines`,
framing a line protocol over a stream).

### 23 Async traits & the provider pattern — `c23`
e01_async_fn_in_trait (native, static dispatch) · e02_async_trait_for_dyn
(why `#[async_trait]` for `Arc<dyn SignalProvider>`) · e03_provider_registry
(`Arc<dyn Trait>` in a map, `is_available`) · e04_bounded_provider_calls
(timeout hanging provider, join_all, fallback) · e05_request_id_correlation
(AtomicU32 request ids, skip stale responses, partial response fallback —
commit 7a210d0). DB: `SignalProvider`, `Provider` impl in `grpc/kuksa_val_v2/val.rs`.

## Part III — The databroker's domains

### 24 Async internals — `c24` (NEW)
e01_future_by_hand (implement `Future` for a countdown, `Poll`, `Context`) ·
e02_waker_and_timer (a future that stores the `Waker` and gets woken from
another thread) · e03_mini_executor (a single-threaded executor built only
on std: task queue, `Wake` trait, `block_on`) · e04_pin_and_unpin (why
self-referential futures must be pinned, `Pin<&mut Self>`, `Box::pin`,
`pin!`, `pin-project-lite` projection) · e05_stream_by_hand (implement
`Stream::poll_next`, then compare with `async-stream`) ·
e06_cancellation_and_drop (a future dropped mid-await runs `Drop`;
cancellation-safe vs unsafe `select!` branches; `JoinHandle::abort`,
`JoinError::is_panic`, panics inside spawned tasks) ·
e07_tower_service (implement `tower::Service` plus a middleware `Layer`
for timeout/logging: the abstraction tonic and axum are built on) ·
e08_checkpoint_part3 (mixed-topic: a rate-limited, cancellable fan-out with
a broadcast shutdown, bounded channels and a timeout budget).
DB: `tonic` interceptors are tower layers (`grpc/server.rs`),
`ReceiverStream`, and the `select!` loop in `open_provider_stream`.

## Part IV — Systems Rust

### 25 Concurrency in depth — `c25` (NEW)
e01_scoped_threads (`thread::scope`, borrowing across threads with no `Arc`) ·
e02_condvar (a blocking bounded queue) · e03_atomics_cas_loop
(`compare_exchange`, a lock-free max tracker, the ABA problem in words) ·
e04_memory_ordering (`Relaxed` vs `Acquire`/`Release` vs `SeqCst` with a
publication flag; why `Relaxed` is fine for `next_request_id`) ·
e05_deadlock_and_lock_ordering (reproduce a two-lock deadlock with
`try_lock`, then fix it with a consistent lock order) ·
e06_data_parallelism_by_hand (chunked parallel map over scoped threads;
the guide explains rayon's work stealing) · e07_rwlock_vs_mutex_vs_channels
(same problem three ways, and when each wins). In the wild:
`crossbeam`, `parking_lot`, `rayon`, `dashmap`.

### 26 Unsafe Rust & FFI — `c26` (NEW)
e01_raw_pointers (create, deref inside `unsafe`, `// SAFETY:` comments) ·
e02_split_at_mut (implement it with raw pointers; soundness vs "it
compiles") · e03_safe_abstraction_invariants (a type whose safe API upholds
the invariants its unsafe interior relies on) · e04_maybeuninit_and_layout
(initialising an array element by element) · e05_ffi_call_c (call libc
`strlen`/`abs` through `extern "C"`, `CStr`/`CString`, ownership across the
boundary) · e06_ffi_expose_rust (an `extern "C" fn` with `#[no_mangle]` and a
C-compatible `#[repr(C)]` struct; callbacks) · e07_ub_catalogue (spot the UB:
reading the listed snippets and choosing sound rewrites; tests exercise the
sound versions). Guide: the Nomicon's rules, Miri (`cargo +nightly miri
test`), `unsafe_code` lints, `no_std` and embedded (relevant for in-vehicle
Rust). Files opt in with `#![allow(unsafe_code)]`.

### 27 Performance & profiling — `c27` (NEW)
e01_counting_allocator (a `GlobalAlloc` wrapper that counts allocations;
later tests assert allocation budgets) · e02_avoid_allocations
(`with_capacity`, reuse buffers, `&str` instead of `String`, `Cow`) ·
e03_iterators_are_zero_cost (index loop vs iterator, `chunks_exact`, bounds
checks; the tests are about correctness and allocations, and the guide covers
reading assembly with cargo-show-asm) · e04_data_layout_for_speed (struct of
arrays vs array of structs, `Vec` over linked lists, `SmallVec`-style inline
storage by hand) · e05_hashing_and_collections (`HashMap` with capacity,
the choice of hasher, `BTreeMap` vs `HashMap`, `IndexMap`) ·
e06_checkpoint_part4 (mixed-topic: a lock-free-read, allocation-budgeted
signal cache with an unsafe-free public API). Guide: benchmarking with
criterion and `std::hint::black_box`, profiling with `cargo flamegraph` and
samply, release profiles (`lto`, `opt-level = "s"`, `codegen-units` in the
databroker's `Cargo.toml`), and when not to optimise.

## Part V — Real-world domains (databroker-shaped)

### 28 Serde & VSS JSON — `c28`
e01_derive_deserialize · e02_rename_default_optional · e03_enum_representations
(`rename_all`, untagged) · e04_serde_json_value (default/allowed values of
mixed type → DataValue) · e05_flatten_vss_tree (recursive tree → `BTreeMap`
of dotted paths). DB: `vss.rs`.

### 29 Strings, globs & permissions — `c29`
e01_string_methods (split/join/starts_with/strip_suffix) ·
e02_validate_vss_path (`is_valid_path`) · e03_glob_to_matcher (VSS glob →
`glob_match` pattern, the "branch matches children" rule) ·
e04_builder_pattern (PermissionBuilder, consuming `self`) ·
e05_permissions_can_read (expiry + read-implied-by-other-scopes). DB: `glob.rs`,
`permissions.rs`, `authorization/jwt/scope.rs`, doc/wildcard_matching.md.

### 30 Protobuf & gRPC with tonic/prost — `c30`
e01_prost_messages (build generated structs from `databroker_proto`) ·
e02_oneof_enums (`signal_id::Signal::{Path,Id}`) · e03_proto_conversions
(Value ↔ DataValue both directions) · e04_status_codes (map domain errors →
`tonic::Status`) · e05_request_extensions_interceptor (Bearer token →
extension, `Request<()>` interceptor shape) · e06_call_val_service_directly
(call the real `Val` trait on `DataBroker` with a `tonic::Request`).
DB: `databroker-proto`, `grpc/server.rs`, `grpc/kuksa_val_v2/{val,conversions}.rs`.

### 31 Building a service: config, logging, shutdown — `c31`
e01_clap_args_env (derive + `env`, defaults) · e02_tracing_basics (levels,
fields, `#[instrument]`, EnvFilter from RUST_LOG) · e03_graceful_shutdown
(signal future → broadcast → tasks exit) · e04_cfg_feature_gates (code that
compiles both ways; guide covers `--features viss,otel`). DB: `main.rs`,
`lib.rs` `init_logging`, `grpc/server.rs` `shutdown`.

### 32 Testing & quality — `c32`
e01_unit_tests_and_should_panic · e02_async_tests_paused_time ·
e03_test_doubles_for_traits (Hanging/Counting providers like `broker.rs`
tests) · e04_test_builders (GetValuesConfigBuilder-style table tests).
Guide: integration tests, cucumber (`databroker/tests`), clippy, fmt, CI.

## Part IV — Capstones
Plus (NEW) e05_property_testing (`proptest`: round-trip `DataValue` ↔ proto
and glob matching invariants) · e06_checkpoint_part5 (write the test suite,
table-driven plus property-based, for a provided implementation that
contains two hidden bugs; the grader runs your tests against good and bad
versions). Guide adds: doc tests (and why integration-test files can't have
them), `cargo test` flags, Miri, fuzzing with cargo-fuzz, loom, coverage,
mutation testing.

### 33 Cargo, the ecosystem & API design — `c33` (NEW)
e01_semver_and_breaking_changes (which changes are breaking; the tests
encode a checklist) · e02_feature_flags_design (additive features, feature
unification, `dep:` syntax, the databroker's `tls`/`viss`/`otel`) ·
e03_api_guidelines (naming: `as_`/`to_`/`into_`, no `get_`, `impl
Into<String>` parameters, `#[must_use]`, the builder, `Debug` on every
public type; refactor a deliberately awkward API) · e04_editions_and_msrv
(2015 → 2018 → 2021 → 2024 differences that change how code reads: `dyn`,
paths, disjoint closure captures, RPIT capture rules, `gen` keyword, let
chains) · e05_choosing_crates (the blessed.rs categories; read a
`Cargo.toml` and `cargo tree` output and answer dependency questions in
code). Guide: workspaces and inheritance, `build.rs`, `cargo deny`/audit,
publishing, docs.rs, `no_std` crates.

### 34 Reading any Rust codebase — `c34` (NEW)
A method: `Cargo.toml` → `lib.rs`/`main.rs` → public types → one request
path → tests. Use rust-analyzer (go to definition, find implementations,
call hierarchy), `cargo doc --open`, `cargo tree`, `cargo expand`, and
`git log -p`. Read-then-rebuild drills on crate sources already on disk
(`~/.cargo/registry/src/*/`, exact versions from `Cargo.lock`):
e01_read_tokio_broadcast (read `tokio::sync::broadcast`, then build a
mini version with the same lag semantics) · e02_read_anyhow_error (then
build a mini `anyhow` with context chains) · e03_read_indexmap (then build
an order-preserving map on `Vec` + `HashMap`) · e04_read_serde_json_value
(then write a mini JSON value type with an indexing API) ·
e05_read_tonic_generated_code (answer structured questions about the
generated `val_server` in `OUT_DIR` by implementing a stub that mirrors it).

## Part VI — Capstones

### 35 Capstone: build a mini-databroker — `c35`
e01_database_register (ids, path_to_id, metadata) · e02_update_and_validate
(type check, min/max, allowed) · e03_authorized_access (lifetimed views +
permissions) · e04_subscriptions_notify (broadcast per subscription, field
matching) · e05_subscribe_stream (initial notification, lag → None,
buffer-size rules) · e06_housekeeping (cleanup when `receiver_count()==0`,
spawned interval task, shutdown).

### 36 Capstone: drive the real databroker — `c36`
e01_register_and_read (`DataBroker`, `authorized_access`, `add_entry`,
`get_datapoint`) · e02_publish_and_subscribe (`update_entries` + `subscribe`
stream) · e03_permissions_denied (PermissionBuilder globs → PermissionDenied) ·
e04_implement_signal_provider (real `SignalProvider` trait, `register_signals`,
`get_values_broker` fallback) · e05_grpc_get_values_end_to_end (Val trait
`get_values` + `publish_value` with Permissions extension).

### 37 Capstone projects with no scaffolding — `c37` (NEW)
Only function signatures and tests are given, with no hints in the file,
and design decisions are yours. e01_kv_server (a TCP key-value server
with a line protocol on tokio: GET/SET/SUBSCRIBE, graceful shutdown; tests
connect over loopback on port 0) · e02_signal_cli (a CLI that loads a VSS
JSON file, answers glob queries and prints a formatted tree, with errors
done properly) · e03_plugin_system (trait-object plugins registered at
runtime, config-driven, with error isolation so one plugin's panic doesn't
take down the host).

### 99 Contributing (guide only)
Codebase tour file by file; three request lifecycles traced end-to-end
(PublishValue → notify subscribers; OpenProviderStream; GetValues with
providers); how to run tests/clippy/fmt; good first contributions.
