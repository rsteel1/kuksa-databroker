# Chapter 22 hints

## c22_e01_stream_next
`sum_first`: keep a counter; `while taken < n { match stream.next().await { Some(v) => sum += v, None => break } taken += 1; }`.

`speeds_over`: chain adaptors like on an iterator. `tokio_stream::StreamExt::filter`
takes a closure returning `bool` for `&Item`; `map` transforms. The closure
uses `limit` after the function returns, so it must own it: `move |&speed| speed > limit`.

## c22_e02_receiver_stream
`spawn_producer`: `let (tx, rx) = mpsc::channel(4);`, then
`tokio::spawn(async move { for v in values { if tx.send(v).await.is_err() { break; } } });`
and finally `ReceiverStream::new(rx)`. Return immediately — don't await the task!
The stream ends when the task finishes and drops `tx`.

`as_updates`: `stream.map(|v| Ok(format!("update {v}")))`.

## c22_e03_broadcast_stream_lagged_to_none
`BroadcastStream::new(tx.subscribe())` yields
`Result<i32, BroadcastStreamRecvError>`. `.map(move |result| match result { ... })`
— `move` because the closure keeps the `Arc<AtomicU64>`. The error enum has one
variant: `BroadcastStreamRecvError::Lagged(n)`; add `n` with
`lagged.fetch_add(n, Ordering::Relaxed)` and return `None`.

`only_values`: `filter_map` with a closure that returns the `Option` as is:
`stream.filter_map(|item| item)`.

## c22_e04_async_stream_macro
`countdown`: `stream! { for n in (1..=from).rev() { sleep(every).await; yield n; } }`

`parse_all`: `try_stream! { for input in inputs { let v: i32 = input.parse()?; yield v; } }`
— inside `try_stream!`, `?` yields the error as the stream's last item and stops.

`until_shutdown`: copy the shape of the v1 `subscribe` in
`databroker/src/grpc/kuksa_val_v1/val.rs`: inside `stream! { ... }`, pin the
input (`let mut updates = Box::pin(updates);`) and `loop { tokio::select! { ... } }`
with two branches: `item = updates.next()` (yield `Ok`, or `break` on `None`)
and `_ = shutdown.recv()` (yield the `Err`, then `break`).

## c22_e05_boxed_stream_type_alias
The error says the match arms have incompatible types: `Map<Iter<..>>`,
`Map<ReceiverStream<..>>` and `Empty<..>` are three different types.
Change the return type to `SubscribeStream` and wrap each arm in `Box::pin(...)`:
all three become the same type, `Pin<Box<dyn Stream<...> + Send>>`.

## c22_e06_async_io_framing
Bring the extension traits into scope (`AsyncReadExt`, `AsyncWriteExt`, as
the file already does): they add `read_u8`, `read_u32` (big-endian),
`read_exact`, `write_u8`, `write_u32` and `write_all`. Don't forget to `flush`.

`read_frame`: `read_u8` fails with `ErrorKind::UnexpectedEof` when the input
has ended. *Before* a header that's a clean end (`Ok(None)`); anywhere later
it's a truncated frame (let `?` propagate it). Check the flag and the length
limit **before** allocating `vec![0; len as usize]`.

`frame_stream`: `futures::stream::unfold(Some(reader), |state| async move { ... })`.
Take the reader out of the state with `let mut reader = state?;`. Return
`Some((item, next_state))` to yield, and `None` to finish. After an error,
make the next state `None` so the stream ends.
