# 22 — Streams

**Why this matters for the databroker.** A databroker subscription *is* a
stream: the client calls `Subscribe` once and receives an open-ended sequence
of updates. Providers talk over bidirectional gRPC streams. Internally,
`AuthorizedAccess::subscribe` returns `impl Stream<Item = Option<EntryUpdates>>`,
the gRPC layer transforms it into a stream of protobuf responses, and tonic
sends each item over HTTP/2. Understanding streams means understanding the
data path from a `PublishValue` to every subscriber's screen.

## Streams are async iterators

| | Sync | Async |
|---|---|---|
| One value | `T` | `Future<Output = T>` |
| Many values | `Iterator<Item = T>` | `Stream<Item = T>` |

`Stream` lives in the `futures-core` crate (re-exported by `futures` and
`tokio-stream`). Its core method is `poll_next`, but you use the extension
trait **`StreamExt`**:

```rust
use tokio_stream::StreamExt;
while let Some(update) = stream.next().await { ... }   // None = stream ended
let v: Vec<_> = stream.map(f).filter(g).take(10).collect().await;
```

Iterator habits carry over — `map`, `filter`, `filter_map`, `take`,
`collect`, `fold` — but results come through `.await`.

**Two `StreamExt`s.** `tokio_stream::StreamExt` and `futures::StreamExt` both
exist, with slightly different method sets (e.g. futures' `filter` takes an
async closure; tokio-stream's takes a sync one). Import only one per scope, or
you'll get "multiple applicable items in scope". The databroker uses
`tokio_stream::StreamExt` (`databroker/src/broker.rs:25`).

**Why `.next()` sometimes demands `Unpin`.** `next()` borrows the stream
mutably across an await. Some streams (e.g. those made by `async-stream`)
contain self-references and must not move once polled; for those,
`next()` requires them to be pinned first: `let mut s = Box::pin(s);` or
`tokio::pin!(s);`. Adaptors that take the stream by value (`collect`, `map`)
don't need that.

## Channels as streams

`tokio_stream::wrappers` adapts Tokio channels into streams:

* `ReceiverStream::new(mpsc_rx)` — yields each message; ends when all senders drop.
* `BroadcastStream::new(broadcast_rx)` — yields `Result<T, BroadcastStreamRecvError>`.
* also `WatchStream`, `IntervalStream`, `TcpListenerStream`, `UnixListenerStream`
  (the gRPC server uses the last two — `databroker/src/grpc/server.rs:22`).

The **server-streaming pattern**: create a channel, spawn a task that fills
the sender, and return the receiver as a stream *immediately*:

```rust
// databroker/src/grpc/kuksa_val_v2/val.rs — open_provider_stream
let (response_stream_sender, response_stream_receiver) = mpsc::channel(10);
tokio::spawn(async move { /* loop: handle provider messages, send responses */ });
Ok(tonic::Response::new(ReceiverStream::new(response_stream_receiver)))
```

When the task exits and drops the sender, the stream ends and tonic closes
the gRPC response. When the *client* goes away, tonic drops the stream (and
thus the receiver), so the task's `send` fails — that's the signal to stop.

## BroadcastStream and the subscribe pipeline

Here is the full path of a v2 subscription:

```
PublishValue ──► update_entries ──► ChangeSubscription::notify
                                    └─ broadcast::Sender::send(Some(EntryUpdates))
                                                 │
AuthorizedAccess::subscribe:                     ▼
  BroadcastStream::new(receiver)                          Stream<Result<Option<EntryUpdates>, Lagged>>
    .map(Ok(m) => m, Err(Lagged(n)) => { warn!; None })   Stream<Option<EntryUpdates>>
                                                 │
grpc/kuksa_val_v2/val.rs subscribe:              ▼
  convert_to_proto_stream(stream, size)
    .filter_map(Some(u) => Some(Ok(proto)), None => None) Stream<Result<SubscribeResponse, Status>>
  Box::pin(...)                                           SubscribeStream
                                                 │
tonic ──► HTTP/2 frames ──► client
```

The code (`databroker/src/broker.rs:1665-1678`):

```rust
let stream = BroadcastStream::new(receiver).map(move |result| match result {
    Ok(message) => message,
    Err(BroadcastStreamRecvError::Lagged(n)) => {
        warn!("Slow subscriber with capacity {} lagged, missed {} signal updates",
              channel_capacity, n);
        None
    }
});
```

Design points worth noticing:

* A lagging subscriber is **not** disconnected — it skips ahead. Its stream
  continues.
* The item type is `Option<EntryUpdates>`: `None` doubles as "nothing to send
  right now" (lag) and as "a provider for your signals disappeared" (the
  housekeeping task sends `None` on purpose — `send_none_to_receiver`,
  `databroker/src/broker.rs:1057`). The gRPC layer drops `None`s with
  `filter_map` (`databroker/src/grpc/kuksa_val_v2/val.rs:1339`).
* The `move` closure captures `channel_capacity` by value so the stream can
  outlive `subscribe`.
* When the client disconnects, tonic drops the stream → the `BroadcastStream`
  → the `broadcast::Receiver`. The subscription's `receiver_count()` drops to
  0 and housekeeping removes it. Resource cleanup via `Drop`, no explicit
  "unsubscribe" message needed.

## Writing streams by hand: async-stream

For streams with their own control flow, the `async-stream` crate lets you
write a generator-like block:

```rust
use async_stream::{stream, try_stream};

let s = stream! {
    for n in (1..=3).rev() {
        tokio::time::sleep(one_second).await;
        yield n;
    }
};

let parsed = try_stream! {       // Item = Result<i32, E>
    for input in inputs {
        let v: i32 = input.parse()?;   // on error: yield Err(e) and end
        yield v;
    }
};
```

The v1 API uses this to end a subscription cleanly when the broker shuts down
(`databroker/src/grpc/kuksa_val_v1/val.rs:622`):

```rust
let wrapped = stream! {
    let mut s = Box::pin(stream);
    loop {
        tokio::select! {
            item = s.next() => match item { Some(res) => yield res, None => break },
            _ = shutdown_rx.recv() => {
                yield Err(Status::unavailable("Databroker shutting down"));
                break;
            }
        }
    }
};
```

(Rust will eventually have native `async gen` blocks; until then, this macro
is the idiom. Streams made by it are `!Unpin` — hence `Box::pin` before
`.next()`.)

## Pin<Box<dyn Stream>>: one type for many streams

`impl Stream<Item = X>` in return position means "one concrete type I won't
name". If different branches produce different stream types (an `Iter`, a
`ReceiverStream`, an `Empty`), `impl Stream` can't describe them all
(E0308, "match arms have incompatible types"). **Type erasure** fixes it:
put each one in a box behind a trait object.

```rust
type SubscribeStream =
    Pin<Box<dyn Stream<Item = Result<proto::SubscribeResponse, tonic::Status>> + Send + Sync + 'static>>;

Ok(tonic::Response::new(Box::pin(stream)))   // databroker/src/grpc/kuksa_val_v2/val.rs:440
```

Read the type inside-out:

* `dyn Stream<Item = ...>` — "some stream of these items", decided at runtime.
* `+ Send + Sync + 'static` — tonic moves it into its own task and keeps it
  around, so it must be thread-safe and own all its data.
* `Box<...>` — a `dyn` value has no fixed size; it must live behind a pointer.
* `Pin<...>` — a promise that the stream won't be moved again. Async state
  machines may hold pointers into themselves (a local borrowed across an
  `.await`); moving them would leave those pointers dangling. Heap-allocating
  and pinning once makes that safe. `Box::pin(x)` does both in one call.

**Pin in one sentence:** it's the type system's way of saying "this future or
stream has been put somewhere it will never move from, so it may safely point
into itself." Day to day you only need three tools: `Box::pin(x)`,
`tokio::pin!(x)` (pin on the stack), and the error messages that ask for them.

tonic's generated service trait requires such a named associated type for
every server-streaming method (`type SubscribeStream`, `type
SubscribeByIdStream`, `type OpenProviderStreamStream`) because traits can't
(yet) use `impl Trait` there in a way tonic supports. `OpenProviderStreamStream`
avoids boxing by naming the concrete type `ReceiverStream<...>`
(`databroker/src/grpc/kuksa_val_v2/val.rs:880`).

## Async I/O: bytes in, frames out

Sockets, files and pipes implement `tokio::io::AsyncRead`/`AsyncWrite`: the
async versions of `std::io::Read`/`Write` (chapter 19). Their extension
traits `AsyncReadExt`/`AsyncWriteExt` add the convenient methods
(`read_exact`, `read_u32`, `write_all`, `flush`), just as `StreamExt`
does for `Stream`.

A byte stream has **no message boundaries**. One `write_all` of 300 bytes
may arrive as three reads of 100, and two small writes may arrive as one.
Every protocol therefore defines **framing**:

* **Delimiters.** Messages end at `\n` (Redis-style text protocols; chapter
  37's key-value server uses `BufReader::lines()`).
* **Length prefixes.** A header states the size of the following payload.
  gRPC does this for every message on an HTTP/2 stream: 1 compression-flag
  byte, a 4-byte big-endian length, then the protobuf bytes. That is how the
  databroker's messages are delimited on the wire. tonic's codec does it for you.

Framing turns `AsyncRead` (bytes) into a `Stream` of messages, which is
exactly what `tonic::Streaming<T>` is. Rules for writing a frame reader:

1. **Distinguish a clean EOF from a truncated frame.** EOF between frames
   means "done"; EOF inside a frame is an error.
2. **Validate lengths before allocating.** Otherwise a peer that sends
   `0xFFFFFFFF` makes you allocate 4 GiB.
3. **Stop after an error** instead of trying to resynchronise with a corrupt stream.

In production, `tokio_util::codec` (`FramedRead`, `LengthDelimitedCodec`,
`LinesCodec`) packages this pattern. `tokio::io::duplex(n)` gives you an
in-memory pipe with a tiny buffer, which is ideal for testing partial reads
and backpressure without any network.

## Mental model

| Concept | Analogy |
|---|---|
| `Stream` | A conveyor belt: items arrive over time; `next().await` waits for the next one |
| `None` from `next()` | The belt has been switched off |
| Adaptors (`map`, `filter_map`) | Stations along the belt that modify or remove items |
| `ReceiverStream` | A belt fed by a worker in the back room (the spawned task) |
| `BroadcastStream` lag | The belt only holds N items; if you're slow, the oldest fall off |
| `Pin<Box<dyn Stream>>` | A sealed box with a standard label so any belt fits the same slot |

**Guarantee:** a stream item type is checked end to end — tonic can only send
what your stream yields. `Pin` guarantees self-referential futures/streams
aren't moved after they've been polled.

## Common misconceptions

| Wrong model | Correct model | You'll see |
|---|---|---|
| "A Stream is a Vec that arrives later" | It may be infinite; `collect` on a subscription never returns | hung tests |
| "`impl Stream` works for any branch" | Only one concrete type; box to mix | E0308 |
| "`Pin` changes the value" | It only restricts moving it | `Unpin` not satisfied errors |
| "Lag closes the subscription" | Items are skipped; stream continues | — |
| "Unsubscribe needs an explicit call" | Dropping the stream drops the receiver; housekeeping cleans up | — |
| "Both StreamExt traits are the same" | Different signatures; import one | "multiple applicable items" |

## Smells and idioms

* `collect()` on a live subscription in a test → use `take(n)` or `next()`
  with a `timeout` (the databroker's tests do `timeout(1s, stream.next())`,
  `databroker/src/broker.rs:4960`).
* Boxing when one concrete type would do (`-> impl Stream` is zero-cost).
  Box only when you must mix types or name the type in a trait.
* `.expect(...)` inside a stream closure (e.g. `convert_to_proto_stream`
  expects `update.path`) panics the *task serving that client*. Consider
  whether an error item would be better.
* Unbounded buffering between a fast source and slow client → bounded channel
  or broadcast with explicit lag handling.

## In the databroker

* `databroker/src/broker.rs:23-25` — imports: `BroadcastStream`,
  `BroadcastStreamRecvError`, `Stream`, `StreamExt`.
* `databroker/src/broker.rs:1601-1680` — `subscribe` builds and returns the stream.
* `databroker/src/broker.rs:1057` — `send_none_to_receiver`: a deliberate `None`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:369-376` — `type SubscribeStream = Pin<Box<dyn Stream<...> + Send + Sync + 'static>>`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:440` — `Box::pin(stream)`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:880` — `type OpenProviderStreamStream = ReceiverStream<...>`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:1072` — `ReceiverStream::new(response_stream_receiver)`.
* `databroker/src/grpc/kuksa_val_v2/val.rs:1339-1366` — `convert_to_proto_stream`: `filter_map` from broker types to protobuf.
* `databroker/src/grpc/kuksa_val_v1/val.rs:622` — `stream!` + `select!` for shutdown-aware subscriptions.
* `databroker/src/grpc/server.rs:22` — `TcpListenerStream`/`UnixListenerStream`: even incoming connections are a stream.

## Exercises

| Exercise | Goal |
|---|---|
| `c22_e01_stream_next` | `while let Some(x) = s.next().await`; adaptors |
| `c22_e02_receiver_stream` | Spawn a producer, return a `ReceiverStream` |
| `c22_e03_broadcast_stream_lagged_to_none` | Rebuild the `subscribe` pipeline and lag counter |
| `c22_e04_async_stream_macro` | `stream!`, `try_stream!`, and the v1 shutdown-aware stream |
| `c22_e05_boxed_stream_type_alias` | Mix stream types behind `Pin<Box<dyn Stream + Send>>` |
| `c22_e06_async_io_framing` | Length-prefixed framing (gRPC's wire format) over `AsyncRead`/`AsyncWrite`; frames as a `Stream` |

## Further reading

* Tokio tutorial — "Streams": https://tokio.rs/tokio/tutorial/streams
* `tokio-stream` docs (`wrappers` module): https://docs.rs/tokio-stream
* `async-stream` docs: https://docs.rs/async-stream
* Async Book — "Pinning": https://rust-lang.github.io/async-book/
* `std::pin` module docs (the long explanation): https://doc.rust-lang.org/std/pin/

## Check yourself

1. What's the async counterpart of `Iterator::next`, and what does `None` mean?
2. How does a client disconnect eventually remove its subscription from the broker?
3. Why does the v2 `subscribe` map lag to `None` instead of ending the stream?
4. Why can't `fn f(b: bool) -> impl Stream<...>` return `iter(..)` in one branch and `empty()` in the other?
5. Why does `SubscribeStream` need `+ Send + 'static`?
6. When do you need `Box::pin(s)` before calling `s.next()`?

<details><summary>Answers</summary>

1. `StreamExt::next(&mut s).await` → `Option<Item>`; `None` means the stream has ended.
2. tonic drops the response stream → the `BroadcastStream` → the `broadcast::Receiver`; `receiver_count()` becomes 0 and the housekeeping task's `cleanup_change_subscriptions` removes it.
3. A slow subscriber should keep receiving the newest data rather than be disconnected; `None` is filtered out before reaching the wire.
4. `impl Trait` names exactly one concrete type; the branches have different types. Box them (`Pin<Box<dyn Stream>>`) or use an enum like `futures::future::Either`.
5. tonic drives the stream from its own spawned task on a multi-threaded runtime, and the stream outlives the handler function.
6. When the stream isn't `Unpin` (e.g. one produced by `stream!` or containing an `async` block) and you want to call `next()` on it in place.

</details>

## Where this goes next

**Chapter 24** implements `Stream::poll_next` by hand and explains the `Pin` in `Pin<Box<dyn Stream>>`. Chapter 30 shows tonic sending these streams over gRPC.
