# Chapter 35 hints

## c35_e01_database_register
Start with `is_valid_path`: `path.split('.')` gives you the segments, and
`Iterator::all` checks a condition for every one. Note that `"".split('.')`
yields one empty segment — so the empty path needs no special case.

`matches_type`: match on the *pair* `(self, data_type)` and list the legal
combinations; `matches!(expr, pattern | pattern | ...)` is a compact way to
turn a match into a `bool`. `NotAvailable` pairs with `_`.

`add` is four early returns in a row: invalid path → `Err`, known path →
`Ok(existing id)` (`if let Some(&id) = self.path_to_id.get(&new.path)`),
bad `allowed` → `Err`, then build the `Entry`. `fetch_add(1, Ordering::SeqCst)`
returns the value *before* adding, so the first id is 0. Insert into
`path_to_id` and `entries`. You'll need `new.path` twice — clone it once
for the metadata and move it into the map.

`get_entry_by_path`: `self.path_to_id.get(path).ok_or(ReadError::NotFound)?`
then call `get_entry_by_id`.

## c35_e02_update_and_validate
`validate_value` is a chain of guard clauses. For min: `value.compare(min)`
returns `Option<Ordering>`; the value is fine when that's
`Some(Greater | Equal)`, anything else (including `None`) is
`OutOfBoundsMinMax`. `allowed.contains(value)` does the allowed-list check.

`validate`: `if let Some(dp) = &update.datapoint` and
`if let Some(Some(target)) = &update.actuator_target` — the nested pattern
skips the "clear the target" case for free. Use `?` to bail out.

`diff` takes `mut update` by value, so you may set `update.datapoint = None`
and return it.

`apply`: move the datapoint in (`self.datapoint = datapoint`) and insert
`Field::Datapoint` into a `HashSet`. For the target, match only the *outer*
`Option` so that `Some(None)` still counts as a change.

`update`: `self.entries.get_mut(&id).ok_or(UpdateError::NotFound)?` gives
`&mut Entry`; then description check, `let update = entry.diff(update);`,
`entry.validate(&update)?;`, `Ok(entry.apply(update))`.

## c35_e03_authorized_access
The `From` impls are plain two-arm `match`es. Once they exist, `?` on a
`Result<(), PermissionError>` inside a function returning
`Result<_, ReadError>` converts automatically.

`get_entry_by_id`: look up in `self.db.entries`, then
`self.permissions.can_read(&entry.metadata.path)?`. Because `self.db` is a
`&'db Database`, the entry reference you get from it is `&'db Entry` —
exactly the return type. That's why the reference outlives the view.

`readable_paths`: `.values().map(|e| e.metadata.path.as_str())
.filter(|p| self.permissions.can_read(p).is_ok()).collect()`, then sort.

Write access `update`: borrow the path immutably for the checks (only when
the corresponding field `is_some()`), then call `self.db.update(id, update)`.
The immutable borrow ends before the mutable one begins (non-lexical
lifetimes), so the borrow checker is happy. If it isn't, make sure you don't
use `path` after calling `self.db.update`.

## c35_e04_subscriptions_notify
`notify`: start with `let db = db.authorized_read_access(&self.permissions);`
and an empty `EntryUpdates`.

- `None`: loop over `&self.entries`; for each readable entry push
  `Self::build_notification(entry, fields.clone())`.
- `Some(changed)`: loop over `changed`; `let Some(subscribed) =
  self.entries.get(id) else { continue };`; the fields to send are
  `subscribed.intersection(changed_fields).copied().collect()`. Match the read
  result: `Err(ReadError::PermissionExpired) => return Err(NotificationError)`.
  Return `Ok(())` early if nothing was collected.

Sort with `updates.sort_by_key(|n| n.id)`, then
`self.sender.send(Some(msg))` — map its `Result<usize, _>` into
`Result<(), NotificationError>`.

`update_entries`: `let mut db = self.broker.database.write().await;` then, in
an inner block, create the write view and loop over the updates, collecting
`changed` (skip empty field sets) and `errors`. After the block,
`let db = db.downgrade();` and call
`self.broker.subscriptions.read().await.notify(Some(&changed), &db)`.
If the compiler complains that `db` is still mutably borrowed when you
downgrade, your write view lives too long — end its scope first.

## c35_e05_subscribe_stream
Validate first (`is_empty`, then a `match buffer_size` with a guard arm
`Some(size) if size > MAX_SUBSCRIBE_BUFFER_SIZE`). Then
`broadcast::channel(capacity)`, `ChangeSubscription::new(entries, sender,
self.permissions.clone())`, initial `notify(None, &db)` with a database read
guard, register via `subscriptions.write().await.add_change_subscription(..)`.

The stream: `BroadcastStream::new(receiver).map(|r| match r { Ok(m) => m,
Err(BroadcastStreamRecvError::Lagged(_)) => None })`, returned as
`Ok(Box::pin(stream))`.

The race question: if you drop the database guard before registering, an
update can be applied and notified *before* your subscription is in the
list — you'd miss it. Holding the read guard until after registration closes
the gap (writers must wait for the read guard).

`subscribe_paths`: build the `HashMap<i32, HashSet<Field>>` inside a block
that holds the read guard, map `ReadError`s with `map_err`, and call
`self.subscribe(..)` *after* the block.

## c35_e06_housekeeping
`cleanup_change_subscriptions`: `self.change_subscriptions.retain(|uuid, sub|
{ ... })` — return `false` to remove. Push removed uuids into a `Vec` captured
by the closure (`*uuid`, it's `Copy`).

`start_housekeeping_task`:
```text
let subscriptions = Arc::clone(&self.subscriptions);
let mut shutdown = self.shutdown_trigger.subscribe();   // before spawn!
tokio::spawn(async move {
    let mut interval = tokio::time::interval(HOUSEKEEPING_INTERVAL);
    loop {
        tokio::select! {
            _ = interval.tick() => { /* cleanup under a write lock */ }
            _ = shutdown.recv() => break,
        }
    }
})
```
A broadcast receiver only sees messages sent after it was created — that's
why subscribing inside the task fails the "right after start" test.

`shutdown`: `self.subscriptions.write().await.clear();` then
`let _ = self.shutdown_trigger.send(());`. Clearing drops every
`ChangeSubscription`, which drops its `Sender`, which ends the streams.
