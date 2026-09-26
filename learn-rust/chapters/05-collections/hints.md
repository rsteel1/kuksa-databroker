# Chapter 05 hints

## c05_e01_vec
`Vec::with_capacity(n)` allocates once up front. `push` appends to the back.
For dropping the oldest items, look at `Vec::drain(..k)` — it removes the
first `k` elements in one go (much cheaper than `remove(0)` in a loop).
`last()` gives `Option<&f32>`; `.copied()` turns it into `Option<f32>`.
For `last_n`, compute the start index with `len().saturating_sub(n)` (it
clamps at 0 instead of underflowing) and return `&self.readings[start..]`.
Average: `self.readings.iter().sum::<f32>() / len as f32`, but return `None`
first when empty (division by zero would give NaN).

## c05_e02_hashmap_path_to_id
`register`: first `if let Some(id) = self.path_to_id.get(path) { return *id; }`.
Otherwise take `self.next_id`, bump it, and insert into BOTH maps
(`path.to_string()` makes an owned `String` key from the `&str`).
`id_of`: `self.path_to_id.get(path).copied()`. Note that `get` accepts a
`&str` even though keys are `String` — that's the `Borrow` trait at work.
`path_of`: `self.id_to_path.get(&id).map(String::as_str)`.
`count_by_root`: `path.split('.').next()` gives the first segment, then
`*counts.entry(root.to_string()).or_insert(0) += 1;`.

## c05_e03_hashset_ops
`subscription_matches`: iterate `changed` and ask, for each id, whether the
subscription has fields for it (`subscribed.get(id)`) and those fields are
not disjoint: `!fields.is_disjoint(changed_fields)`. `Iterator::any` and
`Option::is_some_and` make it a one-liner.
`already_provided`: `owned.intersection(requested).copied().collect::<Vec<_>>()`
then `sort_unstable()` — set iteration order is not defined.
`may_publish`: `publishing.is_subset(registered)`.

## c05_e04_btreemap_sorted
`build`: `.iter().map(|(p, u)| (p.to_string(), u.to_string())).collect()` —
collecting pairs into a BTreeMap inserts them in order, so later duplicates win.
`children_of`: build `let prefix = format!("{branch}.");` (the dot matters:
"Vehicle.CabinX" must not count as a child of "Vehicle.Cabin"). Then
`map.range(prefix.clone()..)` starts at the first key >= prefix;
`.take_while(|(k, _)| k.starts_with(&prefix))` stops at the first non-child.
`first_and_last`: `first_key_value()` and `last_key_value()` return Options —
use `?` to bail out on an empty map.

## c05_e05_indexmap_order
Create `IndexMap::with_capacity(requested.len())` and loop over `requested`
in order. For each id: `provider.get(id).or_else(|| database.get(id))`
gives the first available `Option<&f64>`; `.copied().ok_or(*id)?` turns a
miss into `Err(id)` and returns early. `insert` into the IndexMap preserves
insertion order, so the result iterates in request order.

## c05_e06_retain_cleanup
The compiler error (E0502) is the point: `for (id, sub) in &self.subscriptions`
borrows the map for the whole loop, and `self.subscriptions.remove(id)` needs
`&mut` at the same time. Rust forbids that.
Use `self.subscriptions.retain(|id, sub| { ... keep })`: the closure returns
`true` to keep an entry. Inside it you may push `*id` into a `removed` Vec
declared before the call (the closure borrows it mutably). Sort before returning.
`drop_unavailable` is the same with `Vec::retain(|p| ...)`, which keeps the
original order of survivors — push `p.name.clone()` for the ones you drop.
