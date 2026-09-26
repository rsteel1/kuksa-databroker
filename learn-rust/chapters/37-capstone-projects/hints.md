# Chapter 37 hints

These are projects: the hints give design direction, not code. Try to get
each test passing on your own first, in the order the tests are written.

## c37_e01_kv_server
Design before coding. You need four things:

1. **Shared state.** A map behind a lock, cloned into every connection task
   as `Arc`. A std `Mutex` is fine as long as you never hold it across an
   `.await` (chapter 21).
2. **A change feed.** One `broadcast` channel. Every SET publishes, and
   every connection holds a receiver. To report only SETs made *after*
   SUBSCRIBE, give each change a sequence number, assigned under the same
   lock as the store update, and remember the number at SUBSCRIBE time.
3. **A stop signal.** A `watch` (or `broadcast`) channel that `run` flips on
   shutdown. Each connection `select!`s on it.
4. **One task per connection** in a `JoinSet`, so `run` can wait for all of
   them after signalling stop.

Each connection loop `select!`s over three things: the next line from the
client, the next change from the feed, and the stop signal. Split the
socket with `into_split()` so reading and writing don't fight over one
`&mut`. Keep the command parsing a plain function that returns the reply
text. That keeps the async part small.

In `run`, `tokio::pin!(shutdown)` lets you poll the same future in every
loop iteration with `&mut shutdown`.

## c37_e02_signal_cli
Structure it as layers: `run` parses args, dispatches, and turns errors into
text and exit codes. Below it, `load(path) -> Result<Tree, CliError>` does
I/O and parsing, and pure functions `query` and `tree` write to a `&mut impl Write`.

* clap derive with a `#[derive(Subcommand)]` enum. Use `try_parse_from(args)`
  instead of `parse()`, which would exit the test process. The error knows
  its own exit code (`exit_code()`) and whether it belongs on stderr (`use_stderr()`).
* serde: a recursive `struct Node { #[serde(rename = "type")] kind: String,
  #[serde(default)] children: BTreeMap<String, Node>, datatype: Option<String>, .. }`.
  `BTreeMap` sorts the children for free.
* Errors: two layers with thiserror, `CliError` (`#[source]` an
  `io::Error` or a `FormatError`) and `FormatError` (JSON or structural).
  Print the chain by walking `Error::source()` (chapter 17).
* Globs: reuse the databroker's trick of replacing `.` with `/` and calling
  `glob_match`. For a pattern without `*`, match the path itself or anything
  under `pattern/`.

## c37_e03_plugin_system
* Store `kinds: HashMap<String, Factory>` and
  `instances: BTreeMap<String, Instance>`, where `Instance` holds the boxed
  plugin, a failure counter and a status. `BTreeMap` iterates in name order.
* All-or-nothing loading: build the new instances in a *staging* map, collect
  errors as you go, and merge (`append`) only if there were none. A duplicate
  name can clash with an existing instance *or* one staged earlier in the same text.
* Parse a line with `split_once('=')` for the name, then `split_whitespace()`.
  The first token is the kind, and each remaining token must itself be `key=value`.
* Isolation: `std::panic::catch_unwind(AssertUnwindSafe(|| plugin.on_update(..)))`.
  Its `Err` holds a `Box<dyn Any + Send>`, and `downcast_ref::<&str>()` /
  `downcast_ref::<String>()` recover the message (chapter 14). Disable the
  instance straight away: `AssertUnwindSafe` is only honest if you never
  use the possibly half-updated state again.
