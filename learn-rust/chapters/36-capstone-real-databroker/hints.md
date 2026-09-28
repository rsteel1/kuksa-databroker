# Chapter 36 hints

## c36_e01_register_and_read
`broker.authorized_access(&permissions::ALLOW_ALL)` gives an
`AuthorizedAccess`. `add_entry` takes nine arguments in this order: name
(`String`), `DataType`, `ChangeType`, `EntryType`, description (`String`),
min, max, allowed (all `Option<DataValue>`), unit (`Option<String>`). It is
`async`, so end with `.await` — the result is already the
`Result<i32, RegistrationError>` you need to return.

For the allowed list: `DataValue::StringArray(vec!["P".to_string(), ...])`.

`current_value`: `get_datapoint_by_path(path).await` returns
`Result<Datapoint, ReadError>`; `.map(|dp| dp.value)` keeps only the value.

`sorted_paths`: `.map_entries(|entry| entry.metadata().path.clone()).await`
gives a `Vec<String>`; sort it. (You must clone: the closure only borrows the
entry while the database read lock is held.)

## c36_e02_publish_and_subscribe
`publish`: `EntryUpdate { datapoint: Some(Datapoint { ts, source_ts, value }),
..Default::default() }`, then
`broker.authorized_access(permissions).update_entries([(id, update)]).await`.
An array of tuples satisfies `impl IntoIterator<Item = (i32, EntryUpdate)>`.

`watch`: build `HashMap<i32, HashSet<Field>>` with
`ids.iter().map(|id| (*id, HashSet::from([Field::Datapoint]))).collect()`,
then `.subscribe(entries, Some(10), None).await.expect(..)`.

Adapt the stream with `StreamExt::filter_map`: the closure gets
`Option<EntryUpdates>`; `let updates = message?;` skips `None`. Each
`ChangeNotification` has `change.update.path: Option<String>` and
`change.update.datapoint: Option<Datapoint>`. Sort the pairs by path and
return `Some(pairs)`. Finally `Box::pin(stream)`.

## c36_e03_permissions_denied
`Permissions::builder()` returns a `PermissionBuilder`; each
`add_*_permission(Permission::...)` consumes and returns it, so you can chain.
`build()` returns a `Result` (globs are compiled to regexes) — use
`.expect(..)`.

- cabin: `.add_read_permission(Permission::Glob("Vehicle.Cabin".to_string()))`
- speed provider: `.add_provide_permission(...)`
- expired: `.add_read_permission(Permission::All)
  .expires_at(SystemTime::now() - Duration::from_secs(3600))`

`readable_paths`: in the `filter_map_entries` closure,
`entry.datapoint().ok()?;` bails out for unreadable entries, then
`Some(entry.metadata().path.clone())`. Sort the result.

## c36_e04_implement_signal_provider
`update_filter` → `Ok(())`; `is_available` → `true`.

`get_signals_values_from_provider`: `self.calls.fetch_add(1,
Ordering::Relaxed);` then iterate `signals_ids`, `filter_map` the ones found in
`self.values` (`self.values.get(&signal_id.id())?.clone()`), build a
`Datapoint`, and `collect()` into an `IndexMap<SignalId, Datapoint>`
(`IndexMap` keeps insertion order — that's why the broker uses it). Empty →
`Err(())`.

`register_provider`: `ids.iter().map(|id| (SignalId::new(*id),
TimeInterval::new(0))).collect::<HashMap<_, _>>()`, then
`.register_signals(signals, Box::new(provider)).await`. `Box<FixedProvider>`
coerces to `Box<dyn SignalProvider + Send + Sync>` automatically.

`read_values`: `get_values_broker(ids.iter().copied().map(SignalId::new)
.collect()).await?` — the error type already matches — then map
`response.entries` into `(signal_id.id(), datapoint.value)`.

## c36_e05_grpc_get_values_end_to_end
`authorized`: `let mut request = tonic::Request::new(message);
request.extensions_mut().insert(permissions.clone()); request`.

`by_path`: `proto::SignalId { signal:
Some(proto::signal_id::Signal::Path(path.to_string())) }`.

`publish_float`: build `proto::PublishValueRequest { signal_id, data_point }`
with `proto::Datapoint { timestamp: None, value: Some(proto::Value {
typed_value: Some(proto::value::TypedValue::Float(value)) }) }`. Call
`broker.publish_value(authorized(request, permissions)).await` — the `Val`
trait must be in scope (it is imported at the top) — and map the response
to `()`.

`get_floats`: `broker.get_values(..).await?.into_inner().data_points`, then
for each datapoint use `?` on `Option` inside a closure returning
`Option<f32>`: `match dp.value?.typed_value? { TypedValue::Float(f) =>
Some(f), _ => None }`.

## c36_e06_review_signal_provider
Three independent bugs, each a mismatch between what a `SignalProvider`
promises and what it does.

Bug 1: `is_available` returns `self.calls.load(..) == 0`, so the very
first successful answer makes the provider permanently "unavailable" —
the broker would then stop calling it and only ever read the database.
This provider has no real notion of being down; it should just return
`true`.

Bug 2: `.any(|id| !self.values.contains_key(&id.id()))` triggers `Err(())`
as soon as ONE requested id is unknown — but `get_values_broker` treats
`Err(())` as "the provider answered nothing at all" and falls back to the
database for every id in the request, discarding whatever this provider
DID know. The contract is "error only if we know none of them":
`.all(|id| !self.values.contains_key(&id.id()))`.

Bug 3: sorting `ids` before calling `get_values_broker` reorders the
response too — `get_values_broker` builds its reply by walking the exact
list you hand it, in that order, because that's the whole point (the gRPC
contract promises request order back). Drop the `sort_unstable_by_key`
call; converting `i32` to `SignalId` is all this function needs to do.
